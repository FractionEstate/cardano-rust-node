//! ChainSync Server with Database Integration
//!
//! This module provides a database-backed implementation of the ChainSync server
//! that serves blocks from persistent storage instead of in-memory state.
//!
//! ## Current Status
//!
//! This is a foundational implementation that establishes the database integration
//! pattern for ChainSync. Full functionality requires BlockHeader CBOR serialization
//! support (tracked separately).
//!
//! ### Implemented:
//! - Database-backed tip querying
//! - Client cursor management
//! - Intersection finding via database lookup
//! - Server state machine
//!
//! ### Pending (requires BlockHeader CBOR):
//! - Actual block serving (serve_next)
//! - Block header deserialization from database
//!
//! ### Design
//!
//! The server maintains a cursor per client tracking their sync position. When a client
//! requests the next block, the server queries the database for the block after the cursor
//! and returns it. For intersection finding, the server checks each proposed point against
//! the database until finding a match.

use crate::protocols::chainsync::{
    ChainSyncError, ChainSyncMessage, ChainSyncServerState, Point, Tip,
};
use cardano_consensus::ouroboros::SlotNo;
use cardano_crypto::Blake2b256Hash;
use cardano_storage::chaindb::ChainDatabase;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, info, warn};

/// ChainSync server backed by persistent database
///
/// This server uses a ChainDatabase to serve blocks to clients. It maintains
/// a cursor position per client to track sync progress and queries the database
/// for blocks as needed.
pub struct ChainSyncDbServer<C: ChainDatabase> {
    /// Database for persistent storage
    chaindb: Arc<C>,
    /// Server state (Idle, ServingNext, ServingIntersection)
    state: Arc<RwLock<ChainSyncServerState>>,
    /// Outbound message channel
    outbound_tx: mpsc::UnboundedSender<ChainSyncMessage>,
    /// Inbound message channel
    inbound_rx: Arc<Mutex<mpsc::UnboundedReceiver<ChainSyncMessage>>>,
    /// Client cursor tracking (maps client to their last seen block)
    client_cursor: Arc<Mutex<Option<Blake2b256Hash>>>,
}

impl<C: ChainDatabase + 'static> ChainSyncDbServer<C> {
    /// Create a new database-backed ChainSync server
    ///
    /// Returns the server instance along with channels for message communication:
    /// - outbound_rx: Receives messages from the server to send to client
    /// - inbound_tx: Sends messages from client to the server
    pub fn new(
        chaindb: Arc<C>,
    ) -> (
        Self,
        mpsc::UnboundedReceiver<ChainSyncMessage>,
        mpsc::UnboundedSender<ChainSyncMessage>,
    ) {
        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();

        let server = Self {
            chaindb,
            state: Arc::new(RwLock::new(ChainSyncServerState::Idle)),
            outbound_tx,
            inbound_rx: Arc::new(Mutex::new(inbound_rx)),
            client_cursor: Arc::new(Mutex::new(None)),
        };

        (server, outbound_rx, inbound_tx)
    }

    /// Get current server state
    pub async fn get_state(&self) -> ChainSyncServerState {
        self.state.read().await.clone()
    }

    /// Get current tip from database
    ///
    /// Queries chain metadata from the database to construct the current tip.
    pub async fn get_tip(&self) -> Result<Tip, ChainSyncError> {
        let metadata = self
            .chaindb
            .get_chain_metadata()
            .await
            .map_err(|e| ChainSyncError::InvalidState(format!("Failed to get metadata: {}", e)))?
            .ok_or_else(|| ChainSyncError::InvalidState("No chain metadata found".to_string()))?;

        Ok(Tip {
            slot: SlotNo(metadata.current_slot),
            hash: metadata.tip_hash,
            height: metadata.tip_height,
        })
    }

    /// Process incoming message from client
    ///
    /// Handles FindIntersect and RequestNext messages, updating server state
    /// and sending responses via the outbound channel.
    pub async fn handle_message(&self, message: ChainSyncMessage) -> Result<(), ChainSyncError> {
        let current_state = self.state.read().await.clone();

        match (current_state, message) {
            (ChainSyncServerState::Idle, ChainSyncMessage::RequestNext) => {
                {
                    let mut state = self.state.write().await;
                    *state = ChainSyncServerState::ServingNext;
                }

                let response = self.serve_next().await?;
                self.outbound_tx
                    .send(response)
                    .map_err(|_| ChainSyncError::ConnectionClosed)?;

                {
                    let mut state = self.state.write().await;
                    *state = ChainSyncServerState::Idle;
                }

                Ok(())
            }

            (ChainSyncServerState::Idle, ChainSyncMessage::FindIntersect { points }) => {
                {
                    let mut state = self.state.write().await;
                    *state = ChainSyncServerState::ServingIntersection;
                }

                let response = self.find_intersection(points).await?;
                self.outbound_tx
                    .send(response)
                    .map_err(|_| ChainSyncError::ConnectionClosed)?;

                {
                    let mut state = self.state.write().await;
                    *state = ChainSyncServerState::Idle;
                }

                Ok(())
            }

            (state, _) => Err(ChainSyncError::UnexpectedMessage(format!(
                "Received unexpected message in state {:?}",
                state
            ))),
        }
    }

    /// Wait for and handle the next client request
    ///
    /// Blocks until a message is received on the inbound channel, then processes it.
    pub async fn serve_request(&self) -> Result<(), ChainSyncError> {
        let message = {
            let mut rx = self.inbound_rx.lock().await;
            rx.recv().await
        };

        if let Some(message) = message {
            self.handle_message(message).await
        } else {
            Err(ChainSyncError::ConnectionClosed)
        }
    }

    /// Serve next block request
    ///
    /// **Note:** This is a stub implementation. Full functionality requires
    /// BlockHeader CBOR serialization support. Once that's available, this
    /// will deserialize blocks from the database and return them.
    ///
    /// The intended flow:
    /// 1. Query database for next block after cursor
    /// 2. Deserialize BlockHeader from CBOR
    /// 3. Update cursor to new block
    /// 4. Return RollForward message with header
    async fn serve_next(&self) -> Result<ChainSyncMessage, ChainSyncError> {
        let tip = self.get_tip().await?;
        let cursor = self.client_cursor.lock().await.clone();

        match cursor {
            Some(last_hash) => {
                debug!("Serving next block after {:?}", last_hash);

                // Query database for next block
                let _blocks = self
                    .chaindb
                    .get_blocks_range(&last_hash, 1)
                    .await
                    .map_err(|e| {
                        ChainSyncError::InvalidState(format!("Failed to get next block: {}", e))
                    })?;

                // TODO: Deserialize BlockHeader once CBOR support is added
                // let header: BlockHeader = minicbor::decode(&blocks[0])?;
                // Update cursor: *self.client_cursor.lock().await = Some(header.block_body_hash);
                // Return: ChainSyncMessage::RollForward { header: Box::new(header), tip }

                warn!("BlockHeader CBOR deserialization not yet implemented");
                Err(ChainSyncError::SerializationError(
                    "BlockHeader CBOR support required for serve_next".to_string(),
                ))
            }
            None => {
                warn!("Client requested next without intersection");
                Ok(ChainSyncMessage::IntersectNotFound { tip })
            }
        }
    }

    /// Find intersection with client points
    ///
    /// Searches the database for the most recent point from the client's list
    /// that exists in our chain. Sets the cursor to the found point.
    async fn find_intersection(
        &self,
        points: Vec<Point>,
    ) -> Result<ChainSyncMessage, ChainSyncError> {
        let tip = self.get_tip().await?;
        let num_points = points.len();

        // Search for intersection point
        for point in points {
            let exists = self.chaindb.has_block(&point.hash).await.map_err(|e| {
                ChainSyncError::InvalidState(format!("Failed to check block: {}", e))
            })?;

            if exists {
                info!(
                    "Intersection found at slot={}, hash={:?}",
                    point.slot.0, point.hash
                );

                // Set cursor to intersection point
                *self.client_cursor.lock().await = Some(point.hash);

                return Ok(ChainSyncMessage::IntersectFound {
                    point: point.clone(),
                    tip,
                });
            }
        }

        warn!("No intersection found with {} points", num_points);
        Ok(ChainSyncMessage::IntersectNotFound { tip })
    }

    /// Reset server state and cursor
    ///
    /// Clears the client cursor and resets the state machine to Idle.
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        *state = ChainSyncServerState::Idle;
        *self.client_cursor.lock().await = None;
        debug!("Server reset to Idle state");
    }

    /// Set the client cursor to a specific block hash
    ///
    /// This is useful for resuming sync from a known point.
    pub async fn set_cursor(&self, block_hash: Blake2b256Hash) {
        *self.client_cursor.lock().await = Some(block_hash);
        debug!("Client cursor set to {:?}", block_hash);
    }

    /// Get the current client cursor
    pub async fn get_cursor(&self) -> Option<Blake2b256Hash> {
        self.client_cursor.lock().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cardano_storage::backends::MemoryBackend;
    use cardano_storage::chaindb::{ChainDatabaseImpl, ChainMetadata};

    async fn create_test_chaindb() -> Arc<ChainDatabaseImpl<MemoryBackend>> {
        let backend = Arc::new(MemoryBackend::new());
        let chaindb = Arc::new(ChainDatabaseImpl::new(backend));

        let metadata = ChainMetadata {
            genesis_hash: Blake2b256Hash::hash(b"genesis"),
            tip_hash: Blake2b256Hash::hash(b"block1"),
            tip_height: 1,
            current_slot: 100,
            current_epoch: 0,
            network_magic: 764824073, // preview
        };

        chaindb.store_chain_metadata(&metadata).await.unwrap();
        chaindb
    }

    #[tokio::test]
    async fn test_db_server_creation() {
        let chaindb = create_test_chaindb().await;
        let (server, _rx, _tx) = ChainSyncDbServer::new(chaindb);

        assert!(matches!(
            server.get_state().await,
            ChainSyncServerState::Idle
        ));
    }

    #[tokio::test]
    async fn test_get_tip_from_db() {
        let chaindb = create_test_chaindb().await;
        let (server, _rx, _tx) = ChainSyncDbServer::new(chaindb);

        let tip = server.get_tip().await.unwrap();
        assert_eq!(tip.height, 1);
        assert_eq!(tip.slot.0, 100);
    }

    #[tokio::test]
    async fn test_cursor_management() {
        let chaindb = create_test_chaindb().await;
        let (server, _rx, _tx) = ChainSyncDbServer::new(chaindb);

        assert!(server.get_cursor().await.is_none());

        let hash = Blake2b256Hash::hash(b"test");
        server.set_cursor(hash).await;

        assert_eq!(server.get_cursor().await, Some(hash));
    }

    #[tokio::test]
    async fn test_reset_clears_cursor() {
        let chaindb = create_test_chaindb().await;
        let (server, _rx, _tx) = ChainSyncDbServer::new(chaindb);

        let hash = Blake2b256Hash::hash(b"test");
        server.set_cursor(hash).await;
        assert!(server.get_cursor().await.is_some());

        server.reset().await;
        assert!(server.get_cursor().await.is_none());
    }

    #[tokio::test]
    async fn test_intersection_finding() {
        let chaindb = create_test_chaindb().await;

        // Store a test block
        let block_hash = Blake2b256Hash::hash(b"testblock");
        chaindb
            .store_block(&block_hash, 1, b"blockdata", &[])
            .await
            .unwrap();

        let (server, _rx, _tx) = ChainSyncDbServer::new(chaindb);

        // Try to find intersection
        let points = vec![
            Point {
                slot: SlotNo(50),
                hash: Blake2b256Hash::hash(b"notfound"),
            },
            Point {
                slot: SlotNo(100),
                hash: block_hash,
            },
        ];

        let result = server.find_intersection(points).await.unwrap();

        match result {
            ChainSyncMessage::IntersectFound { point, .. } => {
                assert_eq!(point.hash, block_hash);
                assert_eq!(point.slot.0, 100);
            }
            _ => panic!("Expected IntersectFound"),
        }

        // Cursor should be set to intersection
        assert_eq!(server.get_cursor().await, Some(block_hash));
    }
}
