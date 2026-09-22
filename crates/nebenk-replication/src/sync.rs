use nebenk_core::error::{NebenkError, Result};
use nebenk_core::traits::{LogStore, SnapshotStore, StateEngine};
use nebenk_core::types::OperationEnvelope;
use nebenk_identity::NodeIdentity;
use nebenk_network::NetworkMessage;
use tracing::info;

/// Process an incoming SyncRequest on the Primary node and formulate the catch-up response.
pub fn handle_sync_request<S: LogStore + SnapshotStore>(
    storage: &S,
    last_known_revision: u64,
) -> Result<NetworkMessage> {
    let latest_rev = storage.latest_revision()?;
    let latest_snapshot = storage.get_latest_snapshot()?;

    if let Some(snapshot) = latest_snapshot {
        if last_known_revision < snapshot.header.revision {
            // Replica is behind the latest snapshot. Send snapshot + subsequent ops.
            let ops = storage.get_operations_since(snapshot.header.revision, 5000)?;
            return Ok(NetworkMessage::SyncResponse {
                latest_revision: latest_rev,
                snapshot: Some(snapshot),
                operations: ops,
            });
        }
    }

    // Otherwise, replay missing operations directly from the WAL
    let ops = storage.get_operations_since(last_known_revision, 5000)?;
    Ok(NetworkMessage::SyncResponse {
        latest_revision: latest_rev,
        snapshot: None,
        operations: ops,
    })
}

/// Apply a SyncResponse on a Replica, restoring the snapshot if present and replaying ops.
pub fn apply_sync_response<E: StateEngine, S: LogStore + SnapshotStore>(
    engine: &mut E,
    storage: &mut S,
    response: NetworkMessage,
) -> Result<u64> {
    match response {
        NetworkMessage::SyncResponse {
            snapshot,
            operations,
            ..
        } => {
            if let Some(snap) = snapshot {
                if !snap.verify_hash() {
                    return Err(NebenkError::StateHashMismatch {
                        expected: bs58::encode(&snap.header.state_hash).into_string(),
                        actual: bs58::encode(&nebenk_core::types::Snapshot::compute_hash(&snap.data))
                            .into_string(),
                    });
                }
                info!(
                    "Applying snapshot at revision {} ({} bytes)",
                    snap.header.revision,
                    snap.data.len()
                );
                storage.save_snapshot(&snap)?;
                engine.restore(snap.header.revision, &snap.data)?;
            }

            for op in operations {
                let digest = op.digest_for_signing();
                if !op.signature.is_empty() {
                    NodeIdentity::verify(&op.author_node, &digest, &op.signature)?;
                }
                storage.append_operation(&op)?;
                engine.apply(&op)?;
            }

            Ok(engine.current_revision())
        }
        _ => Err(NebenkError::Replication(
            "Expected SyncResponse message".into(),
        )),
    }
}

/// Apply a single live streamed operation from the Primary.
pub fn apply_replicated_op<E: StateEngine, S: LogStore>(
    engine: &mut E,
    storage: &mut S,
    op: OperationEnvelope,
) -> Result<u64> {
    let digest = op.digest_for_signing();
    if !op.signature.is_empty() {
        NodeIdentity::verify(&op.author_node, &digest, &op.signature)?;
    }

    // Persist to local WAL
    storage.append_operation(&op)?;

    // Apply to local state machine
    let result = engine.apply(&op)?;
    Ok(result.revision)
}
