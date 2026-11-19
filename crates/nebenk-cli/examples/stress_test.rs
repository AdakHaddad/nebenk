// Stress test for NEBENK - run with: cargo run --example stress_test
use nebenk_core::types::ClusterId;
use nebenk_identity::NodeIdentity;
use nebenk_runtime::{NodeConfig, NodeRuntime};
use nebenk_storage::SqliteStore;
use nebenk_wasm::{KvCommand, KvQuery, NativeKvEngine};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::task;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== NEBENK Stress Test Suite ===\n");

    // Test 1: Single-threaded baseline (in-memory)
    println!("--- Test 1: Single-threaded baseline (in-memory) ---");
    run_write_test(5_000, 1, true).await?;

    // Test 2: 4-threaded (in-memory) - mutex contention
    println!("\n--- Test 2: 4-threaded (in-memory) ---");
    run_write_test(5_000, 4, true).await?;

    // Test 3: Single-threaded baseline (file-based)
    println!("\n--- Test 3: Single-threaded baseline (file-based) ---");
    run_write_test(5_000, 1, false).await?;

    // Test 4: Read performance
    println!("\n--- Test 4: Read performance (in-memory, 5k reads) ---");
    run_read_test(5_000).await?;

    // Test 5: Snapshot + Recovery
    println!("\n--- Test 5: Snapshot + Recovery (file-based, 5k ops) ---");
    run_snapshot_recovery_test(5_000).await?;

    Ok(())
}

async fn run_write_test(num_ops: usize, num_threads: usize, in_memory: bool) -> anyhow::Result<()> {
    let ops_per_thread = num_ops / num_threads;

    let identity = NodeIdentity::generate();
    let cluster = ClusterId::new("stress-test");
    let storage = if in_memory {
        SqliteStore::open_in_memory(cluster.clone())?
    } else {
        let data_dir = std::env::temp_dir().join(format!("nebenk-stress-{}", std::process::id()));
        std::fs::create_dir_all(&data_dir)?;
        SqliteStore::open(data_dir.join("state.db"), cluster.clone())?
    };
    let engine = Box::new(NativeKvEngine::new());

    let config = NodeConfig {
        cluster_id: cluster.clone(),
        is_primary: true,
        snapshot_interval: 5000,
    };

    let mut runtime = NodeRuntime::new(identity, config, storage, engine);
    runtime.recover().await?;

    let runtime = Arc::new(Mutex::new(runtime));
    let start = Instant::now();

    let mut handles = vec![];
    for t in 0..num_threads {
        let rt = runtime.clone();
        let handle = task::spawn(async move {
            for i in 0..ops_per_thread {
                let key = format!("key-{}-{}", t, i);
                let cmd = KvCommand::Put {
                    key: key.clone(),
                    value: format!("value-{}", i).into_bytes(),
                };
                let payload = serde_json::to_vec(&cmd).unwrap();
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                let mut rt_locked = rt.lock().await;
                rt_locked.submit_command(payload, ts).await.unwrap();
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.await.unwrap();
    }

    let elapsed = start.elapsed();
    let throughput = num_ops as f64 / elapsed.as_secs_f64();

    let storage_type = if in_memory { "in-memory" } else { "file-based" };
    println!("Storage: {}", storage_type);
    println!("Threads: {}", num_threads);
    println!("Operations: {}", num_ops);
    println!("Time: {:.2?}", elapsed);
    println!("Throughput: {:.0} ops/sec", throughput);
    println!(
        "Latency (avg): {:.2} µs/op",
        elapsed.as_micros() as f64 / num_ops as f64
    );

    Ok(())
}

async fn run_read_test(num_ops: usize) -> anyhow::Result<()> {
    let identity = NodeIdentity::generate();
    let cluster = ClusterId::new("stress-test");
    let storage = SqliteStore::open_in_memory(cluster.clone())?;
    let engine = Box::new(NativeKvEngine::new());

    let config = NodeConfig {
        cluster_id: cluster.clone(),
        is_primary: true,
        snapshot_interval: 5000,
    };

    let mut runtime = NodeRuntime::new(identity, config, storage, engine);
    runtime.recover().await?;

    // Pre-populate
    for i in 0..num_ops {
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

    let runtime = Arc::new(Mutex::new(runtime));
    let read_start = Instant::now();

    let mut read_count = 0;
    for i in 0..num_ops {
        let key = format!("key-{}", i);
        let query = KvQuery::Get { key };
        let query_bytes = serde_json::to_vec(&query).unwrap();
        let rt = runtime.lock().await;
        let _ = rt.query(&query_bytes).unwrap();
        read_count += 1;
    }

    let read_elapsed = read_start.elapsed();
    let read_throughput = read_count as f64 / read_elapsed.as_secs_f64();

    println!("Total reads: {}", read_count);
    println!("Time: {:.2?}", read_elapsed);
    println!("Throughput: {:.0} ops/sec", read_throughput);
    println!(
        "Latency (avg): {:.2} µs/op",
        read_elapsed.as_micros() as f64 / read_count as f64
    );

    Ok(())
}

async fn run_snapshot_recovery_test(num_ops: usize) -> anyhow::Result<()> {
    let data_dir = std::env::temp_dir().join(format!("nebenk-stress-{}", std::process::id()));
    std::fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("state.db");

    let identity = NodeIdentity::generate();
    let cluster = ClusterId::new("stress-test");
    let storage = SqliteStore::open(db_path.clone(), cluster.clone())?;
    let engine = Box::new(NativeKvEngine::new());

    let config = NodeConfig {
        cluster_id: cluster.clone(),
        is_primary: true,
        snapshot_interval: 5000,
    };

    let mut runtime = NodeRuntime::new(identity, config, storage, engine);
    runtime.recover().await?;

    // Write operations
    let start = Instant::now();
    for i in 0..num_ops {
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
    let write_elapsed = start.elapsed();

    // Snapshot
    let snap_start = Instant::now();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let snap = runtime.create_snapshot(ts).await.unwrap();
    let snap_elapsed = snap_start.elapsed();

    // Recovery
    let recovery_start = Instant::now();
    let identity2 = NodeIdentity::generate();
    let storage2 = SqliteStore::open(db_path, cluster.clone())?;
    let engine2 = Box::new(NativeKvEngine::new());
    let config2 = NodeConfig {
        cluster_id: cluster.clone(),
        is_primary: true,
        snapshot_interval: 5000,
    };
    let mut runtime2 = NodeRuntime::new(identity2, config2, storage2, engine2);
    let recovered_rev = runtime2.recover().await.unwrap();
    let recovery_elapsed = recovery_start.elapsed();

    // Verify
    let query = KvQuery::Get {
        key: "key-0".to_string(),
    };
    let query_bytes = serde_json::to_vec(&query).unwrap();
    let out_bytes = runtime2.query(&query_bytes).unwrap();
    let res: nebenk_wasm::KvQueryResult = serde_json::from_slice(&out_bytes).unwrap();
    let verified = matches!(res, nebenk_wasm::KvQueryResult::Value(Some(v)) if v == b"value-0");

    println!(
        "Write {} ops: {:.2?} ({:.0} ops/sec)",
        num_ops,
        write_elapsed,
        num_ops as f64 / write_elapsed.as_secs_f64()
    );
    println!(
        "Snapshot rev {}: {:.2?} ({} bytes)",
        snap.header.revision, snap_elapsed, snap.header.size_bytes
    );
    println!("Recovery rev {}: {:.2?}", recovered_rev, recovery_elapsed);
    println!("Data integrity: {}", if verified { "PASS" } else { "FAIL" });

    Ok(())
}
