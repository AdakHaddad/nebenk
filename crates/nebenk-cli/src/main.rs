use clap::{Parser, Subcommand};
use nebenk_core::types::ClusterId;
use nebenk_identity::NodeIdentity;
use nebenk_runtime::{NodeConfig, NodeRuntime};
use nebenk_storage::SqliteStore;
use nebenk_wasm::{KvCommand, KvQuery, KvQueryResult, NativeKvEngine};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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
    Put {
        key: String,
        value: String,
    },

    /// Read a key value from the state machine
    Get {
        key: String,
    },

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
            let now_s = SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_secs();
            let token = identity.create_invitation(&cluster_id, now_s + expires_in);
            println!("Invitation Token (valid for {}s):", expires_in);
            println!("{}", token);
        }
    }

    Ok(())
}
