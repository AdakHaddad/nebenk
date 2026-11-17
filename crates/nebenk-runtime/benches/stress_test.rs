use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nebenk_core::types::{ClusterId, NodeId};
use nebenk_identity::NodeIdentity;
use nebenk_runtime::{NodeConfig, NodeRuntime};
use nebenk_storage::SqliteStore;
use nebenk_wasm::{KvCommand, KvQuery, NativeKvEngine};
use std::sync::Arc;
use tokio::runtime::Runtime;

fn bench_single_node_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("single_node_put_throughput");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(2));
    group.warm_up_time(std::time::Duration::from_secs(1));

    group.bench_function("single_node_put_throughput", |b| {
        b.iter(|| {
            rt.block_on(async {
                let identity = NodeIdentity::generate();
                let cluster = ClusterId::new("bench-cluster");
                let storage = SqliteStore::open_in_memory(cluster.clone()).unwrap();
                let engine = Box::new(NativeKvEngine::new());

                let config = NodeConfig {
                    cluster_id: cluster,
                    is_primary: true,
                    snapshot_interval: 1000,
                };

                let mut runtime = NodeRuntime::new(identity, config, storage, engine);
                runtime.recover().await.unwrap();

                // Submit 100 puts (reduced from 1000)
                for i in 0..100 {
                    let cmd = KvCommand::Put {
                        key: format!("key{}", i),
                        value: vec![0u8; 100],
                    };
                    let payload = serde_json::to_vec(&cmd).unwrap();
                    runtime.submit_command(payload, i as u64).await.unwrap();
                }

                black_box(runtime.current_revision())
            })
        })
    });
    group.finish();
}

fn bench_snapshot_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("snapshot_creation");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(2));
    group.warm_up_time(std::time::Duration::from_secs(1));

    group.bench_function("snapshot_creation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let identity = NodeIdentity::generate();
                let cluster = ClusterId::new("bench-cluster");
                let storage = SqliteStore::open_in_memory(cluster.clone()).unwrap();
                let engine = Box::new(NativeKvEngine::new());

                let config = NodeConfig {
                    cluster_id: cluster,
                    is_primary: true,
                    snapshot_interval: 0,
                };

                let mut runtime = NodeRuntime::new(identity, config, storage, engine);
                runtime.recover().await.unwrap();

                // Add some data first
                for i in 0..100 {
                    let cmd = KvCommand::Put {
                        key: format!("key{}", i),
                        value: vec![0u8; 1024],
                    };
                    let payload = serde_json::to_vec(&cmd).unwrap();
                    runtime.submit_command(payload, i as u64).await.unwrap();
                }

                // Benchmark snapshot
                runtime.create_snapshot(123456).await.unwrap()
            })
        })
    });
    group.finish();
}

fn bench_recovery(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("recovery_from_wal");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(2));
    group.warm_up_time(std::time::Duration::from_secs(1));

    group.bench_function("recovery_from_wal", |b| {
        b.iter(|| {
            rt.block_on(async {
                let identity = NodeIdentity::generate();
                let cluster = ClusterId::new("bench-cluster");
                let storage = SqliteStore::open_in_memory(cluster.clone()).unwrap();
                let engine = Box::new(NativeKvEngine::new());

                let config = NodeConfig {
                    cluster_id: cluster.clone(),
                    is_primary: true,
                    snapshot_interval: 10,
                };

                let mut runtime = NodeRuntime::new(identity, config.clone(), storage, engine);
                runtime.recover().await.unwrap();

                // Add data with snapshots
                for i in 0..100 {
                    let cmd = KvCommand::Put {
                        key: format!("key{}", i),
                        value: vec![0u8; 100],
                    };
                    let payload = serde_json::to_vec(&cmd).unwrap();
                    runtime.submit_command(payload, i as u64).await.unwrap();
                }

                // Create new runtime and recover
                let identity2 = NodeIdentity::generate();
                let storage2 = SqliteStore::open_in_memory(cluster.clone()).unwrap();
                let engine2 = Box::new(NativeKvEngine::new());
                let config2 = NodeConfig {
                    cluster_id: cluster,
                    is_primary: true,
                    snapshot_interval: 10,
                };
                let mut runtime2 = NodeRuntime::new(identity2, config2, storage2, engine2);
                runtime2.recover().await.unwrap();

                black_box(runtime2.current_revision())
            })
        })
    });
    group.finish();
}

fn bench_concurrent_queries(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // Pre-populate
    let runtime = rt.block_on(async {
        let identity = NodeIdentity::generate();
        let cluster = ClusterId::new("bench-cluster");
        let storage = SqliteStore::open_in_memory(cluster.clone()).unwrap();
        let engine = Box::new(NativeKvEngine::new());

        let config = NodeConfig {
            cluster_id: cluster,
            is_primary: true,
            snapshot_interval: 1000,
        };

        let mut runtime = NodeRuntime::new(identity, config, storage, engine);
        runtime.recover().await.unwrap();

        for i in 0..1000 {
            let cmd = KvCommand::Put {
                key: format!("key{}", i),
                value: vec![i as u8; 100],
            };
            let payload = serde_json::to_vec(&cmd).unwrap();
            runtime.submit_command(payload, i as u64).await.unwrap();
        }

        Arc::new(runtime)
    });

    let mut group = c.benchmark_group("concurrent_queries");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(2));
    group.warm_up_time(std::time::Duration::from_secs(1));

    let concurrency_levels = [1, 10, 50];
    for c_level in concurrency_levels {
        let id = BenchmarkId::from_parameter(c_level);
        group.bench_with_input(id, &c_level, |b, &c_level| {
            let c_val = c_level;
            let runtime_clone = runtime.clone();
            b.iter(|| {
                rt.block_on(async {
                    let mut handles = vec![];

                    for _ in 0..c_val {
                        let rt = runtime_clone.clone();
                        handles.push(tokio::spawn(async move {
                            let query = KvQuery::Get {
                                key: format!("key{}", fastrand::usize(0..1000)),
                            };
                            let payload = serde_json::to_vec(&query).unwrap();
                            rt.query(&payload).unwrap()
                        }));
                    }

                    for h in handles {
                        h.await.unwrap();
                    }
                })
            });
        });
    }
    group.finish();
}

fn bench_membership_failure_detection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("membership_failure_detection");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(2));
    group.warm_up_time(std::time::Duration::from_secs(1));

    group.bench_function("membership_failure_detection_100_nodes", |b| {
        use nebenk_core::types::ClusterId;
        use nebenk_membership::Membership;
        use nebenk_network::NodeRole;

        b.iter(|| {
            rt.block_on(async {
                let cluster = ClusterId::new("bench-cluster");
                let local_id = NodeId::from_bytes([1u8; 32]);

                let (membership, _events) = Membership::new(cluster, local_id, 1000);

                // Add 100 nodes
                for i in 2..102 {
                    let mut bytes = [0u8; 32];
                    bytes[0] = i as u8;
                    let node_id = NodeId::from_bytes(bytes);
                    membership
                        .upsert_member(node_id, bytes, NodeRole::Replica, None)
                        .await
                        .unwrap();

                    // Send heartbeats
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;
                    for j in 0..5 {
                        membership.record_heartbeat(node_id, now + j * 1000).await;
                    }
                }

                // Run failure detection
                membership.run_failure_detection().await
            })
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_single_node_throughput,
    bench_snapshot_creation,
    bench_recovery,
    bench_concurrent_queries,
    bench_membership_failure_detection
);
criterion_main!(benches);
