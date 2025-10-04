//! Block Broadcasting Service
//!
//! Responsible for propagating newly forged blocks to connected network peers.
//! This service receives blocks from the BlockProductionService and broadcasts
//! them via the ChainSync protocol to all connected peers.
//!
//! The broadcaster maintains statistics about broadcast operations and provides
//! event notifications for monitoring and debugging.

use crate::{
    block_production::{BlockHeader, ForgedBlock},
    ConsensusError, Result,
};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::{interval, Duration};

/// Configuration for the block broadcaster
#[derive(Debug, Clone)]
pub struct BlockBroadcasterConfig {
    /// Maximum number of blocks to queue for broadcasting
    pub max_queue_size: usize,
    /// Timeout for individual broadcast operations (in milliseconds)
    pub broadcast_timeout_ms: u64,
    /// Whether to retry failed broadcasts
    pub retry_on_failure: bool,
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Channel size for events
    pub event_channel_size: usize,
}

impl Default for BlockBroadcasterConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 100,
            broadcast_timeout_ms: 5000,
            retry_on_failure: true,
            max_retries: 3,
            event_channel_size: 100,
        }
    }
}

/// Events emitted by the block broadcaster
#[derive(Debug, Clone)]
pub enum BroadcastEvent {
    /// Block added to broadcast queue
    BlockQueued {
        slot: u64,
        block_number: u64,
        block_hash: String,
    },
    /// Block successfully broadcast to a peer
    BlockBroadcast {
        slot: u64,
        block_number: u64,
        peer_count: usize,
    },
    /// Broadcast failed for a block
    BroadcastFailed {
        slot: u64,
        block_number: u64,
        error: String,
        retry_count: u32,
    },
    /// Broadcast queue is full
    QueueFull { dropped_blocks: u64 },
}

/// Statistics tracked by the broadcaster
#[derive(Debug, Clone, Default)]
pub struct BroadcastStats {
    /// Total number of blocks broadcast
    pub blocks_broadcast: u64,
    /// Total number of blocks dropped due to full queue
    pub blocks_dropped: u64,
    /// Total number of broadcast failures
    pub broadcast_failures: u64,
    /// Total number of broadcast retries
    pub broadcast_retries: u64,
    /// Total number of peers blocks were sent to
    pub total_peer_broadcasts: u64,
    /// Number of blocks currently queued
    pub current_queue_size: usize,
    /// Average broadcast latency in milliseconds
    pub avg_broadcast_latency_ms: u64,
}

/// Peer connection handle for broadcasting
pub trait PeerConnection: Send + Sync {
    /// Broadcast a block header to this peer
    fn broadcast_block(&self, header: BlockHeader) -> Result<()>;

    /// Get peer identifier
    fn peer_id(&self) -> String;

    /// Check if peer is still connected
    fn is_connected(&self) -> bool;
}

/// Block broadcaster service
pub struct BlockBroadcaster {
    config: Arc<RwLock<BlockBroadcasterConfig>>,
    stats: Arc<RwLock<BroadcastStats>>,
    peers: Arc<RwLock<Vec<Arc<dyn PeerConnection>>>>,
    event_tx: broadcast::Sender<BroadcastEvent>,
}

impl BlockBroadcaster {
    /// Create a new block broadcaster
    pub fn new(config: BlockBroadcasterConfig) -> Self {
        let (event_tx, _) = broadcast::channel(config.event_channel_size);

        Self {
            config: Arc::new(RwLock::new(config)),
            stats: Arc::new(RwLock::new(BroadcastStats::default())),
            peers: Arc::new(RwLock::new(Vec::new())),
            event_tx,
        }
    }

    /// Subscribe to broadcast events
    pub fn subscribe(&self) -> broadcast::Receiver<BroadcastEvent> {
        self.event_tx.subscribe()
    }

    /// Get current statistics
    pub async fn stats(&self) -> BroadcastStats {
        self.stats.read().await.clone()
    }

    /// Add a peer connection for broadcasting
    pub async fn add_peer(&self, peer: Arc<dyn PeerConnection>) {
        let mut peers = self.peers.write().await;
        tracing::info!("Adding peer {} for block broadcasting", peer.peer_id());
        peers.push(peer);
    }

    /// Remove a peer connection
    pub async fn remove_peer(&self, peer_id: &str) {
        let mut peers = self.peers.write().await;
        peers.retain(|p| p.peer_id() != peer_id);
        tracing::info!("Removed peer {} from block broadcasting", peer_id);
    }

    /// Get count of connected peers
    pub async fn peer_count(&self) -> usize {
        let peers = self.peers.read().await;
        peers.iter().filter(|p| p.is_connected()).count()
    }

    /// Broadcast a block to all connected peers
    async fn broadcast_block_to_peers(
        &self,
        header: BlockHeader,
    ) -> Result<usize> {
        let peers = self.peers.read().await;
        let mut successful_broadcasts = 0;
        let mut failures = Vec::new();

        for peer in peers.iter() {
            if !peer.is_connected() {
                continue;
            }

            match peer.broadcast_block(header.clone()) {
                Ok(_) => {
                    successful_broadcasts += 1;
                    tracing::debug!(
                        "Broadcast block {} to peer {}",
                        header.block_number,
                        peer.peer_id()
                    );
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    failures.push((peer.peer_id(), error_msg.clone()));
                    tracing::warn!(
                        "Failed to broadcast block {} to peer {}: {}",
                        header.block_number,
                        peer.peer_id(),
                        error_msg
                    );
                }
            }
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_peer_broadcasts += successful_broadcasts as u64;
            if !failures.is_empty() {
                stats.broadcast_failures += failures.len() as u64;
            }
        }

        if successful_broadcasts == 0 && !peers.is_empty() {
            return Err(ConsensusError::BlockProductionError(
                "Failed to broadcast block to any peers".to_string(),
            ));
        }

        Ok(successful_broadcasts)
    }

    /// Run the block broadcaster, receiving blocks from a channel
    pub async fn run(
        self: Arc<Self>,
        mut block_rx: mpsc::Receiver<ForgedBlock>,
    ) -> Result<()> {
        tracing::info!("Starting block broadcaster");

        let mut stats_ticker = interval(Duration::from_secs(10));

        loop {
            tokio::select! {
                Some(forged_block) = block_rx.recv() => {
                    self.handle_forged_block(forged_block).await?;
                }
                _ = stats_ticker.tick() => {
                    self.log_stats().await;
                }
                else => {
                    tracing::info!("Block broadcaster shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle a newly forged block
    async fn handle_forged_block(&self, forged_block: ForgedBlock) -> Result<()> {
        let slot = forged_block.header.slot.0;
        let block_number = forged_block.header.block_number;
        let block_hash = hex::encode(forged_block.header.block_body_hash.as_bytes());

        tracing::info!(
            "Broadcasting block {} at slot {} (hash: {})",
            block_number,
            slot,
            &block_hash[..16]
        );

        // Emit queued event
        let _ = self.event_tx.send(BroadcastEvent::BlockQueued {
            slot,
            block_number,
            block_hash: block_hash.clone(),
        });

        // Broadcast to all peers with retry logic
        let config = self.config.read().await;
        let max_retries = if config.retry_on_failure {
            config.max_retries
        } else {
            0
        };
        drop(config);

        let mut retry_count = 0;
        let start_time = std::time::Instant::now();

        loop {
            match self.broadcast_block_to_peers(forged_block.header.clone()).await {
                Ok(peer_count) => {
                    let latency = start_time.elapsed().as_millis() as u64;

                    // Update stats
                    {
                        let mut stats = self.stats.write().await;
                        stats.blocks_broadcast += 1;

                        // Update average latency (running average)
                        if stats.blocks_broadcast == 1 {
                            stats.avg_broadcast_latency_ms = latency;
                        } else {
                            stats.avg_broadcast_latency_ms =
                                (stats.avg_broadcast_latency_ms * (stats.blocks_broadcast - 1) + latency)
                                / stats.blocks_broadcast;
                        }
                    }

                    // Emit success event
                    let _ = self.event_tx.send(BroadcastEvent::BlockBroadcast {
                        slot,
                        block_number,
                        peer_count,
                    });

                    tracing::info!(
                        "Successfully broadcast block {} to {} peers in {}ms",
                        block_number,
                        peer_count,
                        latency
                    );

                    break;
                }
                Err(e) => {
                    retry_count += 1;

                    // Emit failure event
                    let _ = self.event_tx.send(BroadcastEvent::BroadcastFailed {
                        slot,
                        block_number,
                        error: e.to_string(),
                        retry_count,
                    });

                    if retry_count > max_retries {
                        let mut stats = self.stats.write().await;
                        stats.blocks_dropped += 1;
                        stats.broadcast_failures += 1;

                        tracing::error!(
                            "Failed to broadcast block {} after {} retries: {}",
                            block_number,
                            retry_count,
                            e
                        );
                        break;
                    }

                    // Update retry stats
                    {
                        let mut stats = self.stats.write().await;
                        stats.broadcast_retries += 1;
                    }

                    tracing::warn!(
                        "Broadcast failed for block {}, retrying ({}/{}): {}",
                        block_number,
                        retry_count,
                        max_retries,
                        e
                    );

                    // Wait before retry (exponential backoff)
                    tokio::time::sleep(Duration::from_millis(100 * (1 << retry_count))).await;
                }
            }
        }

        Ok(())
    }

    /// Log current statistics
    async fn log_stats(&self) {
        let stats = self.stats.read().await;
        let peer_count = self.peer_count().await;

        tracing::info!(
            "Broadcast stats: {} blocks broadcast, {} dropped, {} failures, {} retries, {} peers, avg latency {}ms",
            stats.blocks_broadcast,
            stats.blocks_dropped,
            stats.broadcast_failures,
            stats.broadcast_retries,
            peer_count,
            stats.avg_broadcast_latency_ms
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_production::{BlockBody, OperationalCertificate};
    use crate::leadership::LeadershipProof;
    use crate::ouroboros::{PoolId, SlotNo};
    use cardano_crypto::{Blake2b256Hash, Ed25519KeyHash, KesSignature, VrfOutput, VrfProof};

    // Mock peer connection for testing
    struct MockPeer {
        id: String,
        connected: bool,
        should_fail: bool,
    }

    impl PeerConnection for MockPeer {
        fn broadcast_block(&self, _header: BlockHeader) -> Result<()> {
            if self.should_fail {
                Err(ConsensusError::BlockProductionError(
                    "Mock broadcast failure".to_string(),
                ))
            } else {
                Ok(())
            }
        }

        fn peer_id(&self) -> String {
            self.id.clone()
        }

        fn is_connected(&self) -> bool {
            self.connected
        }
    }

    fn create_test_header(slot: u64, block_number: u64) -> BlockHeader {
        BlockHeader {
            slot: SlotNo(slot),
            block_number,
            prev_hash: Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap(),
            issuer_vkey: Ed25519KeyHash::from_bytes([1u8; 20]),
            vrf_proof: VrfProof::from_bytes(&[2u8; 80]).unwrap(),
            vrf_output: VrfOutput::from_bytes(&[3u8; 64]).unwrap(),
            block_body_hash: Blake2b256Hash::from_bytes(&[4u8; 32]).unwrap(),
            block_size: 1000,
            operational_cert: OperationalCertificate {
                hot_vkey: Ed25519KeyHash::from_bytes([5u8; 20]),
                sequence_number: 1,
                kes_period: 0,
                sigma: Blake2b256Hash::from_bytes(&[6u8; 32]).unwrap(),
            },
            protocol_magic: 764824073,
        }
    }

    fn create_test_forged_block(slot: u64, block_number: u64) -> ForgedBlock {
        let vrf_proof = VrfProof::from_bytes(&[2u8; 80]).unwrap();
        let vrf_output = VrfOutput::from_bytes(&[3u8; 64]).unwrap();

        ForgedBlock {
            header: create_test_header(slot, block_number),
            body: BlockBody {
                transactions: vec![],
                total_fee: 0,
                total_size: 0,
            },
            proof_of_leadership: LeadershipProof {
                slot: SlotNo(slot),
                vrf_proof,
                vrf_output,
                pool_id: PoolId(Blake2b256Hash::from_bytes(&[8u8; 32]).unwrap()),
            },
            kes_signature: KesSignature::from_bytes(&[7u8; 448]).unwrap(),
        }
    }    #[tokio::test]
    async fn test_broadcaster_creation() {
        let config = BlockBroadcasterConfig::default();
        let broadcaster = BlockBroadcaster::new(config);

        let stats = broadcaster.stats().await;
        assert_eq!(stats.blocks_broadcast, 0);
        assert_eq!(stats.blocks_dropped, 0);
    }

    #[tokio::test]
    async fn test_add_remove_peers() {
        let broadcaster = Arc::new(BlockBroadcaster::new(BlockBroadcasterConfig::default()));

        // Add peers
        let peer1 = Arc::new(MockPeer {
            id: "peer1".to_string(),
            connected: true,
            should_fail: false,
        });
        let peer2 = Arc::new(MockPeer {
            id: "peer2".to_string(),
            connected: true,
            should_fail: false,
        });

        broadcaster.add_peer(peer1.clone()).await;
        broadcaster.add_peer(peer2.clone()).await;

        assert_eq!(broadcaster.peer_count().await, 2);

        // Remove peer
        broadcaster.remove_peer("peer1").await;
        assert_eq!(broadcaster.peer_count().await, 1);
    }

    #[tokio::test]
    async fn test_broadcast_to_peers() {
        let broadcaster = Arc::new(BlockBroadcaster::new(BlockBroadcasterConfig::default()));

        // Add successful peer
        let peer = Arc::new(MockPeer {
            id: "peer1".to_string(),
            connected: true,
            should_fail: false,
        });
        broadcaster.add_peer(peer).await;

        // Create test block
        let header = create_test_header(100, 50);

        // Broadcast
        let result = broadcaster.broadcast_block_to_peers(header).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        // Check stats
        let stats = broadcaster.stats().await;
        assert_eq!(stats.total_peer_broadcasts, 1);
    }

    #[tokio::test]
    async fn test_broadcast_with_failures() {
        let broadcaster = Arc::new(BlockBroadcaster::new(BlockBroadcasterConfig::default()));

        // Add failing peer
        let peer = Arc::new(MockPeer {
            id: "peer1".to_string(),
            connected: true,
            should_fail: true,
        });
        broadcaster.add_peer(peer).await;

        // Create test block
        let header = create_test_header(100, 50);

        // Broadcast should fail
        let result = broadcaster.broadcast_block_to_peers(header).await;
        assert!(result.is_err());

        // Check stats
        let stats = broadcaster.stats().await;
        assert_eq!(stats.broadcast_failures, 1);
    }

    #[tokio::test]
    async fn test_event_emission() {
        let broadcaster = Arc::new(BlockBroadcaster::new(BlockBroadcasterConfig::default()));
        let mut events = broadcaster.subscribe();

        // Add peer
        let peer = Arc::new(MockPeer {
            id: "peer1".to_string(),
            connected: true,
            should_fail: false,
        });
        broadcaster.add_peer(peer).await;

        // Create channel for forged blocks
        let (tx, rx) = mpsc::channel(10);

        // Spawn broadcaster
        let broadcaster_clone = broadcaster.clone();
        tokio::spawn(async move {
            let _ = broadcaster_clone.run(rx).await;
        });

        // Send a forged block
        let forged_block = create_test_forged_block(100, 50);
        tx.send(forged_block).await.unwrap();

        // Receive events
        let event1 = tokio::time::timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();

        match event1 {
            BroadcastEvent::BlockQueued { slot, block_number, .. } => {
                assert_eq!(slot, 100);
                assert_eq!(block_number, 50);
            }
            _ => panic!("Expected BlockQueued event"),
        }

        let event2 = tokio::time::timeout(Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();

        match event2 {
            BroadcastEvent::BlockBroadcast { peer_count, .. } => {
                assert_eq!(peer_count, 1);
            }
            _ => panic!("Expected BlockBroadcast event"),
        }
    }
}
