//! Marketplace benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nebenk_core::types::{ClusterId, NodeId};
use nebenk_identity::NodeIdentity;
use nebenk_marketplace::client::{ClientConfig, MarketplaceClient};
use nebenk_marketplace::provider::{MarketplaceProvider, ProviderConfig};
use nebenk_marketplace::registry::{
    MarketplaceRegistry, ProviderFilter, ProviderListRequest, RegistryConfig,
};
use nebenk_marketplace::reputation::{BenchmarkResult, ReputationConfig, ReputationEngine};
use nebenk_marketplace::settlement::{ProviderEpochMetrics, SettlementConfig, SettlementEngine};
use nebenk_marketplace::types::*;
use std::sync::Arc;
use tokio::runtime::Runtime;

fn benchmark_provider_registration(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("provider_registration", |b| {
        b.iter(|| {
            rt.block_on(async {
                let identity = Arc::new(NodeIdentity::generate());
                let config = ProviderConfig::default();
                let provider = MarketplaceProvider::new(config, identity);

                let capabilities = DeviceCapabilities {
                    cpu_cores: 8,
                    cpu_arch: "x86_64".to_string(),
                    os: "linux".to_string(),
                    memory_gb: 32,
                    storage_gb: 500,
                    storage_type: "nvme".to_string(),
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
                    cpu_per_hour: 10,
                    memory_per_gb_hour: 1,
                    storage_per_gb_hour: 1,
                    gpu_per_hour: None,
                    egress_per_gb: 5,
                    min_epoch_hours: 1,
                };

                let stake = StakeInfo {
                    amount: 1000,
                    token: "USDC".to_string(),
                    lock_duration_secs: 86400 * 30,
                    contract_address: None,
                };

                provider
                    .register(capabilities, pricing, stake)
                    .await
                    .unwrap()
            })
        })
    });
}

fn benchmark_advertisement_signing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let identity = Arc::new(NodeIdentity::generate());
    let config = ProviderConfig::default();
    let provider = MarketplaceProvider::new(config, identity.clone());

    let capabilities = DeviceCapabilities {
        cpu_cores: 8,
        cpu_arch: "x86_64".to_string(),
        os: "linux".to_string(),
        memory_gb: 32,
        storage_gb: 500,
        storage_type: "nvme".to_string(),
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
        cpu_per_hour: 10,
        memory_per_gb_hour: 1,
        storage_per_gb_hour: 1,
        gpu_per_hour: None,
        egress_per_gb: 5,
        min_epoch_hours: 1,
    };

    let stake = StakeInfo {
        amount: 1000,
        token: "USDC".to_string(),
        lock_duration_secs: 86400 * 30,
        contract_address: None,
    };

    let adv = rt.block_on(async {
        provider
            .register(capabilities, pricing, stake)
            .await
            .unwrap()
    });

    c.bench_function("advertisement_signing", |b| {
        b.iter(|| black_box(adv.signing_data()))
    });

    c.bench_function("advertisement_verification", |b| {
        b.iter(|| black_box(adv.verify()))
    });
}

fn benchmark_client_discovery(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let client_config = ClientConfig::default();
    let identity = Arc::new(NodeIdentity::generate());
    let client = MarketplaceClient::new(client_config, identity);

    // Add some providers
    rt.block_on(async {
        for i in 0..10 {
            let provider_id = NodeIdentity::generate();
            let capabilities = DeviceCapabilities {
                cpu_cores: (i + 1) * 2,
                cpu_arch: "x86_64".to_string(),
                os: "linux".to_string(),
                memory_gb: (i + 1) * 8,
                storage_gb: 100,
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
                cpu_per_hour: 10 + i as u64 * 2,
                memory_per_gb_hour: 1,
                storage_per_gb_hour: 1,
                gpu_per_hour: None,
                egress_per_gb: 5,
                min_epoch_hours: 1,
            };
            let stake = StakeInfo {
                amount: 1000,
                token: "USDC".to_string(),
                lock_duration_secs: 86400 * 30,
                contract_address: None,
            };

            let mut adv = ProviderAdvertisement::new(
                provider_id.node_id(),
                ClusterId::new("test"),
                capabilities,
                pricing,
                stake,
            );
            let signing_data = adv.signing_data();
            adv.signature = provider_id.sign(&signing_data);

            client.handle_advertisement(adv).await.unwrap();
        }
    });

    c.bench_function("discover_providers", |b| {
        b.iter(|| {
            rt.block_on(async {
                let requirements = ResourceRequirements {
                    min_cpu_cores: 4,
                    min_memory_gb: 8,
                    min_storage_gb: 50,
                    required_arch: vec!["x86_64".to_string()],
                    required_features: vec!["wasm".to_string()],
                    gpu_required: false,
                };
                let prefs = ClusterPreferences::default();
                client
                    .discover_providers(&requirements, &prefs)
                    .await
                    .unwrap()
            })
        })
    });
}

fn benchmark_reputation_scoring(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = ReputationConfig::default();
    let engine = ReputationEngine::new(config);

    c.bench_function("reputation_init", |b| {
        b.iter(|| {
            rt.block_on(async {
                let node_id = NodeId::from_bytes([1u8; 32]);
                engine.init_provider(node_id).await
            })
        })
    });

    // Pre-initialize for scoring benchmark
    let node_id = rt.block_on(async {
        let id = NodeId::from_bytes([1u8; 32]);
        engine.init_provider(id).await;
        id
    });

    rt.block_on(async {
        for i in 0..10 {
            engine.record_uptime(node_id, i, 99.5).await;
            engine.record_sla(node_id, i, true, 99.5, 100).await;
        }
    });

    c.bench_function("reputation_calculate_score", |b| {
        b.iter(|| rt.block_on(async { engine.get_score(black_box(node_id)).await }))
    });

    c.bench_function("reputation_with_benchmark", |b| {
        b.iter(|| {
            rt.block_on(async {
                engine
                    .record_benchmark(BenchmarkResult {
                        node_id,
                        write_throughput_ops_sec: 2000.0,
                        read_throughput_ops_sec: 1000000.0,
                        snapshot_time_ms: 50,
                        recovery_time_ms: 100,
                        data_integrity: true,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    })
                    .await;
                engine.get_score(black_box(node_id)).await
            })
        })
    });
}

fn benchmark_settlement_calculation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = SettlementConfig::default();
    let engine = SettlementEngine::new(config);

    let deployment_id = DeploymentId::new();
    let developer = NodeId::from_bytes([1u8; 32]);
    let providers = vec![NodeId::from_bytes([2u8; 32])];

    rt.block_on(async {
        engine
            .create_escrow(
                deployment_id,
                developer,
                providers,
                "USDC".to_string(),
                10,
                100,
            )
            .await
            .unwrap();
    });

    let metrics = vec![ProviderEpochMetrics {
        provider_node_id: NodeId::from_bytes([2u8; 32]),
        uptime_percentage: 99.5,
        replication_lag_ms: 100,
        data_corrupted: false,
        malicious_detected: false,
        operations_processed: 1000,
        bytes_transferred: 1024,
    }];

    c.bench_function("settlement_full_cycle", |b| {
        b.iter(|| {
            rt.block_on(async {
                engine
                    .settle_epoch(black_box(deployment_id), black_box(metrics.clone()))
                    .await
                    .unwrap()
            })
        })
    });

    c.bench_function("settlement_slashing", |b| {
        b.iter(|| {
            rt.block_on(async {
                let poor_metrics = vec![ProviderEpochMetrics {
                    provider_node_id: NodeId::from_bytes([2u8; 32]),
                    uptime_percentage: 95.0,
                    replication_lag_ms: 100,
                    data_corrupted: false,
                    malicious_detected: false,
                    operations_processed: 1000,
                    bytes_transferred: 1024,
                }];
                engine
                    .settle_epoch(black_box(deployment_id), black_box(poor_metrics))
                    .await
                    .unwrap()
            })
        })
    });
}

fn benchmark_registry_filter(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let registry = MarketplaceRegistry::new(RegistryConfig::default());

    // Add providers
    rt.block_on(async {
        for i in 0..20 {
            let identity = NodeIdentity::generate();
            let capabilities = DeviceCapabilities {
                cpu_cores: (i + 1) * 2,
                cpu_arch: "x86_64".to_string(),
                os: "linux".to_string(),
                memory_gb: (i + 1) * 8,
                storage_gb: 100,
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
                cpu_per_hour: 10 + i as u64 * 2,
                memory_per_gb_hour: 1,
                storage_per_gb_hour: 1,
                gpu_per_hour: None,
                egress_per_gb: 5,
                min_epoch_hours: 1,
            };
            let stake = StakeInfo {
                amount: 1000,
                token: "USDC".to_string(),
                lock_duration_secs: 86400 * 30,
                contract_address: None,
            };

            let mut adv = ProviderAdvertisement::new(
                identity.node_id(),
                ClusterId::new("test"),
                capabilities,
                pricing,
                stake,
            );
            let signing_data = adv.signing_data();
            adv.signature = identity.sign(&signing_data);

            registry.handle_advertisement(adv).await.unwrap();
        }
    });

    let request = ProviderListRequest {
        requester: NodeId::from_bytes([99u8; 32]),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        filter: Some(ProviderFilter {
            min_cpu_cores: Some(6),
            min_memory_gb: Some(16),
            required_arch: Some(vec!["x86_64".to_string()]),
            min_reputation: Some(25),
            max_price_cpu: Some(15),
            ..Default::default()
        }),
    };

    c.bench_function("registry_filter", |b| {
        b.iter(|| {
            rt.block_on(async {
                registry
                    .handle_list_request(black_box(request.clone()))
                    .await
                    .unwrap()
            })
        })
    });
}

criterion_group!(
    benches,
    benchmark_provider_registration,
    benchmark_advertisement_signing,
    benchmark_client_discovery,
    benchmark_reputation_scoring,
    benchmark_settlement_calculation,
    benchmark_registry_filter,
);

criterion_main!(benches);
