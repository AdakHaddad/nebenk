use clap::{Parser, Subcommand};

use nebenk_core::types::{ClusterId, NodeId};
use nebenk_identity::NodeIdentity;
use nebenk_runtime::{NodeConfig, NodeRuntime};
use nebenk_storage::SqliteStore;
use nebenk_wasm::{KvCommand, KvQuery, KvQueryResult, NativeKvEngine};
use nebenk_marketplace::{
    client::{MarketplaceClient, ClientConfig},
    provider::{MarketplaceProvider, ProviderConfig},
    registry::{MarketplaceRegistry, ProviderFilter, ProviderListRequest},
    reputation::ReputationEngine,
    settlement::{SettlementEngine, ProviderEpochMetrics},
    types::*,
};
use nebenk_network::{
    marketplace::{MarketplaceEvent, MarketplaceNetwork},
    
};

use libp2p::Multiaddr;
use uuid::Uuid;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

#[derive(Parser)]
#[command(name = "nebenk")]
#[command(about = "NEBENK — Lightweight distributed application runtime", long_about = None)]
struct Cli {
    /// Base directory for identity keys and data storage
    #[arg(long, default_value = ".nebenk")]
    data_dir: PathBuf,

    /// Cluster identifier
    #[arg(long, default_value = "nebenk-default")]
    cluster: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new node identity and storage environment
    Init,

    /// Display current node status, identity, and state revision
    Status,

    /// Write a key-value pair to the state machine
    Put { key: String, value: String },

    /// Read a key value from the state machine
    Get { key: String },

    /// List all keys currently in the state machine
    List,

    /// Force creation of an application state snapshot
    Snapshot,

    /// Create an invitation token to invite another node to join the cluster
    Invite {
        /// Validity duration in seconds (default: 3600 = 1 hour)
        #[arg(long, default_value_t = 3600)]
        expires_in: u64,
    },

    /// Marketplace commands
    #[command(subcommand)]
    Marketplace(MarketplaceCommands),
}

#[derive(Subcommand)]
enum MarketplaceCommands {
    /// Register as a marketplace provider
    Register {
        /// Stake amount in tokens
        #[arg(long, default_value_t = 1000)]
        stake: u64,
        /// Token type (e.g., USDC, ETH)
        #[arg(long, default_value = "USDC")]
        token: String,
        /// CPU price per hour
        #[arg(long, default_value_t = 10)]
        cpu_price: u64,
        /// Memory price per GB hour
        #[arg(long, default_value_t = 1)]
        mem_price: u64,
        /// Storage price per GB hour
        #[arg(long, default_value_t = 1)]
        storage_price: u64,
        /// Egress price per GB
        #[arg(long, default_value_t = 5)]
        egress_price: u64,
        /// CPU cores
        #[arg(long, default_value_t = 4)]
        cpu_cores: u32,
        /// Memory GB
        #[arg(long, default_value_t = 16)]
        memory_gb: u32,
        /// Storage GB
        #[arg(long, default_value_t = 100)]
        storage_gb: u32,
        /// Lock duration in seconds (default: 30 days)
        #[arg(long, default_value_t = 2592000)]
        lock_duration: u64,
    },

    /// Deploy an application to the marketplace
    Deploy {
        /// Path to WASM application
        #[arg(long)]
        app: PathBuf,
        /// Required CPU cores
        #[arg(long, default_value_t = 2)]
        cpu: u32,
        /// Required memory GB
        #[arg(long, default_value_t = 4)]
        mem: u32,
        /// Required storage GB
        #[arg(long, default_value_t = 10)]
        storage: u32,
        /// Number of replicas
        #[arg(long, default_value_t = 3)]
        replicas: u32,
        /// Maximum price per CPU hour
        #[arg(long, default_value_t = 20)]
        max_cpu_price: u64,
    },

    /// List available providers
    List {
        /// Minimum CPU cores
        #[arg(long)]
        min_cpu: Option<u32>,
        /// Minimum memory GB
        #[arg(long)]
        min_mem: Option<u32>,
        /// Minimum reputation score
        #[arg(long)]
        min_reputation: Option<u8>,
        /// Maximum CPU price per hour
        #[arg(long)]
        max_cpu_price: Option<u64>,
    },

    /// Get provider reputation
    Reputation {
        /// Provider node ID (base58 encoded)
        #[arg(long)]
        provider: String,
    },

    /// Start marketplace network node
    Network {
        /// Listen address (e.g., /ip4/0.0.0.0/tcp/0)
        #[arg(long, default_value = "/ip4/0.0.0.0/tcp/0")]
        listen: String,
        /// Bootstrap peer address (optional)
        #[arg(long)]
        bootstrap: Option<String>,
    },

    /// Settlement operations
    #[command(subcommand)]
    Settlement(SettlementCommands),
}

#[derive(Subcommand)]
enum SettlementCommands {
    /// Create escrow for deployment
    CreateEscrow {
        /// Deployment ID (UUID)
        #[arg(long)]
        deployment_id: String,
        /// Developer node ID (base58)
        #[arg(long)]
        developer: String,
        /// Provider node IDs (comma-separated base58)
        #[arg(long)]
        providers: String,
        /// Token type
        #[arg(long, default_value = "USDC")]
        token: String,
        /// Amount per epoch
        #[arg(long, default_value_t = 10)]
        amount_per_epoch: u64,
        /// Total epochs
        #[arg(long, default_value_t = 100)]
        epochs: u32,
    },

    /// Settle epoch
    Settle {
        /// Deployment ID (UUID)
        #[arg(long)]
        deployment_id: String,
        /// Provider uptime percentage
        #[arg(long, default_value_t = 99.5)]
        uptime: f32,
    },
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

async fn create_runtime(data_dir: &PathBuf, cluster_name: &str) -> anyhow::Result<NodeRuntime> {
    fs::create_dir_all(data_dir)?;
    let key_path = data_dir.join("node.key");
    let db_path = data_dir.join("state.db");

    let identity = NodeIdentity::load_or_create(key_path)?;
    let cluster_id = ClusterId::new(cluster_name);
    let storage = SqliteStore::open(db_path, cluster_id.clone())?;
    let engine = Box::new(NativeKvEngine::new());

    let config = NodeConfig {
        cluster_id,
        is_primary: true,
        snapshot_interval: 50,
    };

    let mut runtime = NodeRuntime::new(identity, config, storage, engine);
    runtime.recover().await?;
    Ok(runtime)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            fs::create_dir_all(&cli.data_dir)?;
            let key_path = cli.data_dir.join("node.key");
            let identity = NodeIdentity::load_or_create(&key_path)?;
            println!("Node initialized successfully!");
            println!("Node ID:  {}", identity.node_id());
            println!("Key Path: {}", key_path.display());
            println!("Cluster:  {}", cli.cluster);
        }

        Commands::Status => {
            let runtime = create_runtime(&cli.data_dir, &cli.cluster).await?;
            println!("NEBENK Node Status");
            println!("==================");
            println!("Node ID:          {}", runtime.node_id());
            println!("Cluster ID:       {}", runtime.cluster_id());
            println!("Current Revision: {}", runtime.current_revision());
            println!("Data Dir:         {}", cli.data_dir.display());
        }

        Commands::Put { key, value } => {
            let mut runtime = create_runtime(&cli.data_dir, &cli.cluster).await?;
            let cmd = KvCommand::Put {
                key: key.clone(),
                value: value.into_bytes(),
            };
            let payload = serde_json::to_vec(&cmd)?;
            let now = current_timestamp_ms();

            let result = runtime.submit_command(payload, now).await?;
            println!(
                "OK (revision: {}, state_hash: {})",
                result.revision,
                bs58::encode(&result.state_hash).into_string()
            );
        }

        Commands::Get { key } => {
            let runtime = create_runtime(&cli.data_dir, &cli.cluster).await?;
            let query = KvQuery::Get { key: key.clone() };
            let query_bytes = serde_json::to_vec(&query)?;
            let out_bytes = runtime.query(&query_bytes)?;

            let res: KvQueryResult = serde_json::from_slice(&out_bytes)?;
            match res {
                KvQueryResult::Value(Some(v)) => {
                    println!("{}", String::from_utf8_lossy(&v));
                }
                KvQueryResult::Value(None) => {
                    eprintln!("Key '{}' not found", key);
                    std::process::exit(1);
                }
                _ => {}
            }
        }

        Commands::List => {
            let runtime = create_runtime(&cli.data_dir, &cli.cluster).await?;
            let query = KvQuery::ListKeys;
            let query_bytes = serde_json::to_vec(&query)?;
            let out_bytes = runtime.query(&query_bytes)?;

            let res: KvQueryResult = serde_json::from_slice(&out_bytes)?;
            if let KvQueryResult::Keys(keys) = res {
                for k in keys {
                    println!("{k}");
                }
            }
        }

        Commands::Snapshot => {
            let mut runtime = create_runtime(&cli.data_dir, &cli.cluster).await?;
            let now = current_timestamp_ms();
            let snap = runtime.create_snapshot(now).await?;
            println!(
                "Snapshot created successfully for revision {} (size: {} bytes, hash: {})",
                snap.header.revision,
                snap.header.size_bytes,
                bs58::encode(&snap.header.state_hash).into_string()
            );
        }

        Commands::Invite { expires_in } => {
            let key_path = cli.data_dir.join("node.key");
            let identity = NodeIdentity::load_or_create(key_path)?;
            let cluster_id = ClusterId::new(&cli.cluster);
            let now_s = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let token = identity.create_invitation(&cluster_id, now_s + expires_in);
            println!("Invitation Token (valid for {}s):", expires_in);
            println!("{}", token);
        }

        Commands::Marketplace(cmd) => {
            handle_marketplace_command(cmd, &cli.data_dir, &cli.cluster).await?;
        }
    }

    Ok(())
}

async fn handle_marketplace_command(
    cmd: MarketplaceCommands,
    data_dir: &PathBuf,
    cluster_name: &str,
) -> anyhow::Result<()> {
    let key_path = data_dir.join("node.key");
    let identity = std::sync::Arc::new(NodeIdentity::load_or_create(key_path)?);
    let cluster_id = ClusterId::new(cluster_name);

    match cmd {
        MarketplaceCommands::Register {
            stake,
            token,
            cpu_price,
            mem_price,
            storage_price,
            egress_price,
            cpu_cores,
            memory_gb,
            storage_gb,
            lock_duration,
        } => {
            let provider_config = ProviderConfig::default();
            let provider = MarketplaceProvider::new(provider_config, identity.clone());

            let capabilities = DeviceCapabilities {
                cpu_cores,
                cpu_arch: "x86_64".to_string(),
                os: std::env::consts::OS.to_string(),
                memory_gb,
                storage_gb,
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
                cpu_per_hour: cpu_price,
                memory_per_gb_hour: mem_price,
                storage_per_gb_hour: storage_price,
                gpu_per_hour: None,
                egress_per_gb: egress_price,
                min_epoch_hours: 1,
            };

            let stake_info = StakeInfo {
                amount: stake,
                token,
                lock_duration_secs: lock_duration,
                contract_address: None,
            };

            let advertisement = provider.register(capabilities, pricing, stake_info).await?;
            println!("Provider registered successfully!");
            println!("Node ID: {}", advertisement.node_id);
            println!("Reputation: {}", advertisement.reputation.0);
            println!("Registered at: {}", advertisement.registered_at);
        }

        MarketplaceCommands::Deploy {
            app,
            cpu,
            mem,
            storage,
            replicas,
            max_cpu_price,
        } => {
            let client_config = ClientConfig::default();
            let _client = MarketplaceClient::new(client_config, identity.clone());

            println!("Deploying application: {}", app.display());
            println!("Requirements: {} CPU, {} GB RAM, {} GB storage", cpu, mem, storage);
            println!("Replicas: {}", replicas);
            println!("Max CPU price: {}", max_cpu_price);
            println!("Cluster: {}", cluster_id);
            println!("\nNote: Full deployment requires network integration (Phase 2)");
        }

        MarketplaceCommands::List {
            min_cpu,
            min_mem,
            min_reputation,
            max_cpu_price,
        } => {
            let registry = MarketplaceRegistry::default();
            let filter = ProviderFilter {
                min_cpu_cores: min_cpu,
                min_memory_gb: min_mem,
                min_reputation,
                max_price_cpu: max_cpu_price,
                ..Default::default()
            };

            let request = ProviderListRequest {
                requester: identity.node_id(),
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                filter: Some(filter),
            };

            let response = registry.handle_list_request(request).await?;
            println!("Available providers: {}", response.providers.len());
            for provider in response.providers {
                println!(
                    "  {} - {} CPU, {} GB RAM, {} GB storage, {} reputation, {} tokens/CPU/hr",
                    provider.node_id,
                    provider.capabilities.cpu_cores,
                    provider.capabilities.memory_gb,
                    provider.capabilities.storage_gb,
                    provider.reputation.0,
                    provider.pricing.cpu_per_hour
                );
            }
        }

        MarketplaceCommands::Reputation { provider } => {
            let reputation = ReputationEngine::default();
            let provider_id = NodeId::from_base58(&provider)?;
            let score = reputation.get_score(provider_id).await;
            println!("Provider: {}", provider);
            println!("Reputation Score: {}/100", score.0);
        }

        MarketplaceCommands::Network { listen, bootstrap: _ } => {
            let listen_addr: Multiaddr = listen.parse()
                .map_err(|e| anyhow::anyhow!("Invalid listen address: {}", e))?;
            
            let key_path = data_dir.join("node.key");
            let identity = NodeIdentity::load_or_create(key_path)?;
            let keypair = identity.to_libp2p_keypair();
            
            let (_cmd_tx, cmd_rx) = mpsc::unbounded_channel();
            let (evt_tx, mut evt_rx) = mpsc::unbounded_channel();
            
            let network = MarketplaceNetwork::new(keypair, cmd_rx, evt_tx).map_err(|e| anyhow::anyhow!(e))?;
            
            println!("Starting marketplace network on {}", listen_addr);
            println!("Node ID: {}", identity.node_id());
            
            // Handle events in background
            tokio::spawn(async move {
                while let Some(event) = evt_rx.recv().await {
                    match event {
                        MarketplaceEvent::ProviderAdvertisement(adv) => {
                            println!("New provider: {} - {} CPU, {} GB RAM", 
                                adv.node_id, adv.capabilities.cpu_cores, adv.capabilities.memory_gb);
                        }
                        MarketplaceEvent::ProviderHeartbeat(hb) => {
                            println!("Heartbeat from: {}", hb.node_id);
                        }
                        _ => {}
                    }
                }
            });
            
            // Run network (this blocks)
            network.run(listen_addr).await?;
        }

        MarketplaceCommands::Settlement(cmd) => {
            handle_settlement_command(cmd, identity.node_id()).await?;
        }
    }

    Ok(())
}

async fn handle_settlement_command(
    cmd: SettlementCommands,
    developer: NodeId,
) -> anyhow::Result<()> {
    let settlement = SettlementEngine::default();

    match cmd {
        SettlementCommands::CreateEscrow {
            deployment_id,
            developer: dev_str,
            providers,
            token,
            amount_per_epoch,
            epochs,
        } => {
            let dev_id = NodeId::from_base58(&dev_str)?;
            let provider_ids: Vec<NodeId> = providers
                .split(',')
                .map(|s| NodeId::from_base58(s.trim()).unwrap())
                .collect();

            let dep_id = DeploymentId(Uuid::parse_str(&deployment_id)?);
            let escrow = settlement
                .create_escrow(dep_id, dev_id, provider_ids, token, epochs, amount_per_epoch)
                .await?;
            println!("Escrow created!");
            println!("Deployment ID: {}", escrow.deployment_id);
            println!("Total deposited: {} {}", escrow.total_deposited, escrow.token);
            println!("Epochs funded: {}", escrow.epochs_funded);
        }

        SettlementCommands::Settle { deployment_id, uptime } => {
            let dep_id = DeploymentId(Uuid::parse_str(&deployment_id)?);
            let metrics = vec![ProviderEpochMetrics {
                provider_node_id: developer,
                uptime_percentage: uptime,
                replication_lag_ms: 100,
                data_corrupted: false,
                malicious_detected: false,
                operations_processed: 1000,
                bytes_transferred: 1024,
            }];

            let payouts = settlement.settle_epoch(dep_id, metrics).await?;
            println!("Epoch settled!");
            for payout in payouts {
                println!(
                    "  Provider: {} - Payment: {} Slashed: {} SLA Met: {}",
                    payout.provider_node_id,
                    payout.payment_amount,
                    payout.slashed_amount,
                    payout.sla_met
                );
            }
        }
    }

    Ok(())
}
