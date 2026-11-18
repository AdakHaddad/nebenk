//! NEBENK Marketplace - Decentralized device marketplace for distributed applications

pub mod types;
pub mod provider;
pub mod client;
pub mod registry;
pub mod settlement;
pub mod reputation;

// Re-export key types
pub use types::*;
pub use provider::{MarketplaceProvider, ProviderConfig, ProviderState};
pub use client::{MarketplaceClient, ClientConfig};
pub use registry::{MarketplaceRegistry, RegistryConfig, topics, ProviderListRequest, ProviderFilter, ProviderListResponse, DeploymentOffer, DeploymentAcceptance};
pub use settlement::{SettlementEngine, SettlementConfig, EscrowAccount, ProviderStake, ProviderEpochMetrics};
pub use reputation::{ReputationEngine, ReputationConfig, BenchmarkResult, PeerReview};

use nebenk_core::error::Result;

/// Marketplace integration with NEBENK runtime
pub struct MarketplaceIntegration {
    provider: Option<MarketplaceProvider>,
    client: Option<MarketplaceClient>,
    registry: MarketplaceRegistry,
    settlement: SettlementEngine,
    reputation: ReputationEngine,
}

impl MarketplaceIntegration {
    pub fn new() -> Self {
        Self {
            provider: None,
            client: None,
            registry: MarketplaceRegistry::default(),
            settlement: SettlementEngine::default(),
            reputation: ReputationEngine::default(),
        }
    }
    
    /// Enable provider mode
    pub fn enable_provider(
        &mut self,
        config: ProviderConfig,
        identity: std::sync::Arc<nebenk_identity::NodeIdentity>,
    ) {
        self.provider = Some(MarketplaceProvider::new(config, identity));
    }
    
    /// Enable client mode
    pub fn enable_client(
        &mut self,
        config: ClientConfig,
        identity: std::sync::Arc<nebenk_identity::NodeIdentity>,
    ) {
        self.client = Some(MarketplaceClient::new(config, identity));
    }
    
    /// Get provider instance
    pub fn provider(&self) -> Option<&MarketplaceProvider> {
        self.provider.as_ref()
    }
    
    /// Get client instance
    pub fn client(&self) -> Option<&MarketplaceClient> {
        self.client.as_ref()
    }
    
    /// Get registry
    pub fn registry(&self) -> &MarketplaceRegistry {
        &self.registry
    }
    
    /// Get settlement engine
    pub fn settlement(&self) -> &SettlementEngine {
        &self.settlement
    }
    
    /// Get reputation engine
    pub fn reputation(&self) -> &ReputationEngine {
        &self.reputation
    }
}

impl Default for MarketplaceIntegration {
    fn default() -> Self {
        Self::new()
    }
}

use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_marketplace_integration_creation() {
        let integration = MarketplaceIntegration::new();
        assert!(integration.provider().is_none());
        assert!(integration.client().is_none());
    }
}
