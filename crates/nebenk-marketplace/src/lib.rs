//! NEBENK Marketplace - Decentralized device marketplace for distributed applications

pub mod client;
pub mod network_registry;
pub mod provider;
pub mod registry;
pub mod reputation;
pub mod settlement;
pub mod types;

// Blockchain module - only available with "blockchain" feature
#[cfg(feature = "blockchain")]
pub mod blockchain;

// Re-export key types
pub use client::{ClientConfig, MarketplaceClient};
pub use network_registry::NetworkMarketplaceRegistry;
pub use provider::{MarketplaceProvider, ProviderConfig, ProviderState};
pub use registry::{
    topics, DeploymentAcceptance, DeploymentOffer, MarketplaceRegistry, ProviderFilter,
    ProviderListRequest, ProviderListResponse, RegistryConfig,
};
pub use reputation::{BenchmarkResult, PeerReview, ReputationConfig, ReputationEngine};
pub use settlement::{
    EscrowAccount, ProviderEpochMetrics, ProviderStake, SettlementConfig, SettlementEngine,
};
pub use types::*;

// Re-export blockchain types when feature is enabled
#[cfg(feature = "blockchain")]
pub use blockchain::{
    BlockchainClient, BlockchainConfig, OnChainSettlementEngine,
    ProviderStake as BlockchainProviderStake,
};

/// Marketplace integration with NEBENK runtime
pub struct MarketplaceIntegration {
    provider: Option<MarketplaceProvider>,
    client: Option<MarketplaceClient>,
    registry: MarketplaceRegistry,
    settlement: SettlementEngine,
    reputation: ReputationEngine,
    #[cfg(feature = "blockchain")]
    on_chain_settlement: Option<blockchain::OnChainSettlementEngine>,
}

impl MarketplaceIntegration {
    pub fn new() -> Self {
        Self {
            provider: None,
            client: None,
            registry: MarketplaceRegistry::default(),
            settlement: SettlementEngine::default(),
            reputation: ReputationEngine::default(),
            #[cfg(feature = "blockchain")]
            on_chain_settlement: None,
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

    /// Enable blockchain settlement (requires "blockchain" feature)
    #[cfg(feature = "blockchain")]
    pub fn enable_blockchain_settlement(
        &mut self,
        local_config: crate::settlement::SettlementConfig,
        blockchain_config: blockchain::BlockchainConfig,
    ) -> Result<()> {
        self.on_chain_settlement = Some(blockchain::OnChainSettlementEngine::new(
            local_config,
            blockchain_config,
        )?);
        Ok(())
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

    /// Get settlement engine (local)
    pub fn settlement(&self) -> &SettlementEngine {
        &self.settlement
    }

    /// Get on-chain settlement engine (if enabled)
    #[cfg(feature = "blockchain")]
    pub fn on_chain_settlement(&self) -> Option<&blockchain::OnChainSettlementEngine> {
        self.on_chain_settlement.as_ref()
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
