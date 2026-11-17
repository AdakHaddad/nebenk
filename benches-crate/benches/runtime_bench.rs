use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use nebenk_runtime::{NodeRuntime, NodeConfig};
use nebenk_identity::NodeIdentity;
use nebenk_storage::SqliteStore;
use nebenk_wasm::NativeKvEngine;
use nebenk_core::types::{ClusterId, OperationEnvelope, NodeId};
use nebenk_core::traits::StateEngine;
use tempfile::tempdir;
use std::time::Duration;

fn bench_runtime_command_submit(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime_command_submit");
    group.measurement_time(Duration::from_secs(15));
    
    for ops in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*ops as u64));
        group.bench_with_input(BenchmarkId::from_parameter(ops), ops, |b, &ops| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let dir = tempdir().unwrap();
            let cluster_id = ClusterId::new("bench-cluster");
            let storage = SqliteStore::open(dir.path().join("store.db"), cluster_id.clone()).unwrap();
            let engine = Box::new(NativeKvEngine::new());
            
            let identity = NodeIdentity::generate();
            let config = NodeConfig {
                cluster_id: cluster_id.clone(),
                is_primary: true,
                snapshot_interval: 0,
            };
            
            let mut runtime = rt.block_on(async {
                NodeRuntime::new(identity, config, storage, engine)
            });
            rt.block_on(async { runtime.recover().await });
            
            b.iter(|| {
                rt.block_on(async {
                    for i in 0..ops {
                        let payload = format!("PUT key{} value{}", i, i).into_bytes();
                        runtime.submit_command(payload, 1000).await.unwrap();
                    }
                });
            });
        });
    }
    group.finish();
}

fn bench_runtime_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime_query");
    group.measurement_time(Duration::from_secs(10));
    
    let rt = tokio::runtime::Runtime::new().unwrap();
    let dir = tempdir().unwrap();
    let cluster_id = ClusterId::new("bench-cluster");
    let storage = SqliteStore::open(dir.path().join("store.db"), cluster_id.clone()).unwrap();
    let engine = Box::new(NativeKvEngine::new());
    
    let identity = NodeIdentity::generate();
    let config = NodeConfig {
        cluster_id: cluster_id.clone(),
        is_primary: true,
        snapshot_interval: 0,
    };
    
    let mut runtime = rt.block_on(async {
        NodeRuntime::new(identity, config, storage, engine)
    });
    rt.block_on(async { runtime.recover().await });
    
    // Pre-populate 10000 keys
    for i in 0..10000 {
        let payload = format!("PUT key{} value{}", i, i).into_bytes();
        rt.block_on(async { runtime.submit_command(payload, 1000).await });
    }
    
    group.bench_function("query_existing_key", |b| {
        let query = br#"{"Get":{"key":"key5000"}}"#;
        b.iter(|| {
            let _ = runtime.query(black_box(query)).unwrap();
        });
    });
    
    group.bench_function("query_nonexistent_key", |b| {
        let query = br#"{"Get":{"key":"nonexistent"}}"#;
        b.iter(|| {
            let _ = runtime.query(black_box(query)).unwrap();
        });
    });
    
    group.bench_function("query_list_keys", |b| {
        let query = br#"{"ListKeys":{}}"#;
        b.iter(|| {
            let _ = runtime.query(black_box(query)).unwrap();
        });
    });
    
    group.finish();
}

fn bench_runtime_snapshot_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot_creation");
    group.measurement_time(Duration::from_secs(15));
    
    for key_count in [100, 1000, 10000, 100000].iter() {
        group.throughput(Throughput::Elements(*key_count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(key_count), key_count, |b, &key_count| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let dir = tempdir().unwrap();
            let cluster_id = ClusterId::new("bench-cluster");
            let storage = SqliteStore::open(dir.path().join("store.db"), cluster_id.clone()).unwrap();
            let engine = Box::new(NativeKvEngine::new());
            
            let identity = NodeIdentity::generate();
            let config = NodeConfig {
                cluster_id: cluster_id.clone(),
                is_primary: true,
                snapshot_interval: 0,
            };
            
            let mut runtime = rt.block_on(async {
                NodeRuntime::new(identity, config, storage, engine)
            });
            rt.block_on(async { runtime.recover().await });
            
            // Pre-populate
            for i in 0..key_count {
                let payload = format!("PUT key{} value{}", i, i).into_bytes();
                rt.block_on(async { runtime.submit_command(payload, 1000).await });
            }
            
            b.iter(|| {
                rt.block_on(async {
                    runtime.create_snapshot(2000).await.unwrap();
                });
            });
        });
    }
    group.finish();
}

fn bench_recovery(c: &mut Criterion) {
    let mut group = c.benchmark_group("recovery");
    group.measurement_time(Duration::from_secs(15));
    
    for op_count in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*op_count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(op_count), op_count, |b, &op_count| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let dir = tempdir().unwrap();
            let cluster_id = ClusterId::new("bench-cluster");
            
            // First, create a runtime and populate it
            {
                let storage = SqliteStore::open(dir.path().join("store.db"), cluster_id.clone()).unwrap();
                let engine = Box::new(NativeKvEngine::new());
                
                let identity = NodeIdentity::generate();
                let config = NodeConfig {
                    cluster_id: cluster_id.clone(),
                    is_primary: true,
                    snapshot_interval: 100,
                };
                
                let mut runtime = rt.block_on(async {
                    NodeRuntime::new(identity, config, storage, engine)
                });
                rt.block_on(async { runtime.recover().await });
                
                for i in 0..op_count {
                    let payload = format!("PUT key{} value{}", i, i).into_bytes();
                    rt.block_on(async { runtime.submit_command(payload, 1000).await });
                }
            }
            
            // Now benchmark recovery
            b.iter(|| {
                let storage = SqliteStore::open(dir.path().join("store.db"), cluster_id.clone()).unwrap();
                let engine = Box::new(NativeKvEngine::new());
                
                let identity = NodeIdentity::generate();
                let config = NodeConfig {
                    cluster_id: cluster_id.clone(),
                    is_primary: true,
                    snapshot_interval: 100,
                };
                
                let mut runtime = rt.block_on(async {
                    NodeRuntime::new(identity, config, storage, engine)
                });
                rt.block_on(async { runtime.recover().await });
            });
        });
    }
    group.finish();
}

fn bench_native_kv_engine(c: &mut Criterion) {
    let mut group = c.benchmark_group("native_kv_engine");
    group.measurement_time(Duration::from_secs(10));
    
    for ops in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*ops as u64));
        group.bench_with_input(BenchmarkId::from_parameter(ops), ops, |b, &ops| {
            b.iter(|| {
                let mut engine = NativeKvEngine::new();
                <dyn StateEngine>::init(&mut engine, b"").unwrap();
                
                for i in 0..ops {
                    let op = OperationEnvelope::new(
                        i as u64 + 1,
                        ClusterId::new("bench"),
                        NodeId::from_bytes([0u8; 32]),
                        1000,
                        format!("PUT key{} value{}", i, i).into_bytes(),
                        vec![0u8; 64],
                    );
                    <dyn StateEngine>::apply(&mut engine, &op).unwrap();
                }
            });
        });
    }
    group.finish();
}

criterion_group!(
    runtime_benches,
    bench_runtime_command_submit,
    bench_runtime_query,
    bench_runtime_snapshot_creation,
    bench_recovery,
    bench_native_kv_engine
);
criterion_main!(runtime_benches);
