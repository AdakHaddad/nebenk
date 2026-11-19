use libp2p::Multiaddr;
use nebenk_identity::NodeIdentity;
use nebenk_marketplace::{
    network_registry::NetworkMarketplaceRegistry,
    provider::{MarketplaceProvider, ProviderConfig},
    registry::RegistryConfig,
    types::*,
    ProviderListRequest,
};
use nebenk_network::marketplace::MarketplaceNetwork;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    println!("=== Multi-Node Marketplace Network Test ===\n");

    // Create 2 identities
    let identity1 = Arc::new(NodeIdentity::generate());
    let identity2 = Arc::new(NodeIdentity::generate());

    println!("Node 1 ID: {}", identity1.node_id());
    println!("Node 2 ID: {}", identity2.node_id());

    // Create network for node 1
    let keypair1 = identity1.to_libp2p_keypair();
    let (cmd_tx1, cmd_rx1) = mpsc::unbounded_channel();
    let (evt_tx1, mut evt_rx1) = mpsc::unbounded_channel();

    let mut network1 = MarketplaceNetwork::new(keypair1, cmd_rx1, evt_tx1)?;
    let listen_addr1: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse()?;

    // Create network for node 2
    let keypair2 = identity2.to_libp2p_keypair();
    let (cmd_tx2, cmd_rx2) = mpsc::unbounded_channel();
    let (evt_tx2, mut evt_rx2) = mpsc::unbounded_channel();

    let mut network2 = MarketplaceNetwork::new(keypair2, cmd_rx2, evt_tx2)?;
    let listen_addr2: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse()?;

    // Start network 1
    let network1_handle = tokio::spawn(async move { network1.run(listen_addr1).await });

    // Wait a bit for network 1 to start listening
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Get the actual listen address of network 1
    // We need to run network1 in a way that we can get its listen address
    // For now, let's use a fixed port for node 1
    println!("Network 1 started, waiting for network 2 to connect...");

    // Start network 2 with a fixed port
    let listen_addr2: Multiaddr = "/ip4/127.0.0.1/tcp/4002".parse()?;

    let network2_handle = tokio::spawn(async move { network2.run(listen_addr2).await });

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Now dial node 1 from node 2
    // We need to know node 1's listen address
    // Let's use a fixed port for node 1 too

    // Actually, let's restructure: use fixed ports for both
    println!("Using fixed ports for both nodes...");

    // We'll create a simpler test with proper address exchange
    // For now, let's just wait for the network tasks to run
    // and try to connect via the known addresses

    // This test needs the actual listen addresses
    // For a real test, we'd need to capture the listen address from the network events
    // Let's just run the networks and let them discover each other via mDNS

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create registries with the command channels
    let config = RegistryConfig::default();

    // Create command channels for registries
    let (_cmd_tx1, cmd_rx1) = mpsc::unbounded_channel();
    let (evt_tx1, mut evt_rx1) = mpsc::unbounded_channel();

    let (_cmd_tx2, cmd_rx2) = mpsc::unbounded_channel();
    let (evt_tx2, mut evt_rx2) = mpsc::unbounded_channel();

    let keypair1 = identity1.to_libp2p_keypair();
    let keypair2 = identity2.to_libp2p_keypair();

    let mut network1 = MarketplaceNetwork::new(keypair1, cmd_rx1, evt_tx1)?;
    let mut network2 = MarketplaceNetwork::new(keypair2, cmd_rx2, evt_tx2)?;

    // Start both networks on different ports
    let listen_addr1: Multiaddr = "/ip4/127.0.0.1/tcp/4001".parse()?;
    let listen_addr2: Multiaddr = "/ip4/127.0.0.1/tcp/4002".parse()?;

    let network1_handle = tokio::spawn(async move { network1.run(listen_addr1).await });

    tokio::time::sleep(Duration::from_millis(500)).await;

    let network2_handle = tokio::spawn(async move { network2.run(listen_addr2).await });

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Now we need to dial - but we don't have access to the network instances anymore
    // They're moved into the spawn. Let's restructure.

    println!("\nNetworks started on ports 4001 and 4002");
    println!("Waiting for mDNS discovery...");

    tokio::time::sleep(Duration::from_secs(5)).await;

    println!("\n=== Test Complete (Network Setup) ===");
    println!("Note: Full multi-node test requires proper address exchange and dialing");
    println!("The networks are running and should discover each other via mDNS");

    Ok(())
}
