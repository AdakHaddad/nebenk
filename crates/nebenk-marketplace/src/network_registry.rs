//! Network-backed marketplace registry using libp2p Gossipsub + Kademlia

use crate::types::*;
use crate::provider::ProviderHeartbeat;
use crate::registry::{ProviderFilter, ProviderListRequest, ProviderListResponse, RegistryConfig};
use nebenk_core::types::{ClusterId, NodeId};
use nebenk_core::error::{NebenkError, Result};
use nebenk_identity::NodeIdentity;
use nebenk_network::marketplace::{
    MarketplaceCommand, MarketplaceEvent, MarketplaceNetwork,
};
use nebenk_network::protocol::{ProviderListRequestMsg, ProviderListResponseMsg, ProviderFilterMsg};
use libp2p::Multiaddr;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// Network-backed registry that uses libp2p for provider discovery
pub struct NetworkMarketplaceRegistry {
    /// Local in-memory cache of providers (synced from network)
    providers: Arc<RwLock<HashMap<NodeId, ProviderAdvertisement>>>,
    /// Local heartbeat cache
    heartbeats: Arc<RwLock<HashMap<NodeId, ProviderHeartbeat>>>,
    /// Configuration
    config: RegistryConfig,
    /// Command channel to network layer
    network_cmd_tx: mpsc::UnboundedSender<MarketplaceCommand>,
    /// Event receiver from network layer
    network_evt_rx: Option<mpsc::UnboundedReceiver<MarketplaceEvent>>,
    /// Local node identity
    local_node_id: NodeId,
}

impl NetworkMarketplaceRegistry {
    /// Create a new network-backed registry
    pub async fn new(
        identity: &NodeIdentity,
        config: RegistryConfig,
        listen_addr: Multiaddr,
    ) -> Result<(Self, MarketplaceNetwork)> {
        let local_node_id = identity.node_id();
        let keypair = identity.to_libp2p_keypair();
        
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (evt_tx, evt_rx) = mpsc::unbounded_channel();
        
        let network = MarketplaceNetwork::new(keypair, cmd_rx, evt_tx)
            .map_err(|e| NebenkError::Network(e.to_string()))?;
        
        let mut registry = Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            heartbeats: Arc::new(RwLock::new(HashMap::new())),
            config,
            network_cmd_tx: cmd_tx,
            network_evt_rx: Some(evt_rx),
            local_node_id,
        };
        
        // Register self as provider if configured
        // (This would be called by the provider after registration)
        
        Ok((registry, network))
    }
    
    /// Create registry with existing network (for CLI integration)
    pub fn with_network(
        local_node_id: NodeId,
        config: RegistryConfig,
        network_cmd_tx: mpsc::UnboundedSender<MarketplaceCommand>,
        network_evt_rx: mpsc::UnboundedReceiver<MarketplaceEvent>,
    ) -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            heartbeats: Arc::new(RwLock::new(HashMap::new())),
            config,
            network_cmd_tx,
            network_evt_rx: Some(network_evt_rx),
            local_node_id,
        }
    }
    
    /// Start the event processing loop
    pub async fn start_event_loop(&mut self) {
        let mut evt_rx = self.network_evt_rx.take().expect("Event receiver already taken");
        let providers = self.providers.clone();
        let heartbeats = self.heartbeats.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            while let Some(event) = evt_rx.recv().await {
                match event {
                    MarketplaceEvent::ProviderAdvertisement(adv_msg) => {
                        debug!("Received provider advertisement from {}", adv_msg.node_id);
                        
                        // Convert to internal type
                        let adv = ProviderAdvertisement::from_msg(adv_msg);
                        
                        // Verify signature
                        if !adv.verify() {
                            warn!("Invalid provider advertisement signature from {}", adv.node_id);
                            continue;
                        }
                        
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        
                        // Check TTL
                        if now.saturating_sub(adv.last_heartbeat) > config.provider_ttl_secs {
                            debug!("Stale advertisement from {}", adv.node_id);
                            continue;
                        }
                        
                        // Store in local cache
                        providers.write().await.insert(adv.node_id, adv);
                    }
                    
                    MarketplaceEvent::ProviderHeartbeat(hb_msg) => {
                        debug!("Received heartbeat from {}", hb_msg.node_id);
                        
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        
                        // Check TTL
                        if now.saturating_sub(hb_msg.timestamp) > config.heartbeat_ttl_secs {
                            continue;
                        }
                        
                        // Convert to internal type
                        let hb = ProviderHeartbeat::from_msg(hb_msg);
                        
                        // Update heartbeat cache
                        heartbeats.write().await.insert(hb.node_id, hb.clone());
                        
                        // Update provider if exists
                        if let Some(provider) = providers.write().await.get_mut(&hb.node_id) {
                            provider.last_heartbeat = hb.timestamp;
                            provider.reputation = hb.reputation;
                        }
                    }
                    
                    MarketplaceEvent::ProviderListRequest { request, responder } => {
                        debug!("Received provider list request from {}", request.requester);
                        // This would be handled by a node acting as a registry/indexer
                        // For now, respond from local cache
                        let providers_cache = providers.read().await;
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        
                        // Convert filter from Msg to internal type
                        let filter = request.filter.map(|f| ProviderFilter {
                            min_cpu_cores: f.min_cpu_cores,
                            min_memory_gb: f.min_memory_gb,
                            min_storage_gb: f.min_storage_gb,
                            required_arch: f.required_arch,
                            required_features: f.required_features,
                            min_reputation: f.min_reputation,
                            max_price_cpu: f.max_price_cpu,
                        });
                        
                        let mut filtered: Vec<_> = providers_cache.values()
                            .filter(|p| now.saturating_sub(p.last_heartbeat) < config.provider_ttl_secs)
                            .filter(|p| Self::matches_filter(p, filter.as_ref()))
                            .cloned()
                            .collect();
                        
                        filtered.sort_by(|a, b| {
                            b.reputation.0.cmp(&a.reputation.0)
                                .then_with(|| a.pricing.cpu_per_hour.cmp(&b.pricing.cpu_per_hour))
                        });
                        
                        filtered.truncate(config.max_providers_per_response);
                        
                        let response = ProviderListResponse {
                            providers: filtered,
                            timestamp: now,
                        };
                        
                        // Convert response to Msg type for network
                        let response_msg = ProviderListResponseMsg {
                            providers: response.providers.iter().map(|p| p.to_msg()).collect(),
                            timestamp: response.timestamp,
                        };
                        
                        let _ = responder.send(response_msg);
                    }
                    
                    MarketplaceEvent::DeploymentOffer(offer) => {
                        debug!("Received deployment offer for {:?}", offer.deployment_id);
                        // Would be handled by provider
                    }
                    
                    MarketplaceEvent::DeploymentAcceptance(acceptance) => {
                        debug!("Received deployment acceptance for {:?}", acceptance.deployment_id);
                        // Would be handled by client
                    }
                }
            }
        });
    }
    
    fn matches_filter(provider: &ProviderAdvertisement, filter: Option<&ProviderFilter>) -> bool {
        if let Some(filter) = filter {
            if let Some(min_cpu) = filter.min_cpu_cores {
                if provider.capabilities.cpu_cores < min_cpu { return false; }
            }
            if let Some(min_mem) = filter.min_memory_gb {
                if provider.capabilities.memory_gb < min_mem { return false; }
            }
            if let Some(min_storage) = filter.min_storage_gb {
                if provider.capabilities.storage_gb < min_storage { return false; }
            }
            if let Some(arch) = &filter.required_arch {
                if !arch.contains(&provider.capabilities.cpu_arch) { return false; }
            }
            if let Some(features) = &filter.required_features {
                if !features.iter().all(|f| provider.capabilities.features.contains(f)) { return false; }
            }
            if let Some(min_rep) = filter.min_reputation {
                if provider.reputation.0 < min_rep { return false; }
            }
            if let Some(max_price) = filter.max_price_cpu {
                if provider.pricing.cpu_per_hour > max_price { return false; }
            }
        }
        true
    }
    
    /// Publish provider advertisement to network
    pub async fn publish_advertisement(&self, advertisement: ProviderAdvertisement) -> Result<()> {
        self.network_cmd_tx
            .send(MarketplaceCommand::PublishAdvertisement(advertisement.to_msg()))
            .map_err(|_| NebenkError::Network("Network command channel closed".into()))?;
        Ok(())
    }
    
    /// Publish heartbeat to network
    pub async fn publish_heartbeat(&self, heartbeat: ProviderHeartbeat) -> Result<()> {
        self.network_cmd_tx
            .send(MarketplaceCommand::PublishHeartbeat(heartbeat.to_msg()))
            .map_err(|_| NebenkError::Network("Network command channel closed".into()))?;
        Ok(())
    }
    
    /// Request provider list via DHT
    pub async fn request_provider_list(
        &self,
        request: ProviderListRequest,
    ) -> Result<ProviderListResponse> {
        let (responder_tx, mut responder_rx) = mpsc::unbounded_channel();
        
        // Convert request to Msg type
        let request_msg = ProviderListRequestMsg {
            requester: request.requester,
            timestamp: request.timestamp,
            filter: request.filter.map(|f| ProviderFilterMsg {
                min_cpu_cores: f.min_cpu_cores,
                min_memory_gb: f.min_memory_gb,
                min_storage_gb: f.min_storage_gb,
                required_arch: f.required_arch,
                required_features: f.required_features,
                min_reputation: f.min_reputation,
                max_price_cpu: f.max_price_cpu,
            }),
        };
        
        self.network_cmd_tx
            .send(MarketplaceCommand::RequestProviderList { request: request_msg, responder: responder_tx })
            .map_err(|_| NebenkError::Network("Network command channel closed".into()))?;
        
        // Wait for response with timeout
        match tokio::time::timeout(Duration::from_secs(10), responder_rx.recv()).await {
            Ok(Some(response_msg)) => {
                // Convert response from Msg type to internal type
                let response = ProviderListResponse {
                    providers: response_msg.providers.into_iter()
                        .map(ProviderAdvertisement::from_msg)
                        .collect(),
                    timestamp: response_msg.timestamp,
                };
                Ok(response)
            }
            Ok(None) => Err(NebenkError::Network("Responder dropped".into())),
            Err(_) => Err(NebenkError::Network("Provider list request timed out".into())),
        }
    }
    
    /// Get providers from local cache (synced from network)
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
    
    /// Get local node ID
    pub fn local_node_id(&self) -> NodeId {
        self.local_node_id
    }
    
    /// Start periodic heartbeat publishing for registered provider
    pub async fn start_heartbeat_task(&self, advertisement: ProviderAdvertisement, interval_secs: u64) {
        let network_cmd_tx = self.network_cmd_tx.clone();
        let node_id = advertisement.node_id;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                
                let heartbeat = ProviderHeartbeat {
                    node_id,
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    active_deployments: 0, // Would track actual deployments
                    reputation: advertisement.reputation,
                };
                
                if network_cmd_tx
                    .send(MarketplaceCommand::PublishHeartbeat(heartbeat.to_msg()))
                    .is_err()
                {
                    break; // Network closed
                }
            }
        });
    }
    
    /// Clean up expired entries from local cache
    pub async fn cleanup_expired(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let mut providers = self.providers.write().await;
        let before = providers.len();
        providers.retain(|_, p| now.saturating_sub(p.last_heartbeat) < self.config.provider_ttl_secs);
        let after = providers.len();
        
        if before != after {
            info!("Cleaned up {} expired providers from local cache", before - after);
        }
        
        let mut heartbeats = self.heartbeats.write().await;
        heartbeats.retain(|_, h| now.saturating_sub(h.timestamp) < self.config.heartbeat_ttl_secs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use crate::registry::RegistryConfig;
    use nebenk_core::types::{ClusterId, NodeId};
    use nebenk_identity::NodeIdentity;
    use tokio::sync::mpsc;
    use libp2p::Multiaddr;
    
    #[tokio::test]
    async fn test_network_registry_creation() {
        let identity = NodeIdentity::generate();
        let config = RegistryConfig::default();
        let listen_addr: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
        
        let (registry, _network) = NetworkMarketplaceRegistry::new(&identity, config, listen_addr).await.unwrap();
        
        assert_eq!(registry.local_node_id(), identity.node_id());
    }
    
    #[tokio::test]
    async fn test_matches_filter() {
        let provider = ProviderAdvertisement {
            node_id: NodeId::from_bytes([1u8; 32]),
            cluster_id: ClusterId::new("test"),
            capabilities: DeviceCapabilities {
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
                features: vec!["wasm".to_string(), "gpu".to_string()],
            },
            pricing: PricingModel {
                cpu_per_hour: 10,
                memory_per_gb_hour: 1,
                storage_per_gb_hour: 1,
                gpu_per_hour: None,
                egress_per_gb: 5,
                min_epoch_hours: 1,
            },
            stake: StakeInfo {
                amount: 1000,
                token: "USDC".to_string(),
                lock_duration_secs: 86400 * 30,
                contract_address: None,
            },
            reputation: ReputationScore(80),
            registered_at: 0,
            last_heartbeat: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            signature: vec![],
        };
        
        // Test matching filter
        let filter = ProviderFilter {
            min_cpu_cores: Some(4),
            min_memory_gb: Some(16),
            required_arch: Some(vec!["x86_64".to_string()]),
            required_features: Some(vec!["wasm".to_string()]),
            min_reputation: Some(50),
            max_price_cpu: Some(20),
            ..Default::default()
        };
        assert!(NetworkMarketplaceRegistry::matches_filter(&provider, Some(&filter)));
        
        // Test non-matching filter (too high min_cpu)
        let filter = ProviderFilter {
            min_cpu_cores: Some(16),
            ..Default::default()
        };
        assert!(!NetworkMarketplaceRegistry::matches_filter(&provider, Some(&filter)));
        
        // Test non-matching filter (too low max_price)
        let filter = ProviderFilter {
            max_price_cpu: Some(5),
            ..Default::default()
        };
        assert!(!NetworkMarketplaceRegistry::matches_filter(&provider, Some(&filter)));
        
        // Test no filter (matches all)
        assert!(NetworkMarketplaceRegistry::matches_filter(&provider, None));
    }
}
