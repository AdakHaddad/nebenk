//! Settlement layer: escrow, payments, slashing

use crate::types::*;
use nebenk_core::types::NodeId;
use nebenk_core::error::{NebenkError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Settlement configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementConfig {
    pub epoch_duration_secs: u64,
    pub min_uptime_percentage: f32,
    pub max_replication_lag_ms: u64,
    pub slashing: SlashingConfig,
    pub escrow: EscrowConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashingConfig {
    pub uptime_below_threshold_pct: u8,      // % of stake to slash
    pub replication_lag_exceeded_pct: u8,
    pub data_corruption_pct: u8,
    pub malicious_behavior_pct: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscrowConfig {
    pub min_deposit_epochs: u32,
    pub auto_refill_threshold_pct: u8,
    pub withdrawal_lock_epochs: u32,
}

impl Default for SettlementConfig {
    fn default() -> Self {
        Self {
            epoch_duration_secs: 3600, // 1 hour
            min_uptime_percentage: 99.0,
            max_replication_lag_ms: 5000,
            slashing: SlashingConfig {
                uptime_below_threshold_pct: 10,
                replication_lag_exceeded_pct: 20,
                data_corruption_pct: 50,
                malicious_behavior_pct: 100,
            },
            escrow: EscrowConfig {
                min_deposit_epochs: 2,
                auto_refill_threshold_pct: 20,
                withdrawal_lock_epochs: 24,
            },
        }
    }
}

/// Escrow account for a deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscrowAccount {
    pub deployment_id: DeploymentId,
    pub developer_node_id: NodeId,
    pub provider_node_ids: Vec<NodeId>,
    pub token: String,
    pub total_deposited: u64,
    pub total_paid: u64,
    pub total_slashed: u64,
    pub current_balance: u64,
    pub epochs_funded: u32,
    pub created_at: u64,
    pub last_settlement_at: Option<u64>,
    pub status: EscrowStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EscrowStatus {
    Active,
    Depleted,
    Refunding,
    Closed,
    Disputed,
}

/// Settlement engine
pub struct SettlementEngine {
    config: SettlementConfig,
    escrows: Arc<RwLock<HashMap<DeploymentId, EscrowAccount>>>,
    provider_stakes: Arc<RwLock<HashMap<NodeId, ProviderStake>>>,
    settlement_history: Arc<RwLock<Vec<EpochSettlement>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStake {
    pub node_id: NodeId,
    pub amount: u64,
    pub token: String,
    pub locked: bool,
    pub lock_expires_at: Option<u64>,
    pub total_slashed: u64,
}

impl SettlementEngine {
    pub fn new(config: SettlementConfig) -> Self {
        Self {
            config,
            escrows: Arc::new(RwLock::new(HashMap::new())),
            provider_stakes: Arc::new(RwLock::new(HashMap::new())),
            settlement_history: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Create escrow for a new deployment
    pub async fn create_escrow(
        &self,
        deployment_id: DeploymentId,
        developer_node_id: NodeId,
        provider_node_ids: Vec<NodeId>,
        token: String,
        epochs_to_fund: u32,
        estimated_cost_per_epoch: u64,
    ) -> Result<EscrowAccount> {
        let total_deposit = estimated_cost_per_epoch.saturating_mul(epochs_to_fund as u64);
        
        let escrow = EscrowAccount {
            deployment_id,
            developer_node_id,
            provider_node_ids: provider_node_ids.clone(),
            token,
            total_deposited: total_deposit,
            total_paid: 0,
            total_slashed: 0,
            current_balance: total_deposit,
            epochs_funded: epochs_to_fund,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            last_settlement_at: None,
            status: EscrowStatus::Active,
        };
        
        self.escrows.write().await.insert(deployment_id, escrow.clone());
        
        // Initialize provider stakes if not exist
        for provider_id in provider_node_ids {
            self.provider_stakes.write().await
                .entry(provider_id)
                .or_insert(ProviderStake {
                    node_id: provider_id,
                    amount: 0,
                    token: escrow.token.clone(),
                    locked: false,
                    lock_expires_at: None,
                    total_slashed: 0,
                });
        }
        
        info!("Created escrow for deployment {}: {} {}", deployment_id, total_deposit, escrow.token);
        Ok(escrow)
    }
    
    /// Deposit additional funds to escrow
    pub async fn deposit(&self, deployment_id: DeploymentId, amount: u64) -> Result<EscrowAccount> {
        let mut escrows = self.escrows.write().await;
        let escrow = escrows.get_mut(&deployment_id)
            .ok_or(NebenkError::EscrowNotFound)?;
        
        if escrow.status != EscrowStatus::Active {
            return Err(NebenkError::EscrowNotActive);
        }
        
        escrow.total_deposited = escrow.total_deposited.saturating_add(amount);
        escrow.current_balance = escrow.current_balance.saturating_add(amount);
        escrow.epochs_funded += 1; // Approximate
        
        Ok(escrow.clone())
    }
    
    /// Process epoch settlement for a deployment
    pub async fn settle_epoch(
        &self,
        deployment_id: DeploymentId,
        provider_settlements: Vec<ProviderEpochMetrics>,
    ) -> Result<Vec<EpochSettlement>> {
        let mut escrows = self.escrows.write().await;
        let escrow = escrows.get_mut(&deployment_id)
            .ok_or(NebenkError::EscrowNotFound)?;
        
        if escrow.status != EscrowStatus::Active {
            return Err(NebenkError::EscrowNotActive);
        }
        
        let epoch = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() - escrow.created_at) / self.config.epoch_duration_secs;
        
        let epoch_start = escrow.created_at + epoch * self.config.epoch_duration_secs;
        let epoch_end = epoch_start + self.config.epoch_duration_secs;
        
        let mut settlements = Vec::new();
        let mut total_paid_this_epoch = 0u64;
        
        for metrics in provider_settlements {
            // Calculate payment
            let (payment, slashed, sla_met) = self.calculate_payment(&escrow, &metrics).await?;
            
            let settlement = EpochSettlement {
                epoch,
                deployment_id,
                provider_node_id: metrics.provider_node_id,
                epoch_start,
                epoch_end,
                uptime_percentage: metrics.uptime_percentage,
                replication_lag_ms: metrics.replication_lag_ms,
                sla_met,
                payment_amount: payment,
                slashed_amount: slashed,
                settled_at: Some(SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()),
            };
            
            // Update escrow
            escrow.current_balance = escrow.current_balance.saturating_sub(payment);
            escrow.total_paid = escrow.total_paid.saturating_add(payment);
            escrow.total_slashed = escrow.total_slashed.saturating_add(slashed);
            escrow.last_settlement_at = Some(settlement.settled_at.unwrap());
            
            // Update provider stake if slashed
            if slashed > 0 {
                self.slash_provider(metrics.provider_node_id, slashed).await?;
            }
            
            total_paid_this_epoch = total_paid_this_epoch.saturating_add(payment);
            settlements.push(settlement);
        }
        
        // Check if escrow depleted
        if escrow.current_balance == 0 {
            escrow.status = EscrowStatus::Depleted;
            warn!("Escrow depleted for deployment {}", deployment_id);
        }
        
        // Store in history
        self.settlement_history.write().await.extend(settlements.clone());
        
        info!("Settled epoch {} for deployment {}: {} providers, {} paid", 
              epoch, deployment_id, settlements.len(), total_paid_this_epoch);
        
        Ok(settlements)
    }
    
    /// Calculate payment for a provider based on metrics
    async fn calculate_payment(
        &self,
        escrow: &EscrowAccount,
        metrics: &ProviderEpochMetrics,
    ) -> Result<(u64, u64, bool)> {
        let base_payment = self.estimate_epoch_cost(escrow);
        let mut payment = base_payment;
        let mut slashed = 0u64;
        let mut sla_met = true;
        
        // Check uptime
        if metrics.uptime_percentage < self.config.min_uptime_percentage {
            sla_met = false;
            let slash_pct = self.config.slashing.uptime_below_threshold_pct as u64;
            slashed = slashed.saturating_add(
                base_payment.saturating_mul(slash_pct) / 100
            );
            payment = payment.saturating_sub(slashed);
        }
        
        // Check replication lag
        if metrics.replication_lag_ms > self.config.max_replication_lag_ms {
            sla_met = false;
            let slash_pct = self.config.slashing.replication_lag_exceeded_pct as u64;
            let additional_slash = base_payment.saturating_mul(slash_pct) / 100;
            slashed = slashed.saturating_add(additional_slash);
            payment = payment.saturating_sub(additional_slash);
        }
        
        // Check data corruption
        if metrics.data_corrupted {
            sla_met = false;
            let slash_pct = self.config.slashing.data_corruption_pct as u64;
            let additional_slash = base_payment.saturating_mul(slash_pct) / 100;
            slashed = slashed.saturating_add(additional_slash);
            payment = payment.saturating_sub(additional_slash);
        }
        
        // Check malicious behavior
        if metrics.malicious_detected {
            sla_met = false;
            let slash_pct = self.config.slashing.malicious_behavior_pct as u64;
            let additional_slash = base_payment.saturating_mul(slash_pct) / 100;
            slashed = slashed.saturating_add(additional_slash);
            payment = 0; // Full slash
        }
        
        // Ensure payment doesn't exceed escrow balance
        payment = payment.min(escrow.current_balance);
        
        Ok((payment, slashed, sla_met))
    }
    
    /// Estimate cost per epoch based on deployment pricing
    fn estimate_epoch_cost(&self, escrow: &EscrowAccount) -> u64 {
        if escrow.epochs_funded == 0 {
            return 0;
        }
        escrow.total_deposited / escrow.epochs_funded as u64
    }
    
    /// Slash provider stake
    async fn slash_provider(&self, node_id: NodeId, amount: u64) -> Result<()> {
        let mut stakes = self.provider_stakes.write().await;
        if let Some(stake) = stakes.get_mut(&node_id) {
            stake.amount = stake.amount.saturating_sub(amount);
            stake.total_slashed = stake.total_slashed.saturating_add(amount);
            
            warn!("Slashed provider {}: {} tokens (remaining: {})", node_id, amount, stake.amount);
        }
        Ok(())
    }
    
    /// Withdraw earnings for provider
    pub async fn withdraw_earnings(&self, node_id: NodeId, amount: u64) -> Result<u64> {
        let mut stakes = self.provider_stakes.write().await;
        let stake = stakes.get_mut(&node_id)
            .ok_or(NebenkError::ProviderNotFound)?;
        
        // Check lock
        if stake.locked {
            if let Some(expires) = stake.lock_expires_at {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if now < expires {
                    return Err(NebenkError::StakeLocked);
                }
            }
        }
        
        let available = stake.amount;
        let withdraw = amount.min(available);
        
        stake.amount = stake.amount.saturating_sub(withdraw);
        
        info!("Provider {} withdrew {} tokens", node_id, withdraw);
        Ok(withdraw)
    }
    
    /// Get escrow account
    pub async fn get_escrow(&self, deployment_id: DeploymentId) -> Option<EscrowAccount> {
        self.escrows.read().await.get(&deployment_id).cloned()
    }
    
    /// Get provider stake
    pub async fn get_stake(&self, node_id: NodeId) -> Option<ProviderStake> {
        self.provider_stakes.read().await.get(&node_id).cloned()
    }
    
    /// Get settlement history for deployment
    pub async fn get_history(&self, deployment_id: DeploymentId) -> Vec<EpochSettlement> {
        self.settlement_history.read().await
            .iter()
            .filter(|s| s.deployment_id == deployment_id)
            .cloned()
            .collect()
    }
    
    /// Get all settlement history
    pub async fn get_all_history(&self) -> Vec<EpochSettlement> {
        self.settlement_history.read().await.clone()
    }
    
    /// Close escrow and refund remaining balance
    pub async fn close_escrow(&self, deployment_id: DeploymentId) -> Result<EscrowAccount> {
        let mut escrows = self.escrows.write().await;
        let mut escrow = escrows.remove(&deployment_id)
            .ok_or(NebenkError::EscrowNotFound)?;
        
        escrow.status = EscrowStatus::Closed;
        
        // Refund remaining balance to developer (in real impl, would transfer)
        let refund = escrow.current_balance;
        escrow.current_balance = 0;
        
        info!("Closed escrow for deployment {}, refund: {}", deployment_id, refund);
        Ok(escrow)
    }
}

/// Provider metrics for epoch settlement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEpochMetrics {
    pub provider_node_id: NodeId,
    pub uptime_percentage: f32,
    pub replication_lag_ms: u64,
    pub data_corrupted: bool,
    pub malicious_detected: bool,
    pub operations_processed: u64,
    pub bytes_transferred: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DeploymentId;
    use nebenk_core::types::NodeId;
    
    #[tokio::test]
    async fn test_escrow_creation() {
        let engine = SettlementEngine::default();
        
        let deployment_id = DeploymentId::new();
        let developer = NodeId::from_bytes([1u8; 32]);
        let providers = vec![NodeId::from_bytes([2u8; 32]), NodeId::from_bytes([3u8; 32])];
        
        let escrow = engine.create_escrow(
            deployment_id,
            developer,
            providers,
            "USDC".to_string(),
            10, // 10 epochs
            100, // 100 tokens per epoch
        ).await.unwrap();
        
        assert_eq!(escrow.total_deposited, 1000);
        assert_eq!(escrow.current_balance, 1000);
        assert_eq!(escrow.epochs_funded, 10);
        assert_eq!(escrow.status, EscrowStatus::Active);
    }
    
    #[tokio::test]
    async fn test_settlement_calculation() {
        let engine = SettlementEngine::default();
        
        let deployment_id = DeploymentId::new();
        let developer = NodeId::from_bytes([1u8; 32]);
        let providers = vec![NodeId::from_bytes([2u8; 32])];
        
        engine.create_escrow(
            deployment_id,
            developer,
            providers,
            "USDC".to_string(),
            10,
            100,
        ).await.unwrap();
        
        // Good metrics - full payment
        let metrics = vec![ProviderEpochMetrics {
            provider_node_id: NodeId::from_bytes([2u8; 32]),
            uptime_percentage: 99.5,
            replication_lag_ms: 100,
            data_corrupted: false,
            malicious_detected: false,
            operations_processed: 1000,
            bytes_transferred: 1024,
        }];
        
        let settlements = engine.settle_epoch(deployment_id, metrics).await.unwrap();
        assert_eq!(settlements.len(), 1);
        assert!(settlements[0].sla_met);
        assert_eq!(settlements[0].payment_amount, 100);
        assert_eq!(settlements[0].slashed_amount, 0);
        
        // Poor uptime - slashed
        let metrics2 = vec![ProviderEpochMetrics {
            provider_node_id: NodeId::from_bytes([2u8; 32]),
            uptime_percentage: 95.0, // Below 99%
            replication_lag_ms: 100,
            data_corrupted: false,
            malicious_detected: false,
            operations_processed: 1000,
            bytes_transferred: 1024,
        }];
        
        let settlements2 = engine.settle_epoch(deployment_id, metrics2).await.unwrap();
        assert!(!settlements2[0].sla_met);
        assert_eq!(settlements2[0].slashed_amount, 10); // 10% of 100
        assert_eq!(settlements2[0].payment_amount, 90);
    }
    
    #[tokio::test]
    async fn test_stake_slashing() {
        let engine = SettlementEngine::default();
        
        let provider = NodeId::from_bytes([2u8; 32]);
        
        // Initialize stake
        engine.provider_stakes.write().await.insert(provider, ProviderStake {
            node_id: provider,
            amount: 1000,
            token: "USDC".to_string(),
            locked: true,
            lock_expires_at: None,
            total_slashed: 0,
        });
        
        // Slash
        engine.slash_provider(provider, 100).await.unwrap();
        
        let stake = engine.get_stake(provider).await.unwrap();
        assert_eq!(stake.amount, 900);
        assert_eq!(stake.total_slashed, 100);
    }
}

impl Default for SettlementEngine {
    fn default() -> Self {
        Self::new(SettlementConfig::default())
    }
}
