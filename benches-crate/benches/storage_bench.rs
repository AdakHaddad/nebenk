use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use nebenk_storage::SqliteStore;
use nebenk_core::types::{ClusterId, NodeId, OperationEnvelope, Snapshot, SnapshotHeader};
use nebenk_identity::NodeIdentity;
use nebenk_core::traits::{LogStore, SnapshotStore};
use tempfile::tempdir;
use std::time::Duration;

fn bench_wal_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("wal_append");
    group.measurement_time(Duration::from_secs(10));
    
    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let dir = tempdir().unwrap();
            let cluster_id = ClusterId::new("bench-cluster");
            let mut store = SqliteStore::open(dir.path().join("store.db"), cluster_id.clone()).unwrap();
            
            let identity = NodeIdentity::generate();
            let node_id = identity.node_id();
            
            b.iter(|| {
                for i in 0..size {
                    let op = OperationEnvelope::new(
                        i as u64 + 1,
                        cluster_id.clone(),
                        node_id.clone(),
                        1000,
                        vec![0u8; 256],
                        identity.sign(&[i as u8]),
                    );
                    store.append_operation(&op).unwrap();
                }
            });
        });
    }
    group.finish();
}

fn bench_wal_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("wal_read");
    group.measurement_time(Duration::from_secs(10));
    
    let dir = tempdir().unwrap();
    let cluster_id = ClusterId::new("bench-cluster");
    let mut store = SqliteStore::open(dir.path().join("store.db"), cluster_id.clone()).unwrap();
    let identity = NodeIdentity::generate();
    let node_id = identity.node_id();
    
    // Pre-populate
    for i in 0..10000 {
        let op = OperationEnvelope::new(
            i as u64 + 1,
            cluster_id.clone(),
            node_id.clone(),
            1000,
            vec![0u8; 256],
            identity.sign(&[i as u8]),
        );
        store.append_operation(&op).unwrap();
    }
    
    group.bench_function("read_latest", |b| {
        b.iter(|| {
            let _ = <dyn LogStore>::latest_revision(&store).unwrap();
        });
    });
    
    group.bench_function("read_single", |b| {
        b.iter(|| {
            let _ = <dyn LogStore>::get_operation(&store, black_box(5000)).unwrap();
        });
    });
    
    group.bench_function("read_range", |b| {
        b.iter(|| {
            let _ = <dyn LogStore>::get_operations_since(&store, black_box(5000), 100).unwrap();
        });
    });
    
    group.finish();
}

fn bench_snapshot_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot_write");
    group.measurement_time(Duration::from_secs(10));
    
    for size in [1024, 10240, 102400, 1024000].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let dir = tempdir().unwrap();
            let cluster_id = ClusterId::new("bench-cluster");
            let mut store = SqliteStore::open(dir.path().join("store.db"), cluster_id.clone()).unwrap();
            
            let snapshot_data = vec![0u8; size];
            let state_hash = blake3::hash(&snapshot_data);
            let header = SnapshotHeader {
                format_version: 1,
                cluster_id: cluster_id.clone(),
                revision: 1,
                state_hash: *state_hash.as_bytes(),
                timestamp_ms: 1000,
                size_bytes: size as u64,
            };
            let snapshot = Snapshot::new(header, snapshot_data);
            
            b.iter(|| {
                <dyn SnapshotStore>::save_snapshot(&mut store, &snapshot).unwrap();
            });
        });
    }
    group.finish();
}

fn bench_snapshot_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot_read");
    group.measurement_time(Duration::from_secs(10));
    
    let dir = tempdir().unwrap();
    let cluster_id = ClusterId::new("bench-cluster");
    let mut store = SqliteStore::open(dir.path().join("store.db"), cluster_id.clone()).unwrap();
    
    // Pre-populate snapshots
    for rev in [100, 500, 1000, 5000, 10000].iter() {
        let snapshot_data = vec![0u8; 10240];
        let state_hash = blake3::hash(&snapshot_data);
        let header = SnapshotHeader {
            format_version: 1,
            cluster_id: cluster_id.clone(),
            revision: *rev,
            state_hash: *state_hash.as_bytes(),
            timestamp_ms: *rev as u64 * 1000,
            size_bytes: 10240,
        };
        let snapshot = Snapshot::new(header, snapshot_data);
        <dyn SnapshotStore>::save_snapshot(&mut store, &snapshot).unwrap();
    }
    
    group.bench_function("read_latest", |b| {
        b.iter(|| {
            let _ = <dyn SnapshotStore>::get_latest_snapshot(&store).unwrap();
        });
    });
    
    group.bench_function("read_at_revision", |b| {
        b.iter(|| {
            let _ = <dyn SnapshotStore>::get_snapshot_at(&store, black_box(5000)).unwrap();
        });
    });
    
    group.finish();
}

criterion_group!(
    storage_benches,
    bench_wal_append,
    bench_wal_read,
    bench_snapshot_write,
    bench_snapshot_read
);
criterion_main!(storage_benches);
