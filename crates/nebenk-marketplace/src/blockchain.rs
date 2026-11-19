//! Blockchain integration layer for on-chain settlement
//!
//! This module is only available when the "blockchain" feature is enabled.

#[cfg(feature = "blockchain")]
mod blockchain_impl {
    use crate::types::*;
    use crate::settlement::{SettlementEngine, SettlementConfig, ProviderEpochMetrics, EscrowAccount, EpochSettlement};
    use nebenk_core::error::{NebenkError, Result};
    use nebenk_core::types::NodeId;
    use async_trait::async_trait;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::sync::RwLock;
    use tracing::{debug, info, warn};

    // Re-export alloy types when blockchain feature is enabled
    pub use alloy::{
        network::EthereumWallet,
        primitives::{Address, Bytes, FixedBytes, U256},
        providers::{Provider, ProviderBuilder, RootProvider},
        rpc::types::TransactionRequest,
        signers::local::PrivateKeySigner,
        sol,
        transports::http::reqwest::Client,
        transports::http::Http,
    };

    sol! {
        #[sol(rpc)]
        interface IERC20 {
            function transfer(address to, uint256 value) external returns (bool);
            function transferFrom(address from, address to, uint256 value) external returns (bool);
            function balanceOf(address account) external view returns (uint256);
            function allowance(address owner, address spender) external view returns (uint256);
            function approve(address spender, uint256 value) external returns (bool);
            function symbol() external view returns (string memory);
        }

        #[sol(rpc)]
        interface INeonEscrow {
            function createEscrow(
                bytes32 deploymentId,
                address[] calldata providers,
                address token,
                uint32 epochsToFund,
                uint256 estimatedCostPerEpoch
            ) external;

            function deposit(bytes32 deploymentId, uint256 amount) external;

            function settleEpoch(
                bytes32 deploymentId,
                uint32 epoch,
                tuple(
                    address provider,
                    uint256 uptimePercentage,
                    uint256 replicationLagMs,
                    bool dataCorrupted,
                    bool maliciousDetected,
                    uint256 operationsProcessed,
                    uint256 bytesTransferred
                )[] calldata metrics
            ) external;

            function depositStake(address token, uint256 amount) external;

            function lockStake(uint256 epochs) external;

            function unlockStake() external;

            function requestWithdrawal(uint256 amount) external;

            function executeWithdrawal(uint256 amount) external;

            function closeEscrow(bytes32 deploymentId) external;

            function raiseDispute(bytes32 deploymentId, address target, string calldata reason) external;

            function resolveDispute(bytes32 deploymentId, bool providerWon, uint256 compensation) external;

            function getEscrow(bytes32 deploymentId) external view returns (
                address developer,
                address[] memory providers,
                address token,
                uint256 totalDeposited,
                uint256 currentBalance,
                uint32 epochsFunded,
                uint64 createdAt,
                uint8 status
            );

            function getProviderStake(address provider) external view returns (
                uint256 amount,
                string memory token,
                bool locked,
                uint64 lockExpiresAt,
                uint256 totalSlashed
            );

            function getSettlementHistory(bytes32 deploymentId) external view returns (
                tuple(
                    uint32 epoch,
                    bytes32 deploymentId,
                    address provider,
                    uint64 epochStart,
                    uint64 epochEnd,
                    uint256 uptimePercentage,
                    uint256 replicationLagMs,
                    bool slaMet,
                    uint256 paymentAmount,
                    uint256 slashedAmount,
                    uint64 settledAt
                )[] memory
            );
        }
    }

    /// Blockchain configuration
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BlockchainConfig {
        /// RPC endpoint URL (e.g., "https://mainnet.infura.io/v3/...")
        pub rpc_url: String,
        /// Private key for signing transactions (hex, without 0x prefix)
        pub private_key: Option<String>,
        /// Contract address
        pub contract_address: Option<Address>,
        /// Chain ID (1 = Ethereum mainnet, 137 = Polygon, etc.)
        pub chain_id: u64,
        /// Gas price multiplier (1.0 = normal, 1.5 = fast)
        pub gas_price_multiplier: f64,
        /// Max gas price in gwei
        pub max_gas_price_gwei: u64,
        /// Confirmation blocks to wait
        pub confirmations: u64,
    }

    impl Default for BlockchainConfig {
        fn default() -> Self {
            Self {
                rpc_url: "http://localhost:8545".to_string(),
                private_key: None,
                contract_address: None,
                chain_id: 1,
                gas_price_multiplier: 1.2,
                max_gas_price_gwei: 100,
                confirmations: 1,
            }
        }
    }

    /// Blockchain client for interacting with the NeonEscrow contract
    pub struct BlockchainClient {
        config: BlockchainConfig,
        provider: Option<RootProvider<Http<Client>>>,
        contract: Option<INeonEscrow<RootProvider<Http<Client>>>>,
        wallet: Option<EthereumWallet>,
    }

    impl BlockchainClient {
        /// Create a new blockchain client
        pub fn new(config: BlockchainConfig) -> Result<Self> {
            let provider = ProviderBuilder::new()
                .on_http(config.rpc_url.parse().map_err(|e| {
                    NebenkError::Blockchain(format!("Invalid RPC URL: {}", e))
                })?);

            let wallet = if let Some(pk) = &config.private_key {
                let signer = pk.parse::<PrivateKeySigner>().map_err(|e| {
                    NebenkError::Blockchain(format!("Invalid private key: {}", e))
                })?;
                Some(EthereumWallet::from(signer))
            } else {
                None
            };

            let contract = if let Some(addr) = config.contract_address {
                Some(INeonEscrow::new(addr, provider.clone()))
            } else {
                None
            };

            Ok(Self {
                config,
                provider: Some(provider),
                contract,
                wallet,
            })
        }

        /// Create a client with provider and wallet (for testing)
        pub fn with_provider(
            provider: RootProvider<Http<Client>>,
            contract_address: Option<Address>,
            wallet: Option<EthereumWallet>,
        ) -> Self {
            let contract = contract_address.map(|addr| INeonEscrow::new(addr, provider.clone()));
            Self {
                config: BlockchainConfig::default(),
                provider: Some(provider),
                contract,
                wallet,
            }
        }

        /// Get the contract instance
        fn contract(&self) -> Result<&INeonEscrow<RootProvider<Http<Client>>>> {
            self.contract.as_ref().ok_or_else(|| {
                NebenkError::Blockchain("Contract not initialized. Set contract_address in config.".into())
            })
        }

        /// Get the provider
        fn provider(&self) -> Result<&RootProvider<Http<Client>>> {
            self.provider.as_ref().ok_or_else(|| {
                NebenkError::Blockchain("Provider not initialized".into())
            })
        }

        /// Convert DeploymentId to bytes32
        fn deployment_id_to_bytes32(deployment_id: DeploymentId) -> FixedBytes<32> {
            let mut bytes = [0u8; 32];
            bytes[..16].copy_from_slice(deployment_id.0.as_bytes());
            FixedBytes::from(bytes)
        }

        /// Convert NodeId to Address (simplified - in production would use proper mapping)
        fn node_id_to_address(node_id: NodeId) -> Address {
            let mut bytes = [0u8; 20];
            bytes.copy_from_slice(&node_id.0[..20]);
            Address::from(bytes)
        }

        /// Create escrow on-chain
        pub async fn create_escrow(
            &self,
            deployment_id: DeploymentId,
            developer: NodeId,
            providers: Vec<NodeId>,
            token: Address,
            epochs_to_fund: u32,
            estimated_cost_per_epoch: u64,
        ) -> Result<()> {
            let contract = self.contract()?;
            let provider_addresses: Vec<Address> = providers.into_iter()
                .map(Self::node_id_to_address)
                .collect();

            let tx = contract.createEscrow(
                Self::deployment_id_to_bytes32(deployment_id),
                provider_addresses,
                token,
                epochs_to_fund,
                U256::from(estimated_cost_per_epoch),
            );

            let receipt = self.send_transaction(tx).await?;
            info!("Created escrow on-chain: deployment={}, tx={:?}", deployment_id, receipt.transaction_hash);
            Ok(())
        }

        /// Deposit additional funds
        pub async fn deposit(&self, deployment_id: DeploymentId, amount: u64) -> Result<()> {
            let contract = self.contract()?;
            let tx = contract.deposit(
                Self::deployment_id_to_bytes32(deployment_id),
                U256::from(amount),
            );
            let receipt = self.send_transaction(tx).await?;
            info!("Deposited {} to escrow {}: tx={:?}", amount, deployment_id, receipt.transaction_hash);
            Ok(())
        }

        /// Settle epoch on-chain
        pub async fn settle_epoch(
            &self,
            deployment_id: DeploymentId,
            epoch: u32,
            provider_metrics: Vec<ProviderEpochMetrics>,
        ) -> Result<Vec<EpochSettlement>> {
            let contract = self.contract()?;
            
            // Convert metrics to contract format
            let metrics: Vec<_> = provider_metrics.into_iter().map(|m| (
                Self::node_id_to_address(m.provider_node_id),
                U256::from((m.uptime_percentage * 100.0) as u64), // Convert to basis points
                U256::from(m.replication_lag_ms),
                m.data_corrupted,
                m.malicious_detected,
                U256::from(m.operations_processed),
                U256::from(m.bytes_transferred),
            )).collect();

            let tx = contract.settleEpoch(
                Self::deployment_id_to_bytes32(deployment_id),
                epoch,
                metrics,
            );

            let receipt = self.send_transaction(tx).await?;
            info!("Settled epoch {} for deployment {}: tx={:?}", epoch, deployment_id, receipt.transaction_hash);

            // In a real implementation, we'd parse the event logs to get the settlements
            // For now, return empty vec
            Ok(vec![])
        }

        /// Deposit stake as provider
        pub async fn deposit_stake(&self, token: Address, amount: u64) -> Result<()> {
            let contract = self.contract()?;
            let tx = contract.depositStake(token, U256::from(amount));
            let receipt = self.send_transaction(tx).await?;
            info!("Deposited stake: {} tokens, tx={:?}", amount, receipt.transaction_hash);
            Ok(())
        }

        /// Lock stake for epochs
        pub async fn lock_stake(&self, epochs: u64) -> Result<()> {
            let contract = self.contract()?;
            let tx = contract.lockStake(U256::from(epochs));
            let receipt = self.send_transaction(tx).await?;
            info!("Locked stake for {} epochs, tx={:?}", epochs, receipt.transaction_hash);
            Ok(())
        }

        /// Unlock stake
        pub async fn unlock_stake(&self) -> Result<()> {
            let contract = self.contract()?;
            let tx = contract.unlockStake();
            let receipt = self.send_transaction(tx).await?;
            info!("Unlocked stake, tx={:?}", receipt.transaction_hash);
            Ok(())
        }

        /// Request withdrawal
        pub async fn request_withdrawal(&self, amount: u64) -> Result<()> {
            let contract = self.contract()?;
            let tx = contract.requestWithdrawal(U256::from(amount));
            let receipt = self.send_transaction(tx).await?;
            info!("Requested withdrawal: {} tokens, tx={:?}", amount, receipt.transaction_hash);
            Ok(())
        }

        /// Execute withdrawal
        pub async fn execute_withdrawal(&self, amount: u64) -> Result<()> {
            let contract = self.contract()?;
            let tx = contract.executeWithdrawal(U256::from(amount));
            let receipt = self.send_transaction(tx).await?;
            info!("Executed withdrawal: {} tokens, tx={:?}", amount, receipt.transaction_hash);
            Ok(())
        }

        /// Close escrow
        pub async fn close_escrow(&self, deployment_id: DeploymentId) -> Result<()> {
            let contract = self.contract()?;
            let tx = contract.closeEscrow(Self::deployment_id_to_bytes32(deployment_id));
            let receipt = self.send_transaction(tx).await?;
            info!("Closed escrow {}: tx={:?}", deployment_id, receipt.transaction_hash);
            Ok(())
        }

        /// Get escrow info from chain
        pub async fn get_escrow(&self, deployment_id: DeploymentId) -> Result<EscrowAccount> {
            let contract = self.contract()?;
            let result = contract.getEscrow(Self::deployment_id_to_bytes32(deployment_id)).call().await
                .map_err(|e| NebenkError::Blockchain(format!("Call failed: {}", e)))?;

            Ok(EscrowAccount {
                deployment_id,
                developer_node_id: Self::address_to_node_id(result.developer),
                provider_node_ids: result.providers.into_iter()
                    .map(Self::address_to_node_id)
                    .collect(),
                token: format!("{:?}", result.token), // Would need proper token resolution
                total_deposited: result.totalDeposited.to::<u64>(),
                total_paid: U256::from(result.totalDeposited).saturating_sub(result.currentBalance).to::<u64>(),
                total_slashed: 0, // Not returned by contract
                current_balance: result.currentBalance.to::<u64>(),
                epochs_funded: result.epochsFunded,
                created_at: result.createdAt,
                last_settlement_at: None,
                status: match result.status {
                    0 => crate::settlement::EscrowStatus::Active,
                    1 => crate::settlement::EscrowStatus::Depleted,
                    2 => crate::settlement::EscrowStatus::Refunding,
                    3 => crate::settlement::EscrowStatus::Closed,
                    4 => crate::settlement::EscrowStatus::Disputed,
                    _ => crate::settlement::EscrowStatus::Active,
                },
            })
        }

        /// Get provider stake from chain
        pub async fn get_provider_stake(&self, provider: NodeId) -> Result<ProviderStake> {
            let contract = self.contract()?;
            let result = contract.getProviderStake(Self::node_id_to_address(provider)).call().await
                .map_err(|e| NebenkError::Blockchain(format!("Call failed: {}", e)))?;

            Ok(ProviderStake {
                node_id: provider,
                amount: result.amount.to::<u64>(),
                token: result.token,
                locked: result.locked,
                lock_expires_at: Some(result.lockExpiresAt),
                total_slashed: result.totalSlashed.to::<u64>(),
            })
        }

        /// Send transaction and wait for confirmation
        async fn send_transaction(
            &self,
            tx: INeonEscrow::createEscrowCall,
        ) -> Result<alloy::rpc::types::TransactionReceipt> {
            let provider = self.provider()?;
            let wallet = self.wallet.as_ref().ok_or_else(|| {
                NebenkError::Blockchain("Wallet not configured".into())
            })?;

            let pending = tx.send(wallet).await
                .map_err(|e| NebenkError::Blockchain(format!("Send failed: {}", e)))?;
            
            let receipt = pending.get_receipt(self.config.confirmations).await
                .map_err(|e| NebenkError::Blockchain(format!("Receipt failed: {}", e)))?;

            Ok(receipt)
        }

        fn address_to_node_id(address: Address) -> NodeId {
            let mut bytes = [0u8; 32];
            bytes[..20].copy_from_slice(address.as_slice());
            NodeId::from_bytes(bytes)
        }
    }

    /// On-chain settlement engine that combines local and blockchain state
    pub struct OnChainSettlementEngine {
        local_engine: SettlementEngine,
        blockchain: Option<BlockchainClient>,
        config: BlockchainConfig,
    }

    impl OnChainSettlementEngine {
        pub fn new(local_config: SettlementConfig, blockchain_config: BlockchainConfig) -> Result<Self> {
            let blockchain = if blockchain_config.contract_address.is_some() {
                Some(BlockchainClient::new(blockchain_config.clone())?)
            } else {
                None
            };

            Ok(Self {
                local_engine: SettlementEngine::new(local_config),
                blockchain,
                config: blockchain_config,
            })
        }

        /// Create escrow (both local and on-chain)
        pub async fn create_escrow(
            &self,
            deployment_id: DeploymentId,
            developer: NodeId,
            providers: Vec<NodeId>,
            token: String,
            epochs_to_fund: u32,
            estimated_cost_per_epoch: u64,
        ) -> Result<EscrowAccount> {
            // Create locally first
            let escrow = self.local_engine.create_escrow(
                deployment_id,
                developer,
                providers.clone(),
                token.clone(),
                epochs_to_fund,
                estimated_cost_per_epoch,
            ).await?;

            // Create on-chain if configured
            if let Some(bc) = &self.blockchain {
                // Resolve token address from symbol (simplified)
                let token_address = self.resolve_token_address(&token)?;
                bc.create_escrow(deployment_id, developer, providers, token_address, epochs_to_fund, estimated_cost_per_epoch).await?;
            }

            Ok(escrow)
        }

        /// Settle epoch (both local and on-chain)
        pub async fn settle_epoch(
            &self,
            deployment_id: DeploymentId,
            metrics: Vec<ProviderEpochMetrics>,
        ) -> Result<Vec<EpochSettlement>> {
            // Settle locally
            let settlements = self.local_engine.settle_epoch(deployment_id, metrics.clone()).await?;

            // Settle on-chain if configured
            if let Some(bc) = &self.blockchain {
                let epoch = self.calculate_current_epoch(deployment_id).await?;
                bc.settle_epoch(deployment_id, epoch, metrics).await?;
            }

            Ok(settlements)
        }

        /// Calculate current epoch based on escrow creation time
        async fn calculate_current_epoch(&self, deployment_id: DeploymentId) -> Result<u32> {
            if let Some(escrow) = self.local_engine.get_escrow(deployment_id).await {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                let epoch = (now - escrow.created_at) / self.local_engine.config.epoch_duration_secs;
                Ok(epoch as u32)
            } else {
                Err(NebenkError::EscrowNotFound)
            }
        }

        fn resolve_token_address(&self, symbol: &str) -> Result<Address> {
            // In production, this would query a token registry
            match symbol.to_uppercase().as_str() {
                "USDC" => Ok("0xA0b86a33E6441b8c4C8C8C8C8C8C8C8C8C8C8C8C".parse().unwrap()),
                "USDT" => Ok("0xdAC17F958D2ee523a2206206994597C13D831ec7".parse().unwrap()),
                "ETH" => Ok("0x0000000000000000000000000000000000000000".parse().unwrap()),
                _ => Err(NebenkError::Blockchain(format!("Unknown token: {}", symbol))),
            }
        }

        /// Delegate to local engine for other operations
        pub async fn get_escrow(&self, deployment_id: DeploymentId) -> Option<EscrowAccount> {
            self.local_engine.get_escrow(deployment_id).await
        }

        pub async fn get_stake(&self, node_id: NodeId) -> Option<ProviderStake> {
            // Try blockchain first
            if let Some(bc) = &self.blockchain {
                if let Ok(stake) = bc.get_provider_stake(node_id).await {
                    return Some(stake);
                }
            }
            self.local_engine.get_stake(node_id).await
        }

        pub async fn get_history(&self, deployment_id: DeploymentId) -> Vec<EpochSettlement> {
            self.local_engine.get_history(deployment_id).await
        }
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

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::types::*;
        use nebenk_core::types::NodeId;

        #[test]
        fn test_deployment_id_to_bytes32() {
            let deployment_id = DeploymentId::new();
            let bytes32 = BlockchainClient::deployment_id_to_bytes32(deployment_id);
            
            // Check first 16 bytes match UUID
            assert_eq!(&bytes32[..16], deployment_id.0.as_bytes());
        }

        #[test]
        fn test_node_id_to_address() {
            let node_id = NodeId::from_bytes([1u8; 32]);
            let address = BlockchainClient::node_id_to_address(node_id);
            
            // Check first 20 bytes match
            assert_eq!(address.as_slice(), &[1u8; 20]);
        }

        #[test]
        fn test_address_to_node_id() {
            let address = Address::from([2u8; 20]);
            let node_id = BlockchainClient::address_to_node_id(address);
            
            assert_eq!(&node_id.0[..20], &[2u8; 20]);
        }
    }
}

// Re-export when blockchain feature is enabled
#[cfg(feature = "blockchain")]
pub use blockchain_impl::*;

// Provide stub types when blockchain feature is disabled
#[cfg(not(feature = "blockchain"))]
pub mod stub {
    use crate::types::*;
    use nebenk_core::error::{NebenkError, Result};
    use nebenk_core::types::NodeId;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::time::SystemTime;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BlockchainConfig {
        pub rpc_url: String,
        pub private_key: Option<String>,
        pub contract_address: Option<String>,
        pub chain_id: u64,
        pub gas_price_multiplier: f64,
        pub max_gas_price_gwei: u64,
        pub confirmations: u64,
    }

    impl Default for BlockchainConfig {
        fn default() -> Self {
            Self {
                rpc_url: "http://localhost:8545".to_string(),
                private_key: None,
                contract_address: None,
                chain_id: 1,
                gas_price_multiplier: 1.2,
                max_gas_price_gwei: 100,
                confirmations: 1,
            }
        }
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

    pub struct BlockchainClient;

    impl BlockchainClient {
        pub fn new(_config: BlockchainConfig) -> Result<Self> {
            Err(NebenkError::Blockchain("Blockchain feature not enabled. Enable 'blockchain' feature in Cargo.toml".into()))
        }
    }

    pub struct OnChainSettlementEngine;

    impl OnChainSettlementEngine {
        pub fn new(_local_config: crate::settlement::SettlementConfig, _blockchain_config: BlockchainConfig) -> Result<Self> {
            Err(NebenkError::Blockchain("Blockchain feature not enabled. Enable 'blockchain' feature in Cargo.toml".into()))
        }
    }
}

#[cfg(not(feature = "blockchain"))]
pub use stub::*;
