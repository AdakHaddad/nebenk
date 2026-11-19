use crate::protocol::{
    DeploymentAcceptanceMsg, DeploymentOfferMsg, ProviderAdvertisementMsg,
    ProviderHeartbeatMsg, ProviderListRequestMsg, ProviderListResponseMsg,
};
use libp2p::gossipsub::{Behaviour as Gossipsub, ConfigBuilder, Event as GossipsubEvent, IdentTopic, MessageAuthenticity, MessageId, ValidationMode};
use libp2p::kad::{store::MemoryStore, Behaviour as Kademlia, Event as KadEvent, QueryId, QueryResult, GetProvidersOk, RecordKey};
use libp2p::identify::{Behaviour as Identify, Config as IdentifyConfig, Event as IdentifyEvent};
use libp2p::swarm::{NetworkBehaviour, Swarm, SwarmEvent};
use libp2p::{Multiaddr, PeerId, Transport};
use libp2p::identity::Keypair;
use libp2p::noise;
use libp2p::yamux;
use libp2p::tcp;
use std::collections::HashMap;
use anyhow::Result;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use futures_util::StreamExt;

/// Marketplace gossipsub topics
pub mod topics {
    pub const PROVIDER_ADVERTISEMENTS: &str = "nebenk/marketplace/provider_advertisements";
    pub const PROVIDER_HEARTBEATS: &str = "nebenk/marketplace/provider_heartbeats";
}

/// Marketplace Kademlia protocol name
pub const MARKETPLACE_KAD_PROTOCOL: &str = "/nebenk/marketplace/kad/1.0.0";

/// Events from the marketplace network layer
#[derive(Debug, Clone)]
pub enum MarketplaceEvent {
    /// Received a provider advertisement
    ProviderAdvertisement(ProviderAdvertisementMsg),
    /// Received a provider heartbeat
    ProviderHeartbeat(ProviderHeartbeatMsg),
    /// Received a provider list request (DHT)
    ProviderListRequest {
        request: ProviderListRequestMsg,
        responder: mpsc::UnboundedSender<ProviderListResponseMsg>,
    },
    /// Received a deployment offer
    DeploymentOffer(DeploymentOfferMsg),
    /// Received a deployment acceptance
    DeploymentAcceptance(DeploymentAcceptanceMsg),
}

/// Commands to the marketplace network layer
#[derive(Debug)]
pub enum MarketplaceCommand {
    /// Publish a provider advertisement
    PublishAdvertisement(ProviderAdvertisementMsg),
    /// Publish a provider heartbeat
    PublishHeartbeat(ProviderHeartbeatMsg),
    /// Request provider list via DHT
    RequestProviderList {
        request: ProviderListRequestMsg,
        responder: mpsc::UnboundedSender<ProviderListResponseMsg>,
    },
    /// Send deployment offer to provider
    SendDeploymentOffer {
        offer: DeploymentOfferMsg,
        target_peer: PeerId,
    },
    /// Send deployment acceptance
    SendDeploymentAcceptance {
        acceptance: DeploymentAcceptanceMsg,
        target_peer: PeerId,
    },
}

/// Combined behaviour for marketplace networking using libp2p's derive macro
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "MarketplaceBehaviourEvent")]
pub struct MarketplaceBehaviour {
    pub gossipsub: Gossipsub,
    pub kademlia: Kademlia<MemoryStore>,
    pub identify: Identify,
}

impl MarketplaceBehaviour {
    pub fn new(local_peer_id: PeerId, local_keypair: &Keypair) -> anyhow::Result<Self> {
        // Gossipsub config
        let gossipsub_config = ConfigBuilder::default()
            .validation_mode(ValidationMode::Permissive)
            .message_id_fn(|msg: &libp2p::gossipsub::Message| {
                let peer_id = msg.source.unwrap_or(PeerId::random());
                let data = &msg.data;
                MessageId::from(format!("{:?}-{:?}", peer_id, data.len()))
            })
            .build().map_err(|e| anyhow::anyhow!(e))?;

        let gossipsub = Gossipsub::new(
            MessageAuthenticity::Signed(local_keypair.clone()),
            gossipsub_config,
        ).map_err(|e| anyhow::anyhow!(e))?;

        // Kademlia config - protocol is set via the libp2p config
        let store = MemoryStore::new(local_peer_id);
        let kademlia = Kademlia::new(local_peer_id, store);

        // Identify config
        let identify = Identify::new(
            IdentifyConfig::new("/nebenk/marketplace/1.0.0".to_string(), local_keypair.public())
                .with_interval(Duration::from_secs(60)),
        );

        Ok(Self {
            gossipsub,
            kademlia,
            identify,
        })
    }
}

/// Combined behaviour events
#[derive(Debug)]
pub enum MarketplaceBehaviourEvent {
    Gossipsub(GossipsubEvent),
    Kademlia(KadEvent),
    Identify(IdentifyEvent),
}

impl From<GossipsubEvent> for MarketplaceBehaviourEvent {
    fn from(e: GossipsubEvent) -> Self {
        Self::Gossipsub(e)
    }
}

impl From<KadEvent> for MarketplaceBehaviourEvent {
    fn from(e: KadEvent) -> Self {
        Self::Kademlia(e)
    }
}

impl From<IdentifyEvent> for MarketplaceBehaviourEvent {
    fn from(e: IdentifyEvent) -> Self {
        Self::Identify(e)
    }
}

/// Marketplace network service
pub struct MarketplaceNetwork {
    swarm: Swarm<MarketplaceBehaviour>,
    command_rx: mpsc::UnboundedReceiver<MarketplaceCommand>,
    event_tx: mpsc::UnboundedSender<MarketplaceEvent>,
    /// Local provider advertisements cache
    provider_cache: HashMap<PeerId, ProviderAdvertisementMsg>,
    /// Pending DHT requests
    pending_requests: HashMap<QueryId, mpsc::UnboundedSender<ProviderListResponseMsg>>,
}

impl MarketplaceNetwork {
    pub fn new(
        local_keypair: Keypair,
        command_rx: mpsc::UnboundedReceiver<MarketplaceCommand>,
        event_tx: mpsc::UnboundedSender<MarketplaceEvent>,
    ) -> anyhow::Result<Self> {
        let local_peer_id = PeerId::from(local_keypair.public());
        let mut behaviour = MarketplaceBehaviour::new(local_peer_id, &local_keypair)?;

        // Subscribe to marketplace topics
        behaviour
            .gossipsub
            .subscribe(&IdentTopic::new(topics::PROVIDER_ADVERTISEMENTS))?;
        behaviour
            .gossipsub
            .subscribe(&IdentTopic::new(topics::PROVIDER_HEARTBEATS))?;

        // Build transport
        let noise_config = noise::Config::new(&local_keypair)?;
        let yamux_config = yamux::Config::default();
        
        let transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise_config)
            .multiplex(yamux_config)
            .boxed();

        let swarm = Swarm::new(
            transport,
            behaviour,
            local_peer_id,
            libp2p::swarm::Config::with_tokio_executor(),
        );

        Ok(Self {
            swarm,
            command_rx,
            event_tx,
            provider_cache: HashMap::new(),
            pending_requests: HashMap::new(),
        })
    }

    /// Run the network event loop
    pub async fn run(mut self, listen_addr: Multiaddr) -> Result<(), anyhow::Error> {
        self.swarm.listen_on(listen_addr)?;

        loop {
            tokio::select! {
                // Handle incoming commands
                Some(cmd) = self.command_rx.recv() => {
                    self.handle_command(cmd).await;
                }
                // Handle swarm events
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await;
                }
            }
        }
    }

    /// Dial a peer address to establish connection
    pub fn dial(&mut self, addr: Multiaddr) -> Result<(), anyhow::Error> {
        self.swarm.dial(addr)?;
        Ok(())
    }

    /// Get the local peer ID
    pub fn local_peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    /// Get listening addresses
    pub fn listeners(&self) -> impl Iterator<Item = &Multiaddr> {
        self.swarm.listeners()
    }

    async fn handle_command(&mut self, cmd: MarketplaceCommand) {
        match cmd {
            MarketplaceCommand::PublishAdvertisement(adv) => {
                let data = match serde_json::to_vec(&adv) {
                    Ok(d) => d,
                    Err(e) => {
                        warn!("Failed to serialize advertisement: {}", e);
                        return;
                    }
                };
                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(
                    IdentTopic::new(topics::PROVIDER_ADVERTISEMENTS),
                    data,
                ) {
                    warn!("Failed to publish advertisement: {}", e);
                }
            }
            MarketplaceCommand::PublishHeartbeat(hb) => {
                let data = match serde_json::to_vec(&hb) {
                    Ok(d) => d,
                    Err(e) => {
                        warn!("Failed to serialize heartbeat: {}", e);
                        return;
                    }
                };
                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(
                    IdentTopic::new(topics::PROVIDER_HEARTBEATS),
                    data,
                ) {
                    warn!("Failed to publish heartbeat: {}", e);
                }
            }
            MarketplaceCommand::RequestProviderList { request, responder } => {
                // Start a DHT query for providers
                let key = RecordKey::new(&format!(
                    "providers/{}",
                    request.filter.as_ref().map_or("all", |_| "filtered")
                ));
                let query_id = self.swarm.behaviour_mut().kademlia.get_providers(key);
                self.pending_requests.insert(query_id, responder);
            }
            MarketplaceCommand::SendDeploymentOffer { target_peer, .. } => {
                debug!("Sending deployment offer to {:?}", target_peer);
                // Would use a custom protocol for direct messaging
            }
            MarketplaceCommand::SendDeploymentAcceptance { target_peer, .. } => {
                debug!("Sending deployment acceptance to {:?}", target_peer);
            }
        }
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<MarketplaceBehaviourEvent>) {
        match event {
            SwarmEvent::Behaviour(MarketplaceBehaviourEvent::Gossipsub(
                GossipsubEvent::Message {
                    propagation_source: peer_id,
                    message_id: _,
                    message,
                },
            )) => {
                let topic = message.topic.as_str();
                if topic == topics::PROVIDER_ADVERTISEMENTS {
                    if let Ok(adv) = serde_json::from_slice::<ProviderAdvertisementMsg>(&message.data)
                    {
                        self.provider_cache.insert(peer_id, adv.clone());
                        let _ = self.event_tx.send(MarketplaceEvent::ProviderAdvertisement(adv));
                    }
                } else if topic == topics::PROVIDER_HEARTBEATS {
                    if let Ok(hb) = serde_json::from_slice::<ProviderHeartbeatMsg>(&message.data) {
                        let _ = self.event_tx.send(MarketplaceEvent::ProviderHeartbeat(hb));
                    }
                }
            }
            SwarmEvent::Behaviour(MarketplaceBehaviourEvent::Kademlia(
                KadEvent::OutboundQueryProgressed {
                    id,
                    result,
                    ..
                },
            )) => {
                // Check for GetProviders result
                if let QueryResult::GetProviders(Ok(GetProvidersOk::FoundProviders { providers, .. })) = result {
                    if let Some(responder) = self.pending_requests.remove(&id) {
                        // Convert providers to response
                        let providers: Vec<ProviderAdvertisementMsg> = providers
                            .into_iter()
                            .filter_map(|p| self.provider_cache.get(&p).cloned())
                            .collect();

                        let response = ProviderListResponseMsg {
                            providers,
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        };
                        let _ = responder.send(response);
                    }
                }
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Marketplace listening on {}", address);
            }
            SwarmEvent::Behaviour(MarketplaceBehaviourEvent::Identify(
                IdentifyEvent::Received {
                    peer_id: _,
                    info: _,
                    ..
                },
            )) => {
                // Add peer to Kademlia
                // Note: addresses would come from identify info
            }
            _ => {}
        }
    }
}
