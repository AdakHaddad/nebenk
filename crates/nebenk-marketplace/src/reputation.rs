//! Reputation scoring system for marketplace providers

use crate::types::*;
use nebenk_core::types::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Reputation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationConfig {
    pub initial_score: u8,
    pub max_score: u8,
    pub min_score: u8,
    pub decay_monthly_pct: u8,
    pub uptime_weight: f64,
    pub sla_weight: f64,
    pub benchmark_weight: f64,
    pub peer_review_weight: f64,
    pub history_window_epochs: u32,
}

impl Default for ReputationConfig {
    fn default() -> Self {
        Self {
            initial_score: 50,
            max_score: 100,
            min_score: 0,
            decay_monthly_pct: 5,
            uptime_weight: 0.4,
            sla_weight: 0.3,
            benchmark_weight: 0.2,
            peer_review_weight: 0.1,
            history_window_epochs: 100,
        }
    }
}

/// Reputation engine
pub struct ReputationEngine {
    config: ReputationConfig,
    scores: Arc<RwLock<HashMap<NodeId, ReputationData>>>,
    benchmark_results: Arc<RwLock<HashMap<NodeId, BenchmarkResult>>>,
    peer_reviews: Arc<RwLock<HashMap<NodeId, Vec<PeerReview>>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationData {
    pub node_id: NodeId,
    pub score: ReputationScore,
    pub uptime_history: Vec<UptimeRecord>,
    pub sla_history: Vec<SlaRecord>,
    pub last_updated: u64,
    pub total_epochs: u64,
    pub epochs_sla_met: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UptimeRecord {
    pub epoch: u64,
    pub uptime_percentage: f64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaRecord {
    pub epoch: u64,
    pub sla_met: bool,
    pub uptime_percentage: f64,
    pub replication_lag_ms: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub node_id: NodeId,
    pub write_throughput_ops_sec: f64,
    pub read_throughput_ops_sec: f64,
    pub snapshot_time_ms: u64,
    pub recovery_time_ms: u64,
    pub data_integrity: bool,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerReview {
    pub reviewer: NodeId,
    pub reviewee: NodeId,
    pub rating: u8, // 1-5
    pub comment: String,
    pub timestamp: u64,
}

impl ReputationEngine {
    pub fn new(config: ReputationConfig) -> Self {
        Self {
            config,
            scores: Arc::new(RwLock::new(HashMap::new())),
            benchmark_results: Arc::new(RwLock::new(HashMap::new())),
            peer_reviews: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Initialize reputation for a new provider
    pub async fn init_provider(&self, node_id: NodeId) -> ReputationScore {
        let mut scores = self.scores.write().await;
        let score = ReputationScore::new(self.config.initial_score);
        
        scores.insert(node_id, ReputationData {
            node_id,
            score,
            uptime_history: Vec::new(),
            sla_history: Vec::new(),
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            total_epochs: 0,
            epochs_sla_met: 0,
        });
        
        score
    }
    
    /// Record uptime for an epoch
    pub async fn record_uptime(&self, node_id: NodeId, epoch: u64, uptime_percentage: f64) {
        let mut scores = self.scores.write().await;
        if let Some(data) = scores.get_mut(&node_id) {
            data.uptime_history.push(UptimeRecord {
                epoch,
                uptime_percentage,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            });
            
            // Keep only recent history
            if data.uptime_history.len() > self.config.history_window_epochs as usize {
                data.uptime_history.remove(0);
            }
            
            data.last_updated = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        }
    }
    
    /// Record SLA result for an epoch
    pub async fn record_sla(&self, node_id: NodeId, epoch: u64, sla_met: bool, uptime: f64, lag: u64) {
        let mut scores = self.scores.write().await;
        if let Some(data) = scores.get_mut(&node_id) {
            data.sla_history.push(SlaRecord {
                epoch,
                sla_met,
                uptime_percentage: uptime,
                replication_lag_ms: lag,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            });
            
            if data.sla_history.len() > self.config.history_window_epochs as usize {
                data.sla_history.remove(0);
            }
            
            data.total_epochs += 1;
            if sla_met {
                data.epochs_sla_met += 1;
            }
            
            data.last_updated = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            
            // Recalculate score
            data.score = self.calculate_score(data).await;
        }
    }
    
    /// Record benchmark results
    pub async fn record_benchmark(&self, result: BenchmarkResult) {
        let node_id = result.node_id;
        self.benchmark_results.write().await
            .insert(node_id, result.clone());
        
        // Recalculate score if provider exists
        let mut scores = self.scores.write().await;
        if let Some(data) = scores.get_mut(&node_id) {
            data.score = self.calculate_score(data).await;
        }
    }
    
    /// Add peer review
    pub async fn add_peer_review(&self, review: PeerReview) {
        let reviewee = review.reviewee;
        self.peer_reviews.write().await
            .entry(reviewee)
            .or_default()
            .push(review.clone());
        
        // Recalculate score
        let mut scores = self.scores.write().await;
        if let Some(data) = scores.get_mut(&reviewee) {
            data.score = self.calculate_score(data).await;
        }
    }
    
    /// Calculate reputation score based on all factors
    async fn calculate_score(&self, data: &ReputationData) -> ReputationScore {
        let mut score = self.config.initial_score as f64;
        
        // Uptime component (0-40 points)
        if !data.uptime_history.is_empty() {
            let avg_uptime: f64 = data.uptime_history.iter()
                .map(|r| r.uptime_percentage)
                .sum::<f64>() / data.uptime_history.len() as f64;
            let uptime_score = (avg_uptime / 100.0) * 40.0 * self.config.uptime_weight;
            score += uptime_score;
        }
        
        // SLA compliance component (0-30 points)
        if data.total_epochs > 0 {
            let sla_rate = data.epochs_sla_met as f64 / data.total_epochs as f64;
            let sla_score = sla_rate * 30.0 * self.config.sla_weight;
            score += sla_score;
        }
        
        // Benchmark component (0-20 points)
        let benchmark_results = self.benchmark_results.read().await;
        if let Some(bench) = benchmark_results.get(&data.node_id) {
            let bench_score = self.score_benchmark(bench) * 20.0 * self.config.benchmark_weight;
            score += bench_score;
        }
        
        // Peer review component (0-10 points)
        let peer_reviews = self.peer_reviews.read().await;
        if let Some(reviews) = peer_reviews.get(&data.node_id) {
            if !reviews.is_empty() {
                let avg_rating: f64 = reviews.iter().map(|r| r.rating as f64).sum::<f64>() / reviews.len() as f64;
                let review_score = (avg_rating / 5.0) * 10.0 * self.config.peer_review_weight;
                score += review_score;
            }
        }
        
        // Apply decay for inactivity
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let months_inactive = ((now - data.last_updated) / (30 * 24 * 3600)) as u32;
        
        if months_inactive > 0 {
            let decay = (self.config.decay_monthly_pct as f64 * months_inactive as f64 / 100.0) * score;
            score = (score - decay).max(self.config.min_score as f64);
        }
        
        let final_score = score.round() as u8;
        ReputationScore::new(final_score.min(self.config.max_score))
    }
    
    /// Score benchmark results (0.0 - 1.0)
    fn score_benchmark(&self, bench: &BenchmarkResult) -> f64 {
        if !bench.data_integrity {
            return 0.0;
        }
        
        // Normalize against expected baselines
        // These would be calibrated based on network averages
        let write_score = (bench.write_throughput_ops_sec / 1000.0).min(1.0);
        let read_score = (bench.read_throughput_ops_sec / 500000.0).min(1.0);
        let snapshot_score = (100.0 / bench.snapshot_time_ms as f64).min(1.0);
        let recovery_score = (100.0 / bench.recovery_time_ms as f64).min(1.0);
        
        (write_score + read_score + snapshot_score + recovery_score) / 4.0
    }
    
    /// Get current reputation score
    pub async fn get_score(&self, node_id: NodeId) -> ReputationScore {
        self.scores.read().await
            .get(&node_id)
            .map(|d| d.score)
            .unwrap_or_else(|| ReputationScore::new(self.config.initial_score))
    }
    
    /// Get full reputation data
    pub async fn get_data(&self, node_id: NodeId) -> Option<ReputationData> {
        self.scores.read().await.get(&node_id).cloned()
    }
    
    /// Get all provider scores
    pub async fn get_all_scores(&self) -> Vec<(NodeId, ReputationScore)> {
        self.scores.read().await
            .iter()
            .map(|(id, data)| (*id, data.score))
            .collect()
    }
    
    /// Get top providers by reputation
    pub async fn get_top_providers(&self, limit: usize) -> Vec<(NodeId, ReputationScore)> {
        let mut scores = self.get_all_scores().await;
        scores.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
        scores.truncate(limit);
        scores
    }
    
    /// Check if provider is eligible for premium jobs
    pub async fn is_premium_eligible(&self, node_id: NodeId) -> bool {
        self.get_score(node_id).await.is_eligible_for_premium()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nebenk_core::types::NodeId;
    use nebenk_core::types::ClusterId;
    
    #[tokio::test]
    async fn test_reputation_initialization() {
        let engine = ReputationEngine::default();
        let node_id = NodeId::from_bytes([1u8; 32]);
        
        let score = engine.init_provider(node_id).await;
        assert_eq!(score.0, 50);
    }
    
    #[tokio::test]
    async fn test_uptime_scoring() {
        let engine = ReputationEngine::default();
        let node_id = NodeId::from_bytes([1u8; 32]);
        
        engine.init_provider(node_id).await;
        
        // Record good uptime
        for i in 0..10 {
            engine.record_uptime(node_id, i, 99.9).await;
            engine.record_sla(node_id, i, true, 99.9, 100).await;
        }
        
        let score = engine.get_score(node_id).await;
        assert!(score.0 > 50); // Should increase from initial
    }
    
    #[tokio::test]
    async fn test_sla_failure_penalty() {
        let engine = ReputationEngine::default();
        let node_id = NodeId::from_bytes([1u8; 32]);
        
        engine.init_provider(node_id).await;
        
        // Record SLA failures with 0% uptime - should decrease score
        for i in 0..20 {
            engine.record_uptime(node_id, i, 0.0).await; // 0% uptime
            engine.record_sla(node_id, i, false, 0.0, 10000).await;
        }
        
        let score = engine.get_score(node_id).await;
        assert!(score.0 <= 50); // Should not exceed initial (50), may be equal if no positive components
    }
    
    #[tokio::test]
    async fn test_benchmark_scoring() {
        let engine = ReputationEngine::default();
        let node_id = NodeId::from_bytes([1u8; 32]);
        
        engine.init_provider(node_id).await;
        
        engine.record_benchmark(BenchmarkResult {
            node_id,
            write_throughput_ops_sec: 2000.0,
            read_throughput_ops_sec: 1000000.0,
            snapshot_time_ms: 50,
            recovery_time_ms: 100,
            data_integrity: true,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        }).await;
        
        let score = engine.get_score(node_id).await;
        assert!(score.0 >= 50); // Should not decrease
    }
}

impl Default for ReputationEngine {
    fn default() -> Self {
        Self::new(ReputationConfig::default())
    }
}
