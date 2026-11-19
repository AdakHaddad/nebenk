// Quick test for NEBENK - run with: cargo run --example quick_test
use nebenk_core::types::ClusterId;
use nebenk_identity::NodeIdentity;
use nebenk_runtime::{NodeConfig, NodeRuntime};
use nebenk_storage::SqliteStore;
use nebenk_wasm::{KvCommand, KvQuery, NativeKvEngine};
use std::time::Instant;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== NEBENK Quick Test ===\n");

    let identity = NodeIdentity::generate();
    let cluster = ClusterId::new("quick-test");
    let storage = SqliteStore::open_in_memory(cluster.clone())?;
    let engine = Box::new(NativeKvEngine::new());

    let config = NodeConfig {
        cluster_id: cluster.clone(),
        is_primary: true,
        snapshot_interval: 5000,
    };

    let mut runtime = NodeRuntime::new(identity, config, storage, engine);
    runtime.recover().await?;

    // Write 1000 ops
    let start = Instant::now();
    for i in 0..1000 {
        let cmd = KvCommand::Put {
            key: format!("key-{}", i),
            value: format!("value-{}", i).into_bytes(),
        };
        let payload = serde_json::to_vec(&cmd).unwrap();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        runtime.submit_command(payload, ts).await.unwrap();
    }
    let elapsed = start.elapsed();
    println!(
        "Write 1000 ops: {:.2?} ({:.0} ops/sec)",
        elapsed,
        1000.0 / elapsed.as_secs_f64()
    );

    // Read 1000 ops
    let read_start = Instant::now();
    for i in 0..1000 {
        let query = KvQuery::Get {
            key: format!("key-{}", i),
        };
        let query_bytes = serde_json::to_vec(&query).unwrap();
        let _ = runtime.query(&query_bytes).unwrap();
    }
    let read_elapsed = read_start.elapsed();
    println!(
        "Read 1000 ops: {:.2?} ({:.0} ops/sec)",
        read_elapsed,
        1000.0 / read_elapsed.as_secs_f64()
    );

    // Snapshot
    let snap_start = Instant::now();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let snap = runtime.create_snapshot(ts).await.unwrap();
    let snap_elapsed = snap_start.elapsed();
    println!(
        "Snapshot: {:.2?} (rev {}, {} bytes)",
        snap_elapsed, snap.header.revision, snap.header.size_bytes
    );

    Ok(())
}
