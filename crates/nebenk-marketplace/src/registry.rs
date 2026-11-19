//! Marketplace registry - in-memory provider registry

use crate::types::*;
use crate::provider::ProviderHeartbeat;
use nebenk_core::types::{ClusterId, NodeId};
use nebenk_core::error::{NebenkError, Result};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{info};

/// Registry topics (for future libp2p integration)
pub mod topics {
    pub const PROVIDERS: &str = "nebenk/marketplace/providers";
    pub const HEARTBEATS: &str = "nebenk/marketplace/heartbeats";
    pub const PROVIDER_LIST_REQUEST: &str = "nebenk/marketplace/provider_list_request";
    pub const PROVIDER_LIST_RESPONSE: &str = "nebenk/marketplace/provider_list_response";
    pub const DEPLOYMENT_OFFER: &str = "nebenk/marketplace/deployment_offer";
    pub const DEPLOYMENT_ACCEPTANCE: &str = "nebenk/marketplace/deployment_acceptance";
}

/// Provider list request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderListRequest {
    pub requester: NodeId,
    pub timestamp: u64,
    pub filter: Option<ProviderFilter>,
}

/// Provider filter for discovery
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderFilter {
    pub min_cpu_cores: Option<u32>,
    pub min_memory_gb: Option<u32>,
    pub min_storage_gb: Option<u32>,
    pub required_arch: Option<Vec<String>>,
    pub required_features: Option<Vec<String>>,
    pub min_reputation: Option<u8>,
    pub max_price_cpu: Option<u64>,
}

/// Provider list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderListResponse {
    pub providers: Vec<ProviderAdvertisement>,
    pub timestamp: u64,
}

/// Deployment offer (from client to provider)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentOffer {
    pub deployment_id: DeploymentId,
    pub manifest: DeploymentManifest,
    pub cluster_id: ClusterId,
    pub invitation_token: String,
    pub expires_at: u64,
}

/// Provider acceptance response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentAcceptance {
    pub deployment_id: DeploymentId,
    pub provider_node_id: NodeId,
    pub accepted: bool,
    pub reason: Option<String>,
    pub timestamp: u64,
}

/// Registry service for managing provider discovery
pub struct MarketplaceRegistry {
    providers: Arc<RwLock<HashMap<NodeId, ProviderAdvertisement>>>,
    heartbeats: Arc<RwLock<HashMap<NodeId, ProviderHeartbeat>>>,
    config: RegistryConfig,
}

#[derive(Debug, Clone)]
pub struct RegistryConfig {
    pub provider_ttl_secs: u64,
    pub heartbeat_ttl_secs: u64,
    pub max_providers_per_response: usize,
    pub cleanup_interval_secs: u64,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            provider_ttl_secs: 600,       // 10 minutes
            heartbeat_ttl_secs: 120,      // 2 minutes
            max_providers_per_response: 50,
            cleanup_interval_secs: 60,    // 1 minute
        }
    }
}

impl MarketplaceRegistry {
    pub fn new(config: RegistryConfig) -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            heartbeats: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }
    
    /// Handle incoming provider advertisement
    pub async fn handle_advertisement(&self, adv: ProviderAdvertisement) -> Result<()> {
        // Verify signature
        if !adv.verify() {
            return Err(NebenkError::InvalidSignature);
        }
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Check if advertisement is recent
        if now.saturating_sub(adv.last_heartbeat) > self.config.provider_ttl_secs {
            return Err(NebenkError::StaleAdvertisement);
        }
        
        // Store provider
        self.providers.write().await.insert(adv.node_id, adv.clone());
        
        info!("Registered provider: {} (total: {})", adv.node_id, self.providers.read().await.len());
        Ok(())
    }
    
    /// Handle provider heartbeat
    pub async fn handle_heartbeat(&self, hb: ProviderHeartbeat) -> Result<()> {
        let _now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Update heartbeat
        self.heartbeats.write().await.insert(hb.node_id, hb.clone());
        
        // Update provider reputation if exists
        if let Some(provider) = self.providers.write().await.get_mut(&hb.node_id) {
            provider.last_heartbeat = hb.timestamp;
            provider.reputation = hb.reputation;
        }
        
        Ok(())
    }
    
    /// Handle provider list request
    pub async fn handle_list_request(&self, request: ProviderListRequest) -> Result<ProviderListResponse> {
        let providers = self.providers.read().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let mut filtered: Vec<_> = providers.values()
            .filter(|p| {
                // Check TTL
                now.saturating_sub(p.last_heartbeat) < self.config.provider_ttl_secs
            })
            .filter(|p| {
                // Apply filters
                if let Some(filter) = &request.filter {
                    if let Some(min_cpu) = filter.min_cpu_cores {
                        if p.capabilities.cpu_cores < min_cpu { return false; }
                    }
                    if let Some(min_mem) = filter.min_memory_gb {
                        if p.capabilities.memory_gb < min_mem { return false; }
                    }
                    if let Some(min_storage) = filter.min_storage_gb {
                        if p.capabilities.storage_gb < min_storage { return false; }
                    }
                    if let Some(arch) = &filter.required_arch {
                        if !arch.contains(&p.capabilities.cpu_arch) { return false; }
                    }
                    if let Some(features) = &filter.required_features {
                        if !features.iter().all(|f| p.capabilities.features.contains(f)) { return false; }
                    }
                    if let Some(min_rep) = filter.min_reputation {
                        if p.reputation.0 < min_rep { return false; }
                    }
                    if let Some(max_price) = filter.max_price_cpu {
                        if p.pricing.cpu_per_hour > max_price { return false; }
                    }
                }
                true
            })
            .cloned()
            .collect();
        
        // Sort by reputation desc, then price asc
        filtered.sort_by(|a, b| {
            b.reputation.0.cmp(&a.reputation.0)
                .then_with(|| a.pricing.cpu_per_hour.cmp(&b.pricing.cpu_per_hour))
        });
        
        // Limit results
        filtered.truncate(self.config.max_providers_per_response);
        
        Ok(ProviderListResponse {
            providers: filtered,
            timestamp: now,
        })
    }
    
    /// Get all active providers
    pub async fn get_providers(&self) -> Vec<ProviderAdvertisement> {
        let providers = self.providers.read().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        providers.values()
            .filter(|p| now.saturating_sub(p.last_heartbeat) < self.config.provider_ttl_secs)
            .cloned()
            .collect()
    }
    
    /// Get provider by NodeId
    pub async fn get_provider(&self, node_id: NodeId) -> Option<ProviderAdvertisement> {
        let providers = self.providers.read().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        providers.get(&node_id)
            .filter(|p| now.saturating_sub(p.last_heartbeat) < self.config.provider_ttl_secs)
            .cloned()
    }
    
    /// Remove expired providers
    pub async fn cleanup_expired(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Clean providers
        let mut providers = self.providers.write().await;
        let before = providers.len();
        providers.retain(|_, p| now.saturating_sub(p.last_heartbeat) < self.config.provider_ttl_secs);
        let after = providers.len();
        
        if before != after {
            info!("Cleaned up {} expired providers", before - after);
        }
        
        // Clean heartbeats
        let mut heartbeats = self.heartbeats.write().await;
        heartbeats.retain(|_, h| now.saturating_sub(h.timestamp) < self.config.heartbeat_ttl_secs);
    }
    
    /// Start periodic cleanup
    pub async fn start_cleanup_task(&self) {
        let registry = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(registry.config.cleanup_interval_secs));
            loop {
                interval.tick().await;
                registry.cleanup_expired().await;
            }
        });
    }
    
    /// Get provider count
    pub async fn provider_count(&self) -> usize {
        self.providers.read().await.len()
    }
}

impl Clone for MarketplaceRegistry {
    fn clone(&self) -> Self {
        Self {
            providers: self.providers.clone(),
            heartbeats: self.heartbeats.clone(),
            config: self.config.clone(),
        }
    }
}

impl Default for MarketplaceRegistry {
    fn default() -> Self {
        Self::new(RegistryConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use nebenk_core::types::{ClusterId, NodeId};
    use nebenk_identity::NodeIdentity;
    
    
    #[tokio::test]
    async fn test_registry_advertisement() {
        let registry = MarketplaceRegistry::default();
        
        let node_id = NodeId::from_bytes([1u8; 32]);
        let cluster_id = ClusterId::new("test");
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
        
        let mut adv = ProviderAdvertisement::new(node_id, cluster_id, capabilities, pricing, stake);
        
        // Sign with identity matching the node_id
        let identity = NodeIdentity::load_or_create("/tmp/test_key_1").unwrap_or_else(|_| NodeIdentity::generate());
        // Override node_id to match the generated identity
        adv.node_id = identity.node_id();
        let signing_data = adv.signing_data();
        adv.signature = identity.sign(&signing_data);
        
        // Register
        registry.handle_advertisement(adv.clone()).await.unwrap();
        
        // Retrieve
        let retrieved = registry.get_provider(identity.node_id()).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().node_id, identity.node_id());
    }
    
    #[tokio::test]
    async fn test_registry_filter() {
        let registry = MarketplaceRegistry::default();
        
        // Add multiple providers with matching identities
        for i in 0..5 {
            let identity = NodeIdentity::generate();
            let node_id = identity.node_id();
            let cluster_id = ClusterId::new("test");
            let capabilities = DeviceCapabilities {
                cpu_cores: (i + 1) * 2,
                cpu_arch: "x86_64".to_string(),
                os: "linux".to_string(),
                memory_gb: (i + 1) * 8,
                storage_gb: 100,
                storage_type: "ssd".to_string(),
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
                cpu_per_hour: 10 + i as u64 * 2,
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
            
            let mut adv = ProviderAdvertisement::new(node_id, cluster_id, capabilities, pricing, stake);
            let signing_data = adv.signing_data();
            adv.signature = identity.sign(&signing_data);
            
            registry.handle_advertisement(adv).await.unwrap();
        }
        
        // Query with filter
        let request = ProviderListRequest {
            requester: NodeId::from_bytes([99u8; 32]),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            filter: Some(ProviderFilter {
                min_cpu_cores: Some(6),
                min_memory_gb: Some(16),
                required_arch: Some(vec!["x86_64".to_string()]),
                min_reputation: Some(25),
                max_price_cpu: Some(15),
                ..Default::default()
            }),
        };
        
        let response = registry.handle_list_request(request).await.unwrap();
        
        // Should match providers with cpu_cores >= 6 and price <= 15
        // Provider 3 (6 cores, price 14), Provider 4 (8 cores, price 16 - too expensive)
        assert!(response.providers.len() <= 2);
    }
}
