//! Developer client for marketplace deployments

use crate::types::*;
use crate::provider::ProviderHeartbeat;
use nebenk_core::types::{ClusterId, NodeId};
use nebenk_core::error::{NebenkError, Result};
use nebenk_identity::NodeIdentity;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub discovery_timeout_secs: u64,
    pub deployment_timeout_secs: u64,
    pub max_providers_to_contact: u32,
    pub escrow_buffer_epochs: u32,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            discovery_timeout_secs: 30,
            deployment_timeout_secs: 300, // 5 minutes
            max_providers_to_contact: 10,
            escrow_buffer_epochs: 2,
        }
    }
}

/// Marketplace client for developers
pub struct MarketplaceClient {
    config: ClientConfig,
    identity: Arc<NodeIdentity>,
    known_providers: Arc<RwLock<Vec<ProviderAdvertisement>>>,
    active_deployments: Arc<RwLock<HashMap<DeploymentId, ActiveDeployment>>>,
    pending_offers: Arc<RwLock<HashMap<DeploymentId, Vec<ProviderAdvertisement>>>>,
}

impl MarketplaceClient {
    /// Create a new marketplace client
    pub fn new(
        config: ClientConfig,
        identity: Arc<NodeIdentity>,
    ) -> Self {
        Self {
            config,
            identity,
            known_providers: Arc::new(RwLock::new(Vec::new())),
            active_deployments: Arc::new(RwLock::new(HashMap::new())),
            pending_offers: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Start discovery of providers (stub)
    pub async fn start_discovery(&self) -> Result<()> {
        info!("Starting provider discovery (stub)");
        Ok(())
    }
    
    /// Handle incoming provider advertisement
    pub async fn handle_advertisement(&self, adv: ProviderAdvertisement) -> Result<()> {
        // Verify signature
        if !adv.verify() {
            warn!("Invalid provider advertisement signature from {}", adv.node_id);
            return Ok(());
        }
        
        // Update known providers
        let mut providers = self.known_providers.write().await;
        if let Some(pos) = providers.iter().position(|p| p.node_id == adv.node_id) {
            providers[pos] = adv;
        } else {
            providers.push(adv);
        }
        
        Ok(())
    }
    
    /// Handle provider heartbeat
    pub async fn handle_heartbeat(&self, hb: ProviderHeartbeat) -> Result<()> {
        let mut providers = self.known_providers.write().await;
        if let Some(provider) = providers.iter_mut().find(|p| p.node_id == hb.node_id) {
            provider.last_heartbeat = hb.timestamp;
            provider.reputation = hb.reputation;
        }
        Ok(())
    }
    
    /// Discover providers matching requirements
    pub async fn discover_providers(
        &self,
        requirements: &ResourceRequirements,
        prefs: &ClusterPreferences,
    ) -> Result<Vec<ProviderAdvertisement>> {
        let providers = self.known_providers.read().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let mut matches: Vec<_> = providers.iter()
            .filter(|p| {
                // Check if provider is recent (heartbeat within 10 minutes)
                now.saturating_sub(p.last_heartbeat) < 600
            })
            .filter(|p| p.reputation.0 >= prefs.min_reputation.0)
            .filter(|p| {
                // Check capabilities match
                p.capabilities.cpu_cores >= requirements.min_cpu_cores &&
                p.capabilities.memory_gb >= requirements.min_memory_gb &&
                p.capabilities.storage_gb >= requirements.min_storage_gb &&
                (requirements.required_arch.is_empty() || requirements.required_arch.contains(&p.capabilities.cpu_arch)) &&
                requirements.required_features.iter().all(|f| p.capabilities.features.contains(f)) &&
                (!requirements.gpu_required || p.capabilities.gpu.is_some())
            })
            .cloned()
            .collect();
        
        // Sort by reputation (highest first), then by price (lowest first)
        matches.sort_by(|a, b| {
            b.reputation.0.cmp(&a.reputation.0)
                .then_with(|| a.pricing.cpu_per_hour.cmp(&b.pricing.cpu_per_hour))
        });
        
        // Limit results
        matches.truncate(self.config.max_providers_to_contact as usize);
        
        info!("Found {} matching providers", matches.len());
        Ok(matches)
    }
    
    /// Deploy application to marketplace
    pub async fn deploy(
        &self,
        manifest: DeploymentManifest,
    ) -> Result<DeploymentId> {
        info!("Deploying application: {} v{}", manifest.app_name, manifest.app_version);
        
        // Discover matching providers
        let providers = self.discover_providers(&manifest.requirements, &manifest.cluster_prefs).await?;
        
        if providers.len() < manifest.replication.min_replicas as usize {
            return Err(NebenkError::InsufficientReplicas {
                required: manifest.replication.min_replicas,
                available: providers.len() as u32,
            });
        }
        
        // Select providers (top N by reputation/price)
        let selected = &providers[..manifest.replication.min_replicas as usize];
        
        // Create cluster
        let cluster_id = ClusterId::new(format!("deploy-{}", manifest.deployment_id));
        
        // Send deployment offers to selected providers (stub)
        for provider in selected {
            self.send_deployment_offer(provider, &manifest, &cluster_id).await?;
        }
        
        // Track deployment
        let deployment = ActiveDeployment {
            deployment_id: manifest.deployment_id,
            manifest: manifest.clone(),
            status: DeploymentStatus::Pending,
            provider_node_ids: selected.iter().map(|p| p.node_id).collect(),
            cluster_id,
            started_at: None,
            current_epoch: 0,
            escrow_balance: 0,
            total_paid: 0,
        };
        
        self.active_deployments.write().await
            .insert(manifest.deployment_id, deployment);
        
        // Store pending offers for response handling
        self.pending_offers.write().await
            .insert(manifest.deployment_id, selected.to_vec());
        
        info!("Deployment {} sent to {} providers", manifest.deployment_id, selected.len());
        Ok(manifest.deployment_id)
    }
    
    /// Send deployment offer to a provider (stub)
    async fn send_deployment_offer(
        &self,
        provider: &ProviderAdvertisement,
        manifest: &DeploymentManifest,
        cluster_id: &ClusterId,
    ) -> Result<()> {
        let offer = DeploymentOffer {
            deployment_id: manifest.deployment_id,
            manifest: manifest.clone(),
            cluster_id: cluster_id.clone(),
            invitation_token: self.create_invitation_token(cluster_id).await?,
            expires_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() + 3600, // 1 hour
        };
        
        // TODO: Send via network
        debug!("Sent deployment offer to {}: {:?}", provider.node_id, offer.deployment_id);
        Ok(())
    }
    
    /// Create cluster invitation token
    async fn create_invitation_token(&self, cluster_id: &ClusterId) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let token = self.identity.create_invitation(cluster_id, now + 3600);
        Ok(token)
    }
    
    /// Handle provider acceptance of deployment
    pub async fn handle_acceptance(&self, acceptance: DeploymentAcceptance) -> Result<()> {
        let mut pending = self.pending_offers.write().await;
        
        if let Some(providers) = pending.get_mut(&acceptance.deployment_id) {
            // Update provider status
            let _ = providers.iter().position(|p| p.node_id == acceptance.provider_node_id);
        }
        
        // Check if we have enough acceptances
        let mut deployments = self.active_deployments.write().await;
        if let Some(deployment) = deployments.get_mut(&acceptance.deployment_id) {
            deployment.provider_node_ids.push(acceptance.provider_node_id);
            
            if deployment.provider_node_ids.len() >= deployment.manifest.replication.min_replicas as usize {
                deployment.status = DeploymentStatus::Provisioning;
                // TODO: Trigger actual cluster formation via NEBENK runtime
            }
        }
        
        Ok(())
    }
    
    /// Get deployment status
    pub async fn deployment_status(&self, deployment_id: DeploymentId) -> Option<DeploymentStatus> {
        self.active_deployments.read().await
            .get(&deployment_id)
            .map(|d| d.status)
    }
    
    /// List active deployments
    pub async fn list_deployments(&self) -> Vec<ActiveDeployment> {
        self.active_deployments.read().await.values().cloned().collect()
    }
    
    /// Cancel deployment
    pub async fn cancel_deployment(&self, deployment_id: DeploymentId) -> Result<()> {
        let mut deployments = self.active_deployments.write().await;
        if let Some(deployment) = deployments.get_mut(&deployment_id) {
            deployment.status = DeploymentStatus::Cancelled;
            
            // TODO: Notify providers via network
            for provider_id in &deployment.provider_node_ids {
                debug!("Notified provider {} of cancellation", provider_id);
            }
        }
        Ok(())
    }
    
    /// Get known provider count
    pub async fn provider_count(&self) -> usize {
        self.known_providers.read().await.len()
    }
}

/// Deployment offer sent to providers
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeploymentOffer {
    deployment_id: DeploymentId,
    manifest: DeploymentManifest,
    cluster_id: ClusterId,
    invitation_token: String,
    expires_at: u64,
}

/// Provider acceptance response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentAcceptance {
    pub deployment_id: DeploymentId,
    pub provider_node_id: NodeId,
    pub accepted: bool,
    pub reason: Option<String>,
}

/// Deployment cancellation
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeploymentCancellation {
    deployment_id: DeploymentId,
    reason: String,
}

impl Clone for MarketplaceClient {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            identity: self.identity.clone(),
            known_providers: self.known_providers.clone(),
            active_deployments: self.active_deployments.clone(),
            pending_offers: self.pending_offers.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nebenk_core::types::{ClusterId, NodeId};
    use nebenk_identity::NodeIdentity;
    
    #[tokio::test]
    async fn test_client_creation() {
        let identity = Arc::new(NodeIdentity::generate());
        let config = ClientConfig::default();
        
        let client = MarketplaceClient::new(config, identity);
        assert_eq!(client.provider_count().await, 0);
    }
}
