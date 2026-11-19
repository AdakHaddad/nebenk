use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use nebenk_core::types::{ClusterId, NodeId};
use nebenk_identity::NodeIdentity;
use nebenk_membership::Membership;
use nebenk_network::protocol::NodeRole;
use std::time::Duration;

fn bench_heartbeat_recording(c: &mut Criterion) {
    let mut group = c.benchmark_group("heartbeat_recording");
    group.measurement_time(Duration::from_secs(10));

    for node_count in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*node_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(node_count),
            node_count,
            |b, &node_count| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let cluster_id = ClusterId::new("bench-cluster");
                let local_id = NodeIdentity::generate().node_id();
                let (membership, _events) =
                    rt.block_on(async { Membership::new(cluster_id.clone(), local_id, 1000) });

                // Add nodes
                let peer_ids: Vec<NodeId> = (0..node_count)
                    .map(|_| NodeIdentity::generate().node_id())
                    .collect();
                for peer_id in &peer_ids {
                    rt.block_on(async {
                        membership
                            .upsert_member(
                                peer_id.clone(),
                                *peer_id.as_bytes(),
                                NodeRole::Replica,
                                None,
                            )
                            .await
                            .unwrap();
                    });
                }

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                b.iter(|| {
                    rt.block_on(async {
                        for peer_id in &peer_ids {
                            membership.record_heartbeat(peer_id.clone(), now).await;
                        }
                    });
                });
            },
        );
    }
    group.finish();
}

fn bench_failure_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("failure_detection");
    group.measurement_time(Duration::from_secs(10));

    for node_count in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*node_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(node_count),
            node_count,
            |b, &node_count| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let cluster_id = ClusterId::new("bench-cluster");
                let local_id = NodeIdentity::generate().node_id();
                let (membership, _events) =
                    rt.block_on(async { Membership::new(cluster_id.clone(), local_id, 1000) });

                // Add nodes with recent heartbeats
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                let peer_ids: Vec<NodeId> = (0..node_count)
                    .map(|_| NodeIdentity::generate().node_id())
                    .collect();
                for peer_id in &peer_ids {
                    rt.block_on(async {
                        membership
                            .upsert_member(
                                peer_id.clone(),
                                *peer_id.as_bytes(),
                                NodeRole::Replica,
                                None,
                            )
                            .await
                            .unwrap();
                        membership.record_heartbeat(peer_id.clone(), now).await;
                    });
                }

                b.iter(|| {
                    rt.block_on(async {
                        let _ = membership.run_failure_detection().await;
                    });
                });
            },
        );
    }
    group.finish();
}

fn bench_membership_events(c: &mut Criterion) {
    let mut group = c.benchmark_group("membership_events");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("node_join_leave", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let cluster_id = ClusterId::new("bench-cluster");
        let local_id = NodeIdentity::generate().node_id();
        let (membership, mut events) =
            rt.block_on(async { Membership::new(cluster_id.clone(), local_id, 1000) });

        b.iter(|| {
            rt.block_on(async {
                let peer_id = NodeIdentity::generate().node_id();
                membership
                    .upsert_member(
                        peer_id.clone(),
                        *peer_id.as_bytes(),
                        NodeRole::Replica,
                        None,
                    )
                    .await
                    .unwrap();

                // Drain events
                while events.try_recv().is_ok() {}
            });
        });
    });

    group.bench_function("sync_marking", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let cluster_id = ClusterId::new("bench-cluster");
        let local_id = NodeIdentity::generate().node_id();
        let (membership, _events) =
            rt.block_on(async { Membership::new(cluster_id.clone(), local_id, 1000) });

        let peer_id = NodeIdentity::generate().node_id();
        rt.block_on(async {
            membership
                .upsert_member(
                    peer_id.clone(),
                    *peer_id.as_bytes(),
                    NodeRole::Replica,
                    None,
                )
                .await
                .unwrap();
        });

        b.iter(|| {
            rt.block_on(async {
                membership
                    .mark_synced(&peer_id, black_box(1000))
                    .await
                    .unwrap();
            });
        });
    });

    group.finish()
}

fn bench_primary_election(c: &mut Criterion) {
    let mut group = c.benchmark_group("primary_election");
    group.measurement_time(Duration::from_secs(10));

    for node_count in [3, 5, 10, 50].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(node_count),
            node_count,
            |b, &node_count| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let cluster_id = ClusterId::new("bench-cluster");
                let local_id = NodeIdentity::generate().node_id();
                let (membership, _events) =
                    rt.block_on(async { Membership::new(cluster_id.clone(), local_id, 1000) });

                // Add primary + replicas
                let primary_id = NodeIdentity::generate().node_id();
                rt.block_on(async {
                    membership
                        .upsert_member(
                            primary_id.clone(),
                            *primary_id.as_bytes(),
                            NodeRole::Primary,
                            None,
                        )
                        .await
                        .unwrap();
                    membership.mark_synced(&primary_id, 1000).await.unwrap();
                });

                for i in 0..node_count - 1 {
                    let peer_id = NodeIdentity::generate().node_id();
                    rt.block_on(async {
                        membership
                            .upsert_member(
                                peer_id.clone(),
                                *peer_id.as_bytes(),
                                NodeRole::Replica,
                                None,
                            )
                            .await
                            .unwrap();
                        membership
                            .mark_synced(&peer_id, 900 + i as u64)
                            .await
                            .unwrap();
                    });
                }

                b.iter(|| {
                    rt.block_on(async {
                        let _ = membership.handle_primary_failure().await.unwrap();
                    });
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    membership_benches,
    bench_heartbeat_recording,
    bench_failure_detection,
    bench_membership_events,
    bench_primary_election
);
criterion_main!(membership_benches);
