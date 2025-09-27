//! ChainSync Protocol Implementation
//!
//! The ChainSync mini-protocol handles blockchain synchronization between Cardano nodes.
//! It allows nodes to find intersection points in their chains, request block headers,
//! and handle chain forks through rollback/rollforward mechanisms.
//!
//! # Protocol Overview
//!
//! The ChainSync protocol operates as a client-server interaction where:
//! - Client requests next block headers or finds intersections
//! - Server responds with block data or intersection information
//! - Both sides maintain consistent chain state
//!
//! # State Machine
//!
//! ```text
//! Idle -> RequestNext -> RollForward/RollBackward -> Idle
//!      -> FindIntersect -> IntersectFound/NotFound -> Idle
//! ```
//!
//! Based on the Cardano Network Protocol Specification.

use cardano_consensus::block_production::BlockHeader;
use cardano_consensus::ouroboros::SlotNo;
use cardano_crypto::Blake2b256Hash;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// A point on the blockchain identified by slot and hash
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Point {
    pub slot: SlotNo,
    pub hash: Blake2b256Hash,
}

impl Point {
    /// Genesis point (slot 0 with null hash)
    pub fn genesis() -> Self {
        Self {
            slot: SlotNo(0),
            hash: Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap(),
        }
    }

    /// Create a new point with given slot and hash bytes
    pub fn new(slot: u64, hash_bytes: &[u8; 32]) -> Self {
        Self {
            slot: SlotNo(slot),
            hash: Blake2b256Hash::from_bytes(hash_bytes).unwrap(),
        }
    }

    /// Create point from block header
    pub fn from_header(header: &BlockHeader) -> Self {
        // For now, use block_body_hash as the point hash
        // In a real implementation, this would be the actual block hash
        Self {
            slot: header.slot,
            hash: header.block_body_hash.clone(),
        }
    }
}

/// Chain tip information including slot, hash, and block height
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tip {
    pub slot: SlotNo,
    pub hash: Blake2b256Hash,
    pub height: u64,
}

impl Tip {
    /// Create a new tip
    pub fn new(slot: u64, height: u64, hash_bytes: &[u8; 32]) -> Self {
        Self {
            slot: SlotNo(slot),
            height,
            hash: Blake2b256Hash::from_bytes(hash_bytes).unwrap(),
        }
    }

    /// Create tip from block header
    pub fn from_header(header: &BlockHeader, height: u64) -> Self {
        Self {
            slot: header.slot,
            height,
            hash: header.block_body_hash.clone(),
        }
    }

    /// Get the point for this tip
    pub fn as_point(&self) -> Point {
        Point {
            slot: self.slot,
            hash: self.hash.clone(),
        }
    }
}

/// ChainSync protocol messages
#[derive(Debug, Clone)]
pub enum ChainSyncMessage {
    /// Client requests the next block header
    RequestNext,

    /// Client requests to find intersection with given points
    FindIntersect { points: Vec<Point> },

    /// Server responds with next block header (rollforward)
    RollForward { header: BlockHeader, tip: Tip },

    /// Server responds with rollback to a previous point
    RollBackward { point: Point, tip: Tip },

    /// Server found intersection at the given point
    IntersectFound { point: Point, tip: Tip },

    /// Server could not find intersection with any provided points
    IntersectNotFound { tip: Tip },
}

/// ChainSync protocol state for clients
#[derive(Debug, Clone, PartialEq)]
pub enum ChainSyncClientState {
    /// Waiting for user request
    Idle,
    /// Waiting for next block response
    WaitingForNext,
    /// Waiting for intersection response
    WaitingForIntersection,
}

/// ChainSync protocol state for servers
#[derive(Debug, Clone, PartialEq)]
pub enum ChainSyncServerState {
    /// Ready to serve requests
    Idle,
    /// Processing next block request
    ServingNext,
    /// Processing intersection request
    ServingIntersection,
}

/// ChainSync client implementation
pub struct ChainSyncClient {
    state: Arc<Mutex<ChainSyncClientState>>,
    outbound_tx: mpsc::UnboundedSender<ChainSyncMessage>,
    inbound_rx: Arc<Mutex<mpsc::UnboundedReceiver<ChainSyncMessage>>>,
    current_tip: Arc<Mutex<Option<Tip>>>,
    chain_points: Arc<Mutex<Vec<Point>>>,
}

impl ChainSyncClient {
    /// Create a new ChainSync client
    pub fn new() -> (Self, mpsc::UnboundedReceiver<ChainSyncMessage>, mpsc::UnboundedSender<ChainSyncMessage>) {
        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();

        let client = Self {
            state: Arc::new(Mutex::new(ChainSyncClientState::Idle)),
            outbound_tx,
            inbound_rx: Arc::new(Mutex::new(inbound_rx)),
            current_tip: Arc::new(Mutex::new(None)),
            chain_points: Arc::new(Mutex::new(Vec::new())),
        };

        (client, outbound_rx, inbound_tx)
    }

    /// Get current client state
    pub fn get_state(&self) -> ChainSyncClientState {
        self.state.lock().unwrap().clone()
    }

    /// Get current tip
    pub fn get_tip(&self) -> Option<Tip> {
        self.current_tip.lock().unwrap().clone()
    }

    /// Get known chain points
    pub fn get_chain_points(&self) -> Vec<Point> {
        self.chain_points.lock().unwrap().clone()
    }

    /// Request next block header
    pub async fn request_next(&self) -> Result<(), ChainSyncError> {
        let mut state = self.state.lock().unwrap();
        if *state != ChainSyncClientState::Idle {
            return Err(ChainSyncError::InvalidState(
                "Cannot request next in current state".to_string()
            ));
        }

        *state = ChainSyncClientState::WaitingForNext;
        drop(state);

        self.outbound_tx.send(ChainSyncMessage::RequestNext)
            .map_err(|_| ChainSyncError::ConnectionClosed)?;

        Ok(())
    }

    /// Request intersection with given points
    pub async fn find_intersect(&self, points: Vec<Point>) -> Result<(), ChainSyncError> {
        let mut state = self.state.lock().unwrap();
        if *state != ChainSyncClientState::Idle {
            return Err(ChainSyncError::InvalidState(
                "Cannot find intersection in current state".to_string()
            ));
        }

        // Validate points are in descending slot order
        if !self.validate_points_order(&points) {
            return Err(ChainSyncError::InvalidPoints(
                "Points must be in descending slot order".to_string()
            ));
        }

        *state = ChainSyncClientState::WaitingForIntersection;
        drop(state);

        self.outbound_tx.send(ChainSyncMessage::FindIntersect { points })
            .map_err(|_| ChainSyncError::ConnectionClosed)?;

        Ok(())
    }

    /// Process incoming message
    pub async fn handle_message(&self, message: ChainSyncMessage) -> Result<(), ChainSyncError> {
        let mut state = self.state.lock().unwrap();
        let mut tip = self.current_tip.lock().unwrap();
        let mut points = self.chain_points.lock().unwrap();

        match (&*state, message) {
            (ChainSyncClientState::WaitingForNext, ChainSyncMessage::RollForward { header, tip: new_tip }) => {
                // Validate rollforward
                if let Some(current_tip) = &*tip {
                    if header.slot.0 <= current_tip.slot.0 {
                        return Err(ChainSyncError::InvalidRollForward(
                            "New header slot must be greater than current tip".to_string()
                        ));
                    }
                }

                // Update state
                let point = Point::from_header(&header);
                points.push(point);
                *tip = Some(new_tip);
                *state = ChainSyncClientState::Idle;

                Ok(())
            }

            (ChainSyncClientState::WaitingForNext, ChainSyncMessage::RollBackward { point, tip: new_tip }) => {
                // Validate rollback point exists in our chain
                if !points.iter().any(|p| *p == point) {
                    return Err(ChainSyncError::InvalidRollback(
                        "Rollback point not found in chain".to_string()
                    ));
                }

                // Remove points after rollback point
                points.retain(|p| p.slot.0 <= point.slot.0);
                *tip = Some(new_tip);
                *state = ChainSyncClientState::Idle;

                Ok(())
            }

            (ChainSyncClientState::WaitingForIntersection, ChainSyncMessage::IntersectFound { point, tip: new_tip }) => {
                // Update chain to intersection point
                points.clear();
                points.push(point);
                *tip = Some(new_tip);
                *state = ChainSyncClientState::Idle;

                Ok(())
            }

            (ChainSyncClientState::WaitingForIntersection, ChainSyncMessage::IntersectNotFound { tip: new_tip }) => {
                // No intersection found, start from genesis
                points.clear();
                points.push(Point::genesis());
                *tip = Some(new_tip);
                *state = ChainSyncClientState::Idle;

                Ok(())
            }

            _ => Err(ChainSyncError::UnexpectedMessage(
                format!("Received unexpected message in state {:?}", *state)
            ))
        }
    }

    /// Wait for and handle the next message
    pub async fn receive_message(&self) -> Result<(), ChainSyncError> {
        let mut rx = self.inbound_rx.lock().unwrap();
        if let Some(message) = rx.recv().await {
            drop(rx);
            self.handle_message(message).await
        } else {
            Err(ChainSyncError::ConnectionClosed)
        }
    }

    /// Validate that points are in descending slot order
    fn validate_points_order(&self, points: &[Point]) -> bool {
        if points.is_empty() {
            return true;
        }

        for window in points.windows(2) {
            if window[0].slot.0 < window[1].slot.0 {
                return false;
            }
        }
        true
    }

    /// Reset client to initial state
    pub fn reset(&self) {
        let mut state = self.state.lock().unwrap();
        let mut tip = self.current_tip.lock().unwrap();
        let mut points = self.chain_points.lock().unwrap();

        *state = ChainSyncClientState::Idle;
        *tip = None;
        points.clear();
    }
}

/// ChainSync server implementation
pub struct ChainSyncServer {
    state: Arc<Mutex<ChainSyncServerState>>,
    outbound_tx: mpsc::UnboundedSender<ChainSyncMessage>,
    inbound_rx: Arc<Mutex<mpsc::UnboundedReceiver<ChainSyncMessage>>>,
    chain: Arc<Mutex<VecDeque<BlockHeader>>>,
    current_tip: Arc<Mutex<Tip>>,
}

impl ChainSyncServer {
    /// Create a new ChainSync server
    pub fn new(initial_chain: Vec<BlockHeader>) -> (Self, mpsc::UnboundedReceiver<ChainSyncMessage>, mpsc::UnboundedSender<ChainSyncMessage>) {
        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();

        let chain: VecDeque<_> = initial_chain.into_iter().collect();
        let tip = if let Some(header) = chain.back() {
            Tip::from_header(header, chain.len() as u64)
        } else {
            Tip::new(0, 0, &[0u8; 32])
        };

        let server = Self {
            state: Arc::new(Mutex::new(ChainSyncServerState::Idle)),
            outbound_tx,
            inbound_rx: Arc::new(Mutex::new(inbound_rx)),
            chain: Arc::new(Mutex::new(chain)),
            current_tip: Arc::new(Mutex::new(tip)),
        };

        (server, outbound_rx, inbound_tx)
    }

    /// Get current server state
    pub fn get_state(&self) -> ChainSyncServerState {
        self.state.lock().unwrap().clone()
    }

    /// Get current tip
    pub fn get_tip(&self) -> Tip {
        self.current_tip.lock().unwrap().clone()
    }

    /// Add a new block to the chain
    pub fn add_block(&self, header: BlockHeader) {
        let mut chain = self.chain.lock().unwrap();
        let mut tip = self.current_tip.lock().unwrap();

        chain.push_back(header.clone());
        *tip = Tip::from_header(&header, chain.len() as u64);
    }

    /// Process incoming message from client
    pub async fn handle_message(&self, message: ChainSyncMessage) -> Result<(), ChainSyncError> {
        let mut state = self.state.lock().unwrap();

        match (&*state, message) {
            (ChainSyncServerState::Idle, ChainSyncMessage::RequestNext) => {
                *state = ChainSyncServerState::ServingNext;
                drop(state);

                let response = self.serve_next().await?;
                self.outbound_tx.send(response)
                    .map_err(|_| ChainSyncError::ConnectionClosed)?;

                *self.state.lock().unwrap() = ChainSyncServerState::Idle;
                Ok(())
            }

            (ChainSyncServerState::Idle, ChainSyncMessage::FindIntersect { points }) => {
                *state = ChainSyncServerState::ServingIntersection;
                drop(state);

                let response = self.find_intersection(points).await?;
                self.outbound_tx.send(response)
                    .map_err(|_| ChainSyncError::ConnectionClosed)?;

                *self.state.lock().unwrap() = ChainSyncServerState::Idle;
                Ok(())
            }

            _ => Err(ChainSyncError::UnexpectedMessage(
                format!("Received unexpected message in state {:?}", *state)
            ))
        }
    }

    /// Wait for and handle the next client request
    pub async fn serve_request(&self) -> Result<(), ChainSyncError> {
        let mut rx = self.inbound_rx.lock().unwrap();
        if let Some(message) = rx.recv().await {
            drop(rx);
            self.handle_message(message).await
        } else {
            Err(ChainSyncError::ConnectionClosed)
        }
    }

    /// Serve next block request
    async fn serve_next(&self) -> Result<ChainSyncMessage, ChainSyncError> {
        let chain = self.chain.lock().unwrap();
        let tip = self.current_tip.lock().unwrap().clone();

        // For simplicity, always return the latest block
        // In a real implementation, this would track client position
        if let Some(header) = chain.back() {
            Ok(ChainSyncMessage::RollForward {
                header: header.clone(),
                tip,
            })
        } else {
            // No blocks available
            Ok(ChainSyncMessage::IntersectNotFound { tip })
        }
    }

    /// Find intersection with client points
    async fn find_intersection(&self, points: Vec<Point>) -> Result<ChainSyncMessage, ChainSyncError> {
        let chain = self.chain.lock().unwrap();
        let tip = self.current_tip.lock().unwrap().clone();

        // Find the first point that exists in our chain
        for point in points {
            if chain.iter().any(|header| {
                header.slot == point.slot && header.block_body_hash == point.hash
            }) {
                return Ok(ChainSyncMessage::IntersectFound { point, tip });
            }
        }

        // No intersection found
        Ok(ChainSyncMessage::IntersectNotFound { tip })
    }

    /// Reset server state
    pub fn reset(&self) {
        let mut state = self.state.lock().unwrap();
        *state = ChainSyncServerState::Idle;
    }
}

/// ChainSync protocol errors
#[derive(Debug, thiserror::Error)]
pub enum ChainSyncError {
    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Invalid points: {0}")]
    InvalidPoints(String),

    #[error("Invalid rollforward: {0}")]
    InvalidRollForward(String),

    #[error("Invalid rollback: {0}")]
    InvalidRollback(String),

    #[error("Unexpected message: {0}")]
    UnexpectedMessage(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Protocol timeout")]
    Timeout,

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// ChainSync protocol configuration
#[derive(Debug, Clone)]
pub struct ChainSyncConfig {
    /// Maximum number of points in intersection request
    pub max_intersection_points: usize,
    /// Timeout for protocol operations
    pub operation_timeout: std::time::Duration,
    /// Maximum chain length to maintain in memory
    pub max_chain_length: usize,
}

impl Default for ChainSyncConfig {
    fn default() -> Self {
        Self {
            max_intersection_points: 2160, // ~1.5 days at 1min slots
            operation_timeout: std::time::Duration::from_secs(30),
            max_chain_length: 10000,
        }
    }
}

/// Utility functions for ChainSync protocol

/// Create a mock chain of block headers for testing
pub fn create_mock_chain(length: usize) -> Vec<BlockHeader> {
    use cardano_consensus::block_production::OperationalCertificate;
    use cardano_crypto::{Ed25519KeyHash, VrfProof, VrfOutput};

    let mut headers: Vec<BlockHeader> = Vec::new();

    for i in 0..length {
        let hash_bytes = [(i as u8); 32];

        // VrfProof needs 81 bytes
        let mut vrf_proof_bytes = [0u8; 81];
        vrf_proof_bytes[0] = (i as u8 + 100);
        vrf_proof_bytes[80] = (i as u8 + 101);

        // VrfOutput needs 64 bytes
        let mut vrf_output_bytes = [0u8; 64];
        vrf_output_bytes[0] = (i as u8 + 150);
        vrf_output_bytes[63] = (i as u8 + 151);

        // Create 20-byte array for Ed25519KeyHash
        let mut key_bytes = [0u8; 20];
        key_bytes[0] = (i as u8 + 50);
        key_bytes[19] = (i as u8 + 51);

        let mut hot_key_bytes = [0u8; 20];
        hot_key_bytes[0] = (i as u8 + 75);
        hot_key_bytes[19] = (i as u8 + 76);

        headers.push(BlockHeader {
            slot: SlotNo(i as u64),
            prev_hash: if i == 0 {
                Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap()
            } else {
                headers[i-1].block_body_hash.clone()
            },
            issuer_vkey: Ed25519KeyHash::from_bytes(key_bytes),
            vrf_proof: VrfProof::from_bytes(&vrf_proof_bytes).unwrap(),
            vrf_output: VrfOutput::from_bytes(&vrf_output_bytes).unwrap(),
            block_body_hash: Blake2b256Hash::from_bytes(&hash_bytes).unwrap(),
            block_size: (1024 + (i * 10)) as u32,
            operational_cert: OperationalCertificate {
                hot_vkey: Ed25519KeyHash::from_bytes(hot_key_bytes),
                sequence_number: i as u64,
                kes_period: i as u64,
                sigma: Blake2b256Hash::from_bytes(&[(i as u8 + 200); 32]).unwrap(),
            },
            protocol_magic: 764824073, // Mainnet magic
        });
    }

    headers
}

/// Validate ChainSync message according to protocol rules
pub fn validate_message(message: &ChainSyncMessage, config: &ChainSyncConfig) -> Result<(), ChainSyncError> {
    match message {
        ChainSyncMessage::FindIntersect { points } => {
            if points.len() > config.max_intersection_points {
                return Err(ChainSyncError::InvalidPoints(
                    format!("Too many intersection points: {} > {}",
                        points.len(), config.max_intersection_points)
                ));
            }

            // Validate points are in descending order
            for window in points.windows(2) {
                if window[0].slot.0 < window[1].slot.0 {
                    return Err(ChainSyncError::InvalidPoints(
                        "Points must be in descending slot order".to_string()
                    ));
                }
            }
        }

        ChainSyncMessage::RollForward { header, tip } => {
            if header.slot.0 > tip.slot.0 {
                return Err(ChainSyncError::InvalidRollForward(
                    "Header slot cannot exceed tip slot".to_string()
                ));
            }
        }

        _ => {} // Other messages don't need validation
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;
    use std::time::Duration;

    #[tokio::test]
    async fn test_chainsync_client_creation() {
        let (client, _outbound_rx, _inbound_tx) = ChainSyncClient::new();
        assert_eq!(client.get_state(), ChainSyncClientState::Idle);
        assert!(client.get_tip().is_none());
        assert!(client.get_chain_points().is_empty());
    }

    #[tokio::test]
    async fn test_chainsync_server_creation() {
        let chain = create_mock_chain(5);
        let (server, _outbound_rx, _inbound_tx) = ChainSyncServer::new(chain);

        assert_eq!(server.get_state(), ChainSyncServerState::Idle);
        let tip = server.get_tip();
        assert_eq!(tip.height, 5);
        assert_eq!(tip.slot.0, 4);
    }

    #[tokio::test]
    async fn test_point_creation() {
        let point = Point::genesis();
        assert_eq!(point.slot.0, 0);

        let hash_bytes = [1u8; 32];
        let point2 = Point::new(42, &hash_bytes);
        assert_eq!(point2.slot.0, 42);
    }

    #[tokio::test]
    async fn test_tip_creation() {
        let hash_bytes = [1u8; 32];
        let tip = Tip::new(100, 50, &hash_bytes);
        assert_eq!(tip.slot.0, 100);
        assert_eq!(tip.height, 50);

        let point = tip.as_point();
        assert_eq!(point.slot.0, tip.slot.0);
        assert_eq!(point.hash, tip.hash);
    }

    #[tokio::test]
    async fn test_validate_points_order() {
        let client = ChainSyncClient::new().0;

        // Valid descending order
        let points = vec![
            Point::new(100, &[1u8; 32]),
            Point::new(50, &[2u8; 32]),
            Point::new(10, &[3u8; 32]),
        ];
        assert!(client.validate_points_order(&points));

        // Invalid ascending order
        let points = vec![
            Point::new(10, &[1u8; 32]),
            Point::new(50, &[2u8; 32]),
            Point::new(100, &[3u8; 32]),
        ];
        assert!(!client.validate_points_order(&points));

        // Empty is valid
        assert!(client.validate_points_order(&[]));
    }

    #[tokio::test]
    async fn test_message_validation() {
        let config = ChainSyncConfig::default();

        // Valid intersection request
        let points = vec![Point::new(100, &[1u8; 32])];
        let message = ChainSyncMessage::FindIntersect { points };
        assert!(validate_message(&message, &config).is_ok());

        // Too many points
        let points = vec![Point::genesis(); config.max_intersection_points + 1];
        let message = ChainSyncMessage::FindIntersect { points };
        assert!(validate_message(&message, &config).is_err());
    }
}
