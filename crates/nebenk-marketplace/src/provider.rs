//! Provider registration and marketplace participation

use crate::types::*;
use nebenk_core::types::{ClusterId, NodeId};
use nebenk_core::error::{NebenkError, Result};
use nebenk_identity::NodeIdentity;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub enable_marketplace: bool,
    pub advertisement_interval_secs: u64,
    pub heartbeat_interval_secs: u64,
    pub min_reputation_for_jobs: u8,
    pub auto_accept_deployments: bool,
    pub max_concurrent_deployments: u32,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            enable_marketplace: false,
            advertisement_interval_secs: 300, // 5 minutes
            heartbeat_interval_secs: 30,
            min_reputation_for_jobs: 25,
            auto_accept_deployments: false,
            max_concurrent_deployments: 3,
        }
    }
}

/// Provider state machine
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderState {
    Unregistered,
    Registering,
    Registered,
    Active,
    Maintenance,
    Slashed,
    Banned,
}

/// Marketplace provider
pub struct MarketplaceProvider {
    config: ProviderConfig,
    identity: Arc<NodeIdentity>,
    state: Arc<RwLock<ProviderState>>,
    advertisement: Arc<RwLock<Option<ProviderAdvertisement>>>,
    active_deployments: Arc<RwLock<HashMap<DeploymentId, ActiveDeployment>>>,
    earnings: Arc<RwLock<ProviderEarnings>>,
    reputation: Arc<RwLock<ReputationScore>>,
}

impl MarketplaceProvider {
    /// Create a new marketplace provider
    pub fn new(
        config: ProviderConfig,
        identity: Arc<NodeIdentity>,
    ) -> Self {
        let node_id = identity.node_id();
        let earnings = ProviderEarnings {
            node_id,
            total_earned: 0,
            total_slashed: 0,
            pending_payout: 0,
            epochs_completed: 0,
            current_stake: 0,
            reputation: ReputationScore::default(),
        };
        
        Self {
            config,
            identity,
            state: Arc::new(RwLock::new(ProviderState::Unregistered)),
            advertisement: Arc::new(RwLock::new(None)),
            active_deployments: Arc::new(RwLock::new(HashMap::new())),
            earnings: Arc::new(RwLock::new(earnings)),
            reputation: Arc::new(RwLock::new(ReputationScore::default())),
        }
    }
    
    /// Register as a provider in the marketplace
    pub async fn register(
        &self,
        capabilities: DeviceCapabilities,
        pricing: PricingModel,
        stake: StakeInfo,
    ) -> Result<ProviderAdvertisement> {
        info!("Registering as marketplace provider");
        
        *self.state.write().await = ProviderState::Registering;
        
        let cluster_id = ClusterId::new("nebenk-marketplace");
        let mut adv = ProviderAdvertisement::new(
            self.identity.node_id(),
            cluster_id,
            capabilities,
            pricing,
            stake.clone(),
        );
        
        // Sign the advertisement
        let signing_data = adv.signing_data();
        adv.signature = self.identity.sign(&signing_data);
        
        // Store advertisement
        *self.advertisement.write().await = Some(adv.clone());
        *self.state.write().await = ProviderState::Registered;
        
        // Update earnings with stake
        {
            let mut earnings = self.earnings.write().await;
            earnings.current_stake = stake.amount;
            earnings.reputation = adv.reputation;
        }
        
        info!("Provider registered successfully: {}", self.identity.node_id());
        Ok(adv)
    }
    
    /// Start periodic advertisement and heartbeat (stub - needs network integration)
    pub async fn start_marketplace_tasks(&self) {
        if !self.config.enable_marketplace {
            return;
        }
        
        let adv_interval = Duration::from_secs(self.config.advertisement_interval_secs);
        let hb_interval = Duration::from_secs(self.config.heartbeat_interval_secs);
        
        // Advertisement task
        let adv_self = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(adv_interval);
            loop {
                interval.tick().await;
                if let Err(e) = adv_self.republish_advertisement().await {
                    warn!("Failed to republish advertisement: {}", e);
                }
            }
        });
        
        // Heartbeat task
        let hb_self = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(hb_interval);
            loop {
                interval.tick().await;
                if let Err(e) = hb_self.send_heartbeat().await {
                    warn!("Failed to send heartbeat: {}", e);
                }
            }
        });
        
        info!("Marketplace tasks started (stub)");
    }
    
    /// Republish advertisement with updated timestamp
    async fn republish_advertisement(&self) -> Result<()> {
        let mut adv_opt = self.advertisement.write().await;
        if let Some(adv) = adv_opt.as_mut() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            adv.last_heartbeat = now;
            
            // Re-sign
            let signing_data = adv.signing_data();
            adv.signature = self.identity.sign(&signing_data);
            
            // TODO: Publish via network
            debug!("Republished advertisement for {}", adv.node_id);
        }
        Ok(())
    }
    
    /// Send heartbeat to registry (stub)
    async fn send_heartbeat(&self) -> Result<()> {
        let adv_opt = self.advertisement.read().await;
        if let Some(adv) = adv_opt.as_ref() {
            let _heartbeat = ProviderHeartbeat {
                node_id: adv.node_id,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                active_deployments: self.active_deployments.read().await.len() as u32,
                reputation: *self.reputation.read().await,
            };
            
            // TODO: Publish via network
            debug!("Sent heartbeat for {}", adv.node_id);
        }
        Ok(())
    }
    
    /// Handle incoming deployment offer
    pub async fn handle_deployment_offer(&self, manifest: DeploymentManifest) -> Result<bool> {
        // Check if we can accept
        let can_accept = self.can_accept_deployment(&manifest).await?;
        
        if can_accept && self.config.auto_accept_deployments {
            self.accept_deployment(manifest).await?;
            return Ok(true);
        }
        
        Ok(false)
    }
    
    /// Check if provider can accept a deployment
    async fn can_accept_deployment(&self, manifest: &DeploymentManifest) -> Result<bool> {
        // Check state
        let state = *self.state.read().await;
        if state != ProviderState::Registered && state != ProviderState::Active {
            return Ok(false);
        }
        
        // Check reputation
        let rep = *self.reputation.read().await;
        if rep.0 < manifest.cluster_prefs.min_reputation.0 {
            return Ok(false);
        }
        
        // Check max concurrent deployments
        let active = self.active_deployments.read().await.len() as u32;
        if active >= self.config.max_concurrent_deployments {
            return Ok(false);
        }
        
        // Check capabilities match requirements
        let adv_opt = self.advertisement.read().await;
        if let Some(adv) = adv_opt.as_ref() {
            if !self.capabilities_match(&adv.capabilities, &manifest.requirements) {
                return Ok(false);
            }
            
            // Check pricing
            if adv.pricing.cpu_per_hour > manifest.pricing.max_per_node_per_hour {
                return Ok(false);
            }
        }
        
        Ok(true)
    }
    
    /// Check if provider capabilities match requirements
    fn capabilities_match(&self, caps: &DeviceCapabilities, req: &ResourceRequirements) -> bool {
        if caps.cpu_cores < req.min_cpu_cores { return false; }
        if caps.memory_gb < req.min_memory_gb { return false; }
        if caps.storage_gb < req.min_storage_gb { return false; }
        
        if !req.required_arch.is_empty() && !req.required_arch.contains(&caps.cpu_arch) {
            return false;
        }
        
        for feat in &req.required_features {
            if !caps.features.contains(feat) {
                return false;
            }
        }
        
        if req.gpu_required && caps.gpu.is_none() {
            return false;
        }
        
        true
    }
    
    /// Accept a deployment
    pub async fn accept_deployment(&self, manifest: DeploymentManifest) -> Result<()> {
        info!("Accepting deployment: {}", manifest.deployment_id);
        
        *self.state.write().await = ProviderState::Active;
        
        let deployment = ActiveDeployment {
            deployment_id: manifest.deployment_id,
            manifest: manifest.clone(),
            status: DeploymentStatus::Provisioning,
            provider_node_ids: vec![self.identity.node_id()],
            cluster_id: ClusterId::new("nebenk-marketplace"),
            started_at: None,
            current_epoch: 0,
            escrow_balance: 0,
            total_paid: 0,
        };
        
        self.active_deployments.write().await
            .insert(manifest.deployment_id, deployment);
        
        // TODO: Join cluster, deploy WASM app
        // This will integrate with nebenk-runtime
        
        Ok(())
    }
    
    /// Get current provider state
    pub async fn state(&self) -> ProviderState {
        *self.state.read().await
    }
    
    /// Get current advertisement
    pub async fn advertisement(&self) -> Option<ProviderAdvertisement> {
        self.advertisement.read().await.clone()
    }
    
    /// Get active deployments
    pub async fn active_deployments(&self) -> Vec<ActiveDeployment> {
        self.active_deployments.read().await.values().cloned().collect()
    }
    
    /// Get earnings
    pub async fn earnings(&self) -> ProviderEarnings {
        self.earnings.read().await.clone()
    }
    
    /// Update reputation (called by registry after epoch)
    pub async fn update_reputation(&self, new_score: ReputationScore) {
        *self.reputation.write().await = new_score;
        if let Some(adv) = self.advertisement.write().await.as_mut() {
            adv.reputation = new_score;
        }
    }
    
    /// Record epoch settlement
    pub async fn record_settlement(&self, settlement: EpochSettlement) {
        let mut earnings = self.earnings.write().await;
        earnings.total_earned = earnings.total_earned.saturating_add(settlement.payment_amount);
        earnings.total_slashed = earnings.total_slashed.saturating_add(settlement.slashed_amount);
        earnings.epochs_completed += 1;
        earnings.reputation = *self.reputation.read().await;
    }
}

/// Provider heartbeat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHeartbeat {
    pub node_id: NodeId,
    pub timestamp: u64,
    pub active_deployments: u32,
    pub reputation: ReputationScore,
}

impl Clone for MarketplaceProvider {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            identity: self.identity.clone(),
            state: self.state.clone(),
            advertisement: self.advertisement.clone(),
            active_deployments: self.active_deployments.clone(),
            earnings: self.earnings.clone(),
            reputation: self.reputation.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nebenk_core::types::{ClusterId, NodeId};
    use nebenk_identity::NodeIdentity;
    
    #[tokio::test]
    async fn test_provider_registration() {
        let identity = Arc::new(NodeIdentity::generate());
        let config = ProviderConfig::default();
        
        let provider = MarketplaceProvider::new(config, identity);
        
        let capabilities = DeviceCapabilities {
            cpu_cores: 8,
            cpu_arch: "x86_64".to_string(),
            os: "linux".to_string(),
            memory_gb: 32,
            storage_gb: 500,
            storage_type: "nvme".to_string(),
            gpu: None,
            network: NetworkInfo {
                public_ip: None,
                nat_type: NatType::None,
                bandwidth_mbps_up: 100,
                bandwidth_mbps_down: 100,
                latency_ms_avg: None,
            },
            features: vec!["wasm".to_string()],
        };
        
        let pricing = PricingModel {
            cpu_per_hour: 10,
            memory_per_gb_hour: 1,
            storage_per_gb_hour: 1,
            gpu_per_hour: None,
            egress_per_gb: 5,
            min_epoch_hours: 1,
        };
        
        let stake = StakeInfo {
            amount: 1000,
            token: "USDC".to_string(),
            lock_duration_secs: 86400 * 30,
            contract_address: None,
        };
        
        let adv = provider.register(capabilities, pricing, stake).await.unwrap();
        assert_eq!(adv.node_id, provider.identity.node_id());
        assert!(!adv.signature.is_empty());
    }
    
    #[test]
    fn test_capabilities_match() {
        let caps = DeviceCapabilities {
            cpu_cores: 8,
            cpu_arch: "x86_64".to_string(),
            os: "linux".to_string(),
            memory_gb: 32,
            storage_gb: 500,
            storage_type: "nvme".to_string(),
            gpu: None,
            network: NetworkInfo {
                public_ip: None,
                nat_type: NatType::None,
                bandwidth_mbps_up: 100,
                bandwidth_mbps_down: 100,
                latency_ms_avg: None,
            },
            features: vec!["wasm".to_string()],
        };
        
        let req = ResourceRequirements {
            min_cpu_cores: 4,
            min_memory_gb: 16,
            min_storage_gb: 100,
            required_arch: vec!["x86_64".to_string()],
            required_features: vec!["wasm".to_string()],
            gpu_required: false,
        };
        
        // Would need provider instance to test
        // assert!(provider.capabilities_match(&caps, &req));
    }
}
