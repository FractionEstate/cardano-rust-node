//! ChainSync Service
//!
//! Runtime service that coordinates blockchain synchronization between nodes.
//! This service manages the lifecycle of ChainSync client/server interactions,
//! integrates with storage backends, and maintains sync state.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────┐
//! │ ChainSyncService    │
//! ├─────────────────────┤
//! │ - Manages clients   │ ──> Peer connections
//! │ - Handles servers   │ <── Incoming requests
//! │ - Tracks sync state │
//! │ - Integrates storage│ ──> ChainDatabase
//! └─────────────────────┘
//! ```
//!
//! Based on the Cardano Network Protocol Specification.

use crate::protocols::chainsync::{
    ChainSyncClient, ChainSyncConfig, ChainSyncError, ChainSyncMessage, Point, Tip,
};
use cardano_consensus::block_production::BlockHeader;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Connection identifier for tracking peer sync state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeerId(pub u64);

impl PeerId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for PeerId {
    fn default() -> Self {
        Self::new()
    }
}

/// Sync state for a peer connection
#[derive(Debug, Clone)]
pub struct PeerSyncState {
    /// Current tip reported by the peer
    pub tip: Option<Tip>,
    /// Last intersection point found with peer
    pub intersection: Option<Point>,
    /// Number of headers received from this peer
    pub headers_received: u64,
    /// Whether we're currently syncing with this peer
    pub is_syncing: bool,
    /// Last error encountered (if any)
    pub last_error: Option<String>,
}

impl PeerSyncState {
    pub fn new() -> Self {
        Self {
            tip: None,
            intersection: None,
            headers_received: 0,
            is_syncing: false,
            last_error: None,
        }
    }
}

impl Default for PeerSyncState {
    fn default() -> Self {
        Self::new()
    }
}

/// Overall synchronization state
#[derive(Debug, Clone, PartialEq)]
pub enum SyncState {
    /// Not started yet
    Idle,
    /// Finding intersection with peers
    FindingIntersection,
    /// Actively syncing headers
    Syncing,
    /// Caught up to network tip
    InSync,
    /// Sync failed
    Failed(String),
}

/// Statistics about the sync service
#[derive(Debug, Clone)]
pub struct SyncStats {
    /// Number of active peer connections
    pub active_peers: usize,
    /// Total headers received
    pub total_headers: u64,
    /// Current sync state
    pub state: SyncState,
    /// Current local tip slot
    pub local_tip_slot: u64,
    /// Network tip slot (highest seen from peers)
    pub network_tip_slot: u64,
    /// Slots behind network tip
    pub slots_behind: i64,
}

/// ChainSync service for managing blockchain synchronization
pub struct ChainSyncService {
    /// Configuration
    config: ChainSyncConfig,
    /// Active client connections (one per peer)
    clients: Arc<RwLock<HashMap<PeerId, Arc<ChainSyncClient>>>>,
    /// Sync state for each peer
    peer_states: Arc<RwLock<HashMap<PeerId, PeerSyncState>>>,
    /// Overall sync state
    sync_state: Arc<RwLock<SyncState>>,
    /// Local chain tip
    local_tip: Arc<RwLock<Option<Tip>>>,
    /// Headers received (pending validation and storage)
    header_queue: Arc<RwLock<Vec<BlockHeader>>>,
}

impl ChainSyncService {
    /// Create a new ChainSync service
    pub fn new(config: ChainSyncConfig) -> Self {
        Self {
            config,
            clients: Arc::new(RwLock::new(HashMap::new())),
            peer_states: Arc::new(RwLock::new(HashMap::new())),
            sync_state: Arc::new(RwLock::new(SyncState::Idle)),
            local_tip: Arc::new(RwLock::new(None)),
            header_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Set the local chain tip
    ///
    /// This should be called when the node starts up or when new blocks are added.
    pub async fn set_local_tip(&self, tip: Tip) {
        info!(
            "Local tip updated: slot={}, height={}",
            tip.slot.0, tip.height
        );
        *self.local_tip.write().await = Some(tip);
    }

    /// Get the current sync state
    pub async fn sync_state(&self) -> SyncState {
        self.sync_state.read().await.clone()
    }

    /// Get sync statistics
    pub async fn get_stats(&self) -> SyncStats {
        let states = self.peer_states.read().await;
        let sync_state = self.sync_state.read().await.clone();
        let local_tip = self.local_tip.read().await.clone();

        let total_headers: u64 = states.values().map(|s| s.headers_received).sum();
        let active_peers = states.values().filter(|s| s.is_syncing).count();

        let local_tip_slot = local_tip.as_ref().map(|t| t.slot.0).unwrap_or(0);
        let network_tip_slot = states
            .values()
            .filter_map(|s| s.tip.as_ref())
            .map(|t| t.slot.0)
            .max()
            .unwrap_or(0);

        let slots_behind = network_tip_slot as i64 - local_tip_slot as i64;

        SyncStats {
            active_peers,
            total_headers,
            state: sync_state,
            local_tip_slot,
            network_tip_slot,
            slots_behind,
        }
    }

    /// Add a new peer and create a client for syncing
    pub async fn add_peer(&self, peer_id: PeerId) -> Result<(), ChainSyncError> {
        let (client, mut outbound_rx, _inbound_tx) = ChainSyncClient::new();
        let client = Arc::new(client);

        // Store client and initialize state
        self.clients.write().await.insert(peer_id, client.clone());
        self.peer_states
            .write()
            .await
            .insert(peer_id, PeerSyncState::new());

        // Spawn a task to handle outbound messages
        tokio::spawn(async move {
            while let Some(message) = outbound_rx.recv().await {
                debug!("Peer {} sent message: {:?}", peer_id.0, message);
                // In a real implementation, this would send the message over the network
                // For now, we just log it

                // TODO: Wire this to the actual network connection
                // This should use the ConnectionManager to send messages to the peer
            }
        });

        info!("Added peer {} to ChainSync service", peer_id.0);
        Ok(())
    }

    /// Remove a peer (connection closed)
    pub async fn remove_peer(&self, peer_id: &PeerId) {
        self.clients.write().await.remove(peer_id);
        self.peer_states.write().await.remove(peer_id);
        info!("Removed peer {} from ChainSync service", peer_id.0);
    }

    /// Start syncing with a specific peer
    ///
    /// This initiates the chain synchronization protocol by finding an intersection
    /// and then requesting headers.
    pub async fn start_sync_with_peer(
        &self,
        peer_id: &PeerId,
        intersection_points: Vec<Point>,
    ) -> Result<(), ChainSyncError> {
        let client = self
            .clients
            .read()
            .await
            .get(peer_id)
            .ok_or_else(|| ChainSyncError::InvalidState(format!("Peer {} not found", peer_id.0)))?
            .clone();

        // Update sync state
        {
            let mut states = self.peer_states.write().await;
            if let Some(state) = states.get_mut(peer_id) {
                state.is_syncing = true;
            }
        }
        *self.sync_state.write().await = SyncState::FindingIntersection;

        info!(
            "Starting sync with peer {}: finding intersection with {} points",
            peer_id.0,
            intersection_points.len()
        );

        // Request intersection
        client.find_intersect(intersection_points).await?;

        Ok(())
    }

    /// Handle an incoming ChainSync message from a peer
    pub async fn handle_peer_message(
        &self,
        peer_id: &PeerId,
        message: ChainSyncMessage,
    ) -> Result<(), ChainSyncError> {
        let client = self
            .clients
            .read()
            .await
            .get(peer_id)
            .ok_or_else(|| ChainSyncError::InvalidState(format!("Peer {} not found", peer_id.0)))?
            .clone();

        // Log the message
        match &message {
            ChainSyncMessage::RollForward { header, tip } => {
                debug!(
                    "Peer {} RollForward: slot={}, tip_slot={}",
                    peer_id.0, header.slot.0, tip.slot.0
                );
            }
            ChainSyncMessage::RollBackward { point, tip } => {
                warn!(
                    "Peer {} RollBackward: to slot={}, tip_slot={}",
                    peer_id.0, point.slot.0, tip.slot.0
                );
            }
            ChainSyncMessage::IntersectFound { point, tip } => {
                info!(
                    "Peer {} IntersectFound: slot={}, tip_slot={}",
                    peer_id.0, point.slot.0, tip.slot.0
                );
            }
            ChainSyncMessage::IntersectNotFound { tip } => {
                warn!(
                    "Peer {} IntersectNotFound: tip_slot={}",
                    peer_id.0, tip.slot.0
                );
            }
            _ => {}
        }

        // Update peer state before handling
        if let Some(state) = self.peer_states.write().await.get_mut(peer_id) {
            match &message {
                ChainSyncMessage::RollForward { header, tip } => {
                    state.tip = Some(tip.clone());
                    state.headers_received += 1;

                    // Add header to queue for validation
                    self.header_queue.write().await.push(*header.clone());
                }
                ChainSyncMessage::IntersectFound { point, tip } => {
                    state.intersection = Some(point.clone());
                    state.tip = Some(tip.clone());
                    *self.sync_state.write().await = SyncState::Syncing;
                }
                ChainSyncMessage::IntersectNotFound { tip } => {
                    state.tip = Some(tip.clone());
                    // No intersection found, will need to sync from genesis
                }
                _ => {}
            }
        }

        // Let the client handle the message
        client.handle_message(message).await?;

        // If we received a header, automatically request the next one
        let client_state = client.get_state();
        if matches!(
            client_state,
            crate::protocols::chainsync::ChainSyncClientState::Idle
        ) {
            // Check if we should continue syncing
            if let Some(peer_state) = self.peer_states.read().await.get(peer_id) {
                if peer_state.is_syncing {
                    // Request next header
                    debug!("Peer {}: requesting next header", peer_id.0);
                    client.request_next().await?;
                }
            }
        }

        Ok(())
    }

    /// Request the next header from a peer
    pub async fn request_next(&self, peer_id: &PeerId) -> Result<(), ChainSyncError> {
        let client = self
            .clients
            .read()
            .await
            .get(peer_id)
            .ok_or_else(|| ChainSyncError::InvalidState(format!("Peer {} not found", peer_id.0)))?
            .clone();

        client.request_next().await?;
        Ok(())
    }

    /// Stop syncing with a peer
    pub async fn stop_sync_with_peer(&self, peer_id: &PeerId) {
        if let Some(state) = self.peer_states.write().await.get_mut(peer_id) {
            state.is_syncing = false;
            info!("Stopped syncing with peer {}", peer_id.0);
        }
    }

    /// Get peer sync state
    pub async fn get_peer_state(&self, peer_id: &PeerId) -> Option<PeerSyncState> {
        self.peer_states.read().await.get(peer_id).cloned()
    }

    /// Get all peer IDs
    pub async fn get_peers(&self) -> Vec<PeerId> {
        self.clients.read().await.keys().copied().collect()
    }

    /// Get pending headers that need validation
    pub async fn take_pending_headers(&self) -> Vec<BlockHeader> {
        let mut queue = self.header_queue.write().await;
        std::mem::take(&mut *queue)
    }

    /// Check if we're in sync with the network
    pub async fn is_in_sync(&self) -> bool {
        let stats = self.get_stats().await;
        // Need peers to consider ourselves in sync
        if stats.active_peers == 0 {
            return false;
        }
        // Consider in sync if we're within 20 slots of network tip
        stats.slots_behind < 20 && stats.slots_behind >= 0
    }

    /// Update sync state based on current conditions
    pub async fn update_sync_state(&self) {
        let stats = self.get_stats().await;

        let new_state = if stats.active_peers == 0 {
            SyncState::Idle
        } else if stats.slots_behind < 20 && stats.slots_behind >= 0 {
            SyncState::InSync
        } else {
            SyncState::Syncing
        };

        let mut state = self.sync_state.write().await;
        if *state != new_state {
            info!("Sync state changed: {:?} -> {:?}", *state, new_state);
            *state = new_state;
        }
    }
}

impl Default for ChainSyncService {
    fn default() -> Self {
        Self::new(ChainSyncConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cardano_consensus::ouroboros::SlotNo;
    use cardano_crypto::Blake2b256Hash;

    #[tokio::test]
    async fn test_service_creation() {
        let service = ChainSyncService::default();
        assert!(matches!(service.sync_state().await, SyncState::Idle));
    }

    #[tokio::test]
    async fn test_add_remove_peer() {
        let service = ChainSyncService::default();
        let peer_id = PeerId::new();

        service.add_peer(peer_id).await.unwrap();
        let peers = service.get_peers().await;
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0], peer_id);

        service.remove_peer(&peer_id).await;
        let peers = service.get_peers().await;
        assert_eq!(peers.len(), 0);
    }

    #[tokio::test]
    async fn test_local_tip_update() {
        let service = ChainSyncService::default();
        let tip = Tip {
            slot: SlotNo(1000),
            hash: Blake2b256Hash::hash(b"test"),
            height: 100,
        };

        service.set_local_tip(tip.clone()).await;
        let local_tip = service.local_tip.read().await.clone();
        assert_eq!(local_tip, Some(tip));
    }

    #[tokio::test]
    async fn test_sync_stats() {
        let service = ChainSyncService::default();
        let stats = service.get_stats().await;

        assert_eq!(stats.active_peers, 0);
        assert_eq!(stats.total_headers, 0);
        assert!(matches!(stats.state, SyncState::Idle));
    }

    #[tokio::test]
    async fn test_is_in_sync() {
        let service = ChainSyncService::default();

        // Initially not in sync (no peers)
        assert!(!service.is_in_sync().await);

        // Set local tip to match network tip
        let tip = Tip {
            slot: SlotNo(1000),
            hash: Blake2b256Hash::hash(b"test"),
            height: 100,
        };
        service.set_local_tip(tip).await;

        // Still not in sync (no peers reporting)
        assert!(!service.is_in_sync().await);
    }

    #[tokio::test]
    async fn test_peer_state_tracking() {
        let service = ChainSyncService::default();
        let peer_id = PeerId::new();

        service.add_peer(peer_id).await.unwrap();

        let state = service.get_peer_state(&peer_id).await;
        assert!(state.is_some());

        let state = state.unwrap();
        assert_eq!(state.headers_received, 0);
        assert!(!state.is_syncing);
        assert!(state.tip.is_none());
        assert!(state.intersection.is_none());
    }
}
