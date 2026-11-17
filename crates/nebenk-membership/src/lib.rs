use nebenk_core::error::{NebenkError, Result};
use nebenk_core::types::{ClusterId, NodeId};
use nebenk_network::{
    ClusterMember, Heartbeat as NetworkHeartbeat, HeartbeatAck, NetworkMessage, NodeRole, NodeState,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{info, warn};

/// Cluster membership list with failure detection.
#[derive(Clone)]
pub struct Membership {
    cluster_id: ClusterId,
    local_node_id: NodeId,
    members: Arc<RwLock<HashMap<NodeId, ClusterMember>>>,
    failure_detector: Arc<Mutex<PhiAccrualFailureDetector>>,
    suspicion_threshold: f64,
    failure_threshold: f64,
    event_tx: mpsc::UnboundedSender<MembershipEvent>,
}

#[derive(Clone, Debug)]
pub enum MembershipEvent {
    NodeJoined(NodeId),
    NodeLeft(NodeId),
    NodeSuspected(NodeId),
    NodeFailed(NodeId),
    NodeRecovered(NodeId),
    PrimaryChanged(NodeId),
    StateChanged(NodeId, NodeState),
}

impl Membership {
    /// Create a new membership instance for the local node.
    pub fn new(
        cluster_id: ClusterId,
        local_node_id: NodeId,
        _heartbeat_interval_ms: u64,
    ) -> (Self, mpsc::UnboundedReceiver<MembershipEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let failure_detector = PhiAccrualFailureDetector::new(
            8.0,   // suspicion threshold (phi)
            16.0,  // failure threshold (phi)
            1000,  // max sample size
            100.0, // min std deviation
        );

        let mut members = HashMap::new();
        let now = current_time_ms();
        members.insert(
            local_node_id,
            ClusterMember {
                node_id: local_node_id,
                public_key: *local_node_id.as_bytes(),
                role: NodeRole::Primary,
                state: NodeState::Active,
                last_seen_ms: now,
                last_synced_rev: 0,
                address: None,
            },
        );

        let membership = Self {
            cluster_id,
            local_node_id,
            members: Arc::new(RwLock::new(members)),
            failure_detector: Arc::new(Mutex::new(failure_detector)),
            suspicion_threshold: 8.0,
            failure_threshold: 16.0,
            event_tx,
        };

        (membership, event_rx)
    }

    /// Get the local node's ID.
    pub fn local_node_id(&self) -> NodeId {
        self.local_node_id
    }

    /// Get the cluster ID.
    pub fn cluster_id(&self) -> &ClusterId {
        &self.cluster_id
    }

    /// Get the current primary node ID, if any.
    pub async fn primary(&self) -> Option<NodeId> {
        let members = self.members.read().await;
        for member in members.values() {
            if member.role == NodeRole::Primary && member.state == NodeState::Active {
                return Some(member.node_id);
            }
        }
        None
    }

    /// Check if the local node is the primary.
    pub async fn is_primary(&self) -> bool {
        self.primary().await == Some(self.local_node_id)
    }

    /// Get all cluster members.
    pub async fn members(&self) -> Vec<ClusterMember> {
        let members = self.members.read().await;
        members.values().cloned().collect()
    }

    /// Get active members (excluding failed/leaving).
    pub async fn active_members(&self) -> Vec<ClusterMember> {
        let members = self.members.read().await;
        members
            .values()
            .filter(|m| m.state == NodeState::Active || m.state == NodeState::Syncing)
            .cloned()
            .collect()
    }

    /// Get a specific member by NodeId.
    pub async fn get_member(&self, node_id: &NodeId) -> Option<ClusterMember> {
        let members = self.members.read().await;
        members.get(node_id).cloned()
    }

    /// Add or update a member (used when a node joins or heartbeats).
    pub async fn upsert_member(
        &self,
        node_id: NodeId,
        public_key: [u8; 32],
        role: NodeRole,
        address: Option<String>,
    ) -> Result<bool> {
        let mut members = self.members.write().await;
        let now = current_time_ms();
        let is_new = !members.contains_key(&node_id);

        if is_new {
            info!("Node {} joining cluster {}", node_id, self.cluster_id);
            members.insert(
                node_id,
                ClusterMember {
                    node_id,
                    public_key,
                    role,
                    state: NodeState::Syncing,
                    last_seen_ms: now,
                    last_synced_rev: 0,
                    address,
                },
            );
            let _ = self.event_tx.send(MembershipEvent::NodeJoined(node_id));
        } else {
            if let Some(member) = members.get_mut(&node_id) {
                member.last_seen_ms = now;
                member.address = address.or(member.address.clone());
                member.role = role;
                member.state = NodeState::Active;
                // Record heartbeat arrival for failure detection
                self.record_heartbeat(node_id, now).await;
            }
        }

        Ok(is_new)
    }

    /// Mark a member as having synced to a specific revision.
    pub async fn mark_synced(&self, node_id: &NodeId, revision: u64) -> Result<()> {
        let mut members = self.members.write().await;
        if let Some(member) = members.get_mut(node_id) {
            member.last_synced_rev = revision;
            if member.state == NodeState::Syncing {
                member.state = NodeState::Active;
                let _ = self
                    .event_tx
                    .send(MembershipEvent::StateChanged(*node_id, NodeState::Active));
            }
        }
        Ok(())
    }

    /// Record a heartbeat arrival from a peer for failure detection.
    pub async fn record_heartbeat(&self, node_id: NodeId, timestamp_ms: u64) {
        let mut detector = self.failure_detector.lock().await;
        detector.heartbeat(node_id, timestamp_ms);
    }

    /// Process periodic failure detection (should be called regularly).
    pub async fn run_failure_detection(&self) -> Vec<MembershipEvent> {
        let now = current_time_ms();
        let detector = self.failure_detector.lock().await;
        let mut members = self.members.write().await;
        let mut events = Vec::new();

        let all_node_ids: Vec<NodeId> = members.keys().cloned().collect();

        for node_id in all_node_ids {
            if node_id == self.local_node_id {
                continue; // Don't monitor ourselves
            }

            let phi = detector.phi(node_id, now);

            if let Some(member) = members.get_mut(&node_id) {
                let prev_state = member.state;
                let new_state = if phi >= self.failure_threshold {
                    NodeState::Failed
                } else if phi >= self.suspicion_threshold {
                    NodeState::Suspected
                } else {
                    NodeState::Active
                };

                if new_state != prev_state {
                    member.state = new_state;
                    match new_state {
                        NodeState::Suspected => {
                            warn!("Node {} suspected (phi={:.2})", node_id, phi);
                            events.push(MembershipEvent::NodeSuspected(node_id));
                        }
                        NodeState::Failed => {
                            warn!("Node {} failed (phi={:.2})", node_id, phi);
                            events.push(MembershipEvent::NodeFailed(node_id));
                        }
                        NodeState::Active => {
                            if matches!(prev_state, NodeState::Suspected | NodeState::Failed) {
                                info!("Node {} recovered (phi={:.2})", node_id, phi);
                                events.push(MembershipEvent::NodeRecovered(node_id));
                            }
                        }
                        _ => {}
                    }
                    events.push(MembershipEvent::StateChanged(node_id, new_state));
                }
            }
        }

        // Send events
        for event in &events {
            let _ = self.event_tx.send(event.clone());
        }

        events
    }

    /// Initiate a graceful leave for the local node.
    pub async fn leave(&self) -> Result<()> {
        let mut members = self.members.write().await;
        if let Some(member) = members.get_mut(&self.local_node_id) {
            member.state = NodeState::Leaving;
        }
        let _ = self
            .event_tx
            .send(MembershipEvent::NodeLeft(self.local_node_id));
        Ok(())
    }

    /// Promote a node to primary (used during failover).
    pub async fn promote_to_primary(&self, node_id: NodeId) -> Result<bool> {
        let mut members = self.members.write().await;
        let mut old_primary = None;

        // Demote current primary
        for member in members.values_mut() {
            if member.role == NodeRole::Primary {
                if member.node_id == node_id {
                    return Ok(false); // Already primary
                }
                member.role = NodeRole::Replica;
                old_primary = Some(member.node_id);
            }
        }

        // Promote new primary
        if let Some(member) = members.get_mut(&node_id) {
            // Allow promotion from Syncing state as well for failover
            if member.state != NodeState::Active && member.state != NodeState::Syncing {
                return Err(NebenkError::Membership(
                    "Cannot promote non-active node to primary".into(),
                ));
            }
            member.role = NodeRole::Primary;
            let _ = self.event_tx.send(MembershipEvent::PrimaryChanged(node_id));
            info!(
                "Node {} promoted to primary (was {:?})",
                node_id, old_primary
            );
            Ok(true)
        } else {
            Err(NebenkError::Membership("Node not found in cluster".into()))
        }
    }

    /// Handle failover when primary is suspected/failed.
    pub async fn handle_primary_failure(&self) -> Result<Option<NodeId>> {
        let primary = self.primary().await;
        if primary == Some(self.local_node_id) {
            return Ok(None); // We are primary, no failover needed
        }

        // Check if primary is failed/suspected
        let members = self.members.read().await;
        let primary_failed = primary.is_some_and(|pid| {
            members
                .get(&pid)
                .map(|m| m.state == NodeState::Failed || m.state == NodeState::Suspected)
                .unwrap_or(true)
        });

        if !primary_failed {
            return Ok(None);
        }

        drop(members);

        // Elect new primary: highest synced revision among active replicas
        let members = self.members.read().await;
        let mut candidates: Vec<_> = members
            .values()
            .filter(|m| {
                m.role == NodeRole::Replica
                    && (m.state == NodeState::Active || m.state == NodeState::Syncing)
            })
            .collect();

        candidates.sort_by_key(|a| std::cmp::Reverse(a.last_synced_rev));

        if let Some(new_primary) = candidates.first() {
            let new_primary_id = new_primary.node_id;
            drop(members);
            self.promote_to_primary(new_primary_id).await?;
            Ok(Some(new_primary_id))
        } else {
            Ok(None)
        }
    }

    /// Get the event receiver for external consumption.
    pub fn event_receiver(&self) -> mpsc::UnboundedReceiver<MembershipEvent> {
        unreachable!("Use the receiver returned from Membership::new()")
    }
}

/// Phi Accrual Failure Detector implementation.
/// Based on the paper "The Phi Accrual Failure Detector" by Hayashibara et al.
struct PhiAccrualFailureDetector {
    /// Heartbeat intervals per node: node_id -> Vec<interval_ms>
    intervals: HashMap<NodeId, Vec<u64>>,
    /// Last heartbeat timestamp per node
    last_heartbeat: HashMap<NodeId, u64>,
    max_sample_size: usize,
    min_std_deviation_ms: f64,
}

impl PhiAccrualFailureDetector {
    fn new(
        _suspicion_threshold: f64,
        _failure_threshold: f64,
        max_sample_size: usize,
        min_std_deviation_ms: f64,
    ) -> Self {
        Self {
            intervals: HashMap::new(),
            last_heartbeat: HashMap::new(),
            max_sample_size,
            min_std_deviation_ms,
        }
    }

    fn heartbeat(&mut self, node_id: NodeId, timestamp_ms: u64) {
        let intervals = self.intervals.entry(node_id).or_default();

        if let Some(&last_ts) = self.last_heartbeat.get(&node_id) {
            let interval = timestamp_ms.saturating_sub(last_ts);
            intervals.push(interval);
            if intervals.len() > self.max_sample_size {
                intervals.remove(0);
            }
        }
        self.last_heartbeat.insert(node_id, timestamp_ms);
    }

    fn phi(&self, node_id: NodeId, now_ms: u64) -> f64 {
        let intervals = match self.intervals.get(&node_id) {
            Some(h) if h.len() >= 2 => h,
            _ => return 0.0, // Not enough data for phi calculation
        };

        let last_hb = match self.last_heartbeat.get(&node_id) {
            Some(&ts) => ts,
            None => return 0.0,
        };

        let mean = intervals.iter().map(|&x| x as f64).sum::<f64>() / intervals.len() as f64;
        let variance = intervals
            .iter()
            .map(|&x| {
                let diff = x as f64 - mean;
                diff * diff
            })
            .sum::<f64>()
            / intervals.len() as f64;

        let std_dev = variance.sqrt().max(self.min_std_deviation_ms);

        // Time since last heartbeat
        let time_since_last = now_ms.saturating_sub(last_hb) as f64;

        // Phi = -log10(1 - CDF(time_since_last))
        // For normal distribution:
        // CDF(x) = 0.5 * (1 + erf((x - mean) / (std_dev * sqrt(2))))
        if time_since_last <= mean {
            return 0.0;
        }

        let y = (time_since_last - mean) / (std_dev * 2.0_f64.sqrt());
        let cdf = 0.5 * (1.0 + erf(y));
        -((1.0 - cdf).max(1e-10)).log10()
    }
}

/// Error function approximation for phi calculation.
fn erf(x: f64) -> f64 {
    // Abramowitz and Stegun approximation
    let sign = if x >= 0.0 { 1.0 } else { -1.0 };
    let x = x.abs();

    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

    sign * y
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Heartbeat message used internally for membership (includes cluster info).
#[derive(Clone, Debug)]
pub struct Heartbeat {
    pub node_id: NodeId,
    pub cluster_id: ClusterId,
    pub revision: u64,
    pub timestamp_ms: u64,
    pub role: NodeRole,
    pub state: NodeState,
}

/// Background task for sending heartbeats and processing incoming ones.
pub struct HeartbeatManager {
    membership: Arc<Membership>,
    network_sender: mpsc::UnboundedSender<NetworkMessage>,
    local_revision: Arc<Mutex<u64>>,
    interval_ms: u64,
}

impl Clone for HeartbeatManager {
    fn clone(&self) -> Self {
        Self {
            membership: self.membership.clone(),
            network_sender: self.network_sender.clone(),
            local_revision: self.local_revision.clone(),
            interval_ms: self.interval_ms,
        }
    }
}

impl HeartbeatManager {
    pub fn new(
        membership: Arc<Membership>,
        network_sender: mpsc::UnboundedSender<NetworkMessage>,
        local_revision: Arc<Mutex<u64>>,
        interval_ms: u64,
    ) -> Self {
        Self {
            membership,
            network_sender,
            local_revision,
            interval_ms,
        }
    }

    pub async fn run(&self) {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_millis(self.interval_ms));

        loop {
            interval.tick().await;

            let _revision = *self.local_revision.lock().await;
            let role = self.get_local_role().await;
            let state = self.get_local_state().await;

            // Create internal heartbeat
            let internal_hb = Heartbeat {
                node_id: self.membership.local_node_id(),
                cluster_id: self.membership.cluster_id().clone(),
                revision: 0,
                timestamp_ms: current_time_ms(),
                role,
                state,
            };

            // Convert to network heartbeat message (using named fields)
            let network_hb = NetworkMessage::Heartbeat {
                node_id: internal_hb.node_id,
                cluster_id: internal_hb.cluster_id,
                revision: internal_hb.revision,
                timestamp_ms: internal_hb.timestamp_ms,
                role: internal_hb.role,
                state: internal_hb.state,
            };

            // Broadcast to all active peers
            let members = self.membership.active_members().await;
            for member in members {
                if member.node_id == self.membership.local_node_id() {
                    continue;
                }
                // In real implementation, send via network layer
                let _ = self.network_sender.send(network_hb.clone());
            }
        }
    }

    pub async fn handle_heartbeat(&self, heartbeat: NetworkHeartbeat) -> HeartbeatAck {
        // Update membership with peer's heartbeat
        self.membership
            .upsert_member(
                heartbeat.node_id,
                *heartbeat.node_id.as_bytes(),
                NodeRole::Replica, // Default, will be updated via gossip
                None,
            )
            .await
            .ok();

        self.membership
            .record_heartbeat(heartbeat.node_id, heartbeat.timestamp_ms)
            .await;

        let _revision = *self.local_revision.lock().await;
        let members = self.membership.active_members().await;

        HeartbeatAck {
            node_id: self.membership.local_node_id(),
            timestamp_ms: current_time_ms(),
            members,
        }
    }

    async fn get_local_role(&self) -> NodeRole {
        if self.membership.is_primary().await {
            NodeRole::Primary
        } else {
            NodeRole::Replica
        }
    }

    async fn get_local_state(&self) -> NodeState {
        NodeState::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nebenk_core::types::ClusterId;

    #[tokio::test]
    async fn test_membership_basic() {
        let cluster = ClusterId::new("test");
        let local_id = NodeId::from_bytes([1u8; 32]);

        let (membership, mut events) = Membership::new(cluster, local_id, 1000);

        // Local node should be primary
        assert!(membership.is_primary().await);
        assert_eq!(membership.primary().await, Some(local_id));

        // Add a replica
        let replica_id = NodeId::from_bytes([2u8; 32]);
        membership
            .upsert_member(replica_id, [2u8; 32], NodeRole::Replica, None)
            .await
            .unwrap();

        // Wait for join event
        let event = events.recv().await.unwrap();
        assert!(matches!(event, MembershipEvent::NodeJoined(id) if id == replica_id));

        let members = membership.active_members().await;
        assert_eq!(members.len(), 2);
    }

    #[tokio::test]
    async fn test_phi_detector_basic() {
        let cluster = ClusterId::new("test");
        let local_id = NodeId::from_bytes([1u8; 32]);
        let peer_id = NodeId::from_bytes([2u8; 32]);

        let (membership, _events) = Membership::new(cluster, local_id, 1000);

        // Add peer as Active (not Syncing) to avoid state change
        {
            let mut members = membership.members.write().await;
            members.insert(
                peer_id,
                ClusterMember {
                    node_id: peer_id,
                    public_key: [2u8; 32],
                    role: NodeRole::Replica,
                    state: NodeState::Active,
                    last_seen_ms: current_time_ms(),
                    last_synced_rev: 0,
                    address: None,
                },
            );
        }

        let now = current_time_ms();

        // Send a few heartbeats at regular intervals
        for i in 0..5 {
            membership.record_heartbeat(peer_id, now + i * 1000).await;
        }

        // Immediately after last heartbeat, phi should be low (no state change events for non-local nodes)
        let events = membership.run_failure_detection().await;
        // Filter out local node state changes
        let peer_events: Vec<_> = events
            .into_iter()
            .filter(|e| !matches!(e, MembershipEvent::StateChanged(id, _) if *id == local_id))
            .collect();
        assert!(
            peer_events.is_empty(),
            "Expected no peer events immediately after heartbeats, got: {:?}",
            peer_events
        );
    }

    #[tokio::test]
    async fn test_phi_detector_suspicion() {
        let cluster = ClusterId::new("test");
        let local_id = NodeId::from_bytes([1u8; 32]);
        let peer_id = NodeId::from_bytes([2u8; 32]);

        let (membership, mut events) = Membership::new(cluster, local_id, 1000);

        membership
            .upsert_member(peer_id, [2u8; 32], NodeRole::Replica, None)
            .await
            .unwrap();
        let _ = events.recv().await; // join event

        let now = current_time_ms();

        // Send heartbeats at regular intervals
        for i in 0..5 {
            membership.record_heartbeat(peer_id, now + i * 1000).await;
        }

        // Now simulate time passing - no heartbeats for a long time
        let future = now + 10000; // 10 seconds later
        membership.run_failure_detection().await; // run once to establish baseline

        // Manually test phi calculation
        let phi = {
            let detector = membership.failure_detector.lock().await;
            detector.phi(peer_id, future)
        };

        // With 1000ms intervals and 10000ms gap, phi should be high
        assert!(phi > 8.0, "Phi should be high after long gap: {}", phi);
    }

    #[tokio::test]
    async fn test_promote_to_primary() {
        let cluster = ClusterId::new("test");
        let local_id = NodeId::from_bytes([1u8; 32]);
        let replica_id = NodeId::from_bytes([2u8; 32]);

        let (membership, mut events) = Membership::new(cluster, local_id, 1000);

        membership
            .upsert_member(replica_id, [2u8; 32], NodeRole::Replica, None)
            .await
            .unwrap();
        let _ = events.recv().await; // join event

        // Mark replica as synced (Active state)
        membership.mark_synced(&replica_id, 5).await.unwrap();

        // Promote replica to primary
        membership.promote_to_primary(replica_id).await.unwrap();

        // Should receive PrimaryChanged event
        let mut found_primary_changed = false;
        for _ in 0..3 {
            if let Ok(event) = events.try_recv() {
                if matches!(event, MembershipEvent::PrimaryChanged(id) if id == replica_id) {
                    found_primary_changed = true;
                    break;
                }
            }
        }
        assert!(
            found_primary_changed,
            "Expected PrimaryChanged event for replica"
        );

        assert!(!membership.is_primary().await);
        assert_eq!(membership.primary().await, Some(replica_id));
    }

    #[test]
    fn test_erf_approximation() {
        // Test erf at known values - using looser tolerance for floating point
        assert!(erf(0.0).abs() < 1e-9, "erf(0) = {}", erf(0.0));
        assert!((erf(1.0) - 0.8427).abs() < 0.01, "erf(1) = {}", erf(1.0));
        assert!((erf(-1.0) + 0.8427).abs() < 0.01, "erf(-1) = {}", erf(-1.0));
        assert!((erf(2.0) - 0.9953).abs() < 0.01, "erf(2) = {}", erf(2.0));
    }
}
