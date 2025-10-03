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

use bytes::Bytes;
use cardano_consensus::block_production::{BlockHeader, OperationalCertificate};
use cardano_consensus::ouroboros::SlotNo;
use cardano_crypto::{Blake2b256Hash, Ed25519KeyHash, VrfOutput, VrfProof};
use minicbor::{Decode, Encode};
use std::collections::{HashMap, VecDeque};
use std::convert::TryFrom;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, Mutex as AsyncMutex};

use crate::connection::multiplexer::ProtocolHandler;
use crate::connection::{ConnectionId, ProtocolId};
use crate::{NetworkError, Result as NetworkResult};

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
            hash: header.block_body_hash,
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
            hash: header.block_body_hash,
        }
    }

    /// Get the point for this tip
    pub fn as_point(&self) -> Point {
        Point {
            slot: self.slot,
            hash: self.hash,
        }
    }
}

#[derive(Debug, Clone, Encode, Decode)]
pub struct PointWire {
    #[n(0)]
    slot: u64,
    #[n(1)]
    hash: Blake2b256Hash,
}

#[derive(Debug, Clone, Encode, Decode)]
pub struct TipWire {
    #[n(0)]
    slot: u64,
    #[n(1)]
    hash: Blake2b256Hash,
    #[n(2)]
    height: u64,
}

#[derive(Debug, Clone, Encode, Decode)]
pub struct OperationalCertificateWire {
    #[n(0)]
    hot_vkey: [u8; 20],
    #[n(1)]
    sequence_number: u64,
    #[n(2)]
    kes_period: u64,
    #[n(3)]
    sigma: Blake2b256Hash,
}

#[derive(Debug, Clone, Encode, Decode)]
pub struct BlockHeaderWire {
    #[n(0)]
    slot: u64,
    #[n(1)]
    prev_hash: Blake2b256Hash,
    #[n(2)]
    issuer_vkey: [u8; 20],
    #[n(3)]
    vrf_proof: Vec<u8>,
    #[n(4)]
    vrf_output: Vec<u8>,
    #[n(5)]
    block_body_hash: Blake2b256Hash,
    #[n(6)]
    block_size: u32,
    #[n(7)]
    operational_cert: OperationalCertificateWire,
    #[n(8)]
    protocol_magic: u32,
}

#[derive(Debug, Clone, Encode, Decode)]
pub enum ChainSyncWireMessage {
    #[n(0)]
    RequestNext,
    #[n(1)]
    FindIntersect {
        #[n(0)]
        points: Vec<PointWire>,
    },
    #[n(2)]
    RollForward {
        #[n(0)]
        header: BlockHeaderWire,
        #[n(1)]
        tip: TipWire,
    },
    #[n(3)]
    RollBackward {
        #[n(0)]
        point: PointWire,
        #[n(1)]
        tip: TipWire,
    },
    #[n(4)]
    IntersectFound {
        #[n(0)]
        point: PointWire,
        #[n(1)]
        tip: TipWire,
    },
    #[n(5)]
    IntersectNotFound {
        #[n(0)]
        tip: TipWire,
    },
}

impl ChainSyncWireMessage {
    fn from_domain(message: ChainSyncMessage) -> Result<Self, ChainSyncError> {
        match message {
            ChainSyncMessage::RequestNext => Ok(Self::RequestNext),
            ChainSyncMessage::FindIntersect { points } => {
                let wires = points.into_iter().map(PointWire::from).collect();
                Ok(Self::FindIntersect { points: wires })
            }
            ChainSyncMessage::RollForward { header, tip } => Ok(Self::RollForward {
                header: BlockHeaderWire::from(&*header),
                tip: TipWire::from(tip),
            }),
            ChainSyncMessage::RollBackward { point, tip } => Ok(Self::RollBackward {
                point: PointWire::from(point),
                tip: TipWire::from(tip),
            }),
            ChainSyncMessage::IntersectFound { point, tip } => Ok(Self::IntersectFound {
                point: PointWire::from(point),
                tip: TipWire::from(tip),
            }),
            ChainSyncMessage::IntersectNotFound { tip } => Ok(Self::IntersectNotFound {
                tip: TipWire::from(tip),
            }),
        }
    }

    fn into_domain(self) -> Result<ChainSyncMessage, ChainSyncError> {
        match self {
            Self::RequestNext => Ok(ChainSyncMessage::RequestNext),
            Self::FindIntersect { points } => {
                let points = points.into_iter().map(Point::from).collect();
                Ok(ChainSyncMessage::FindIntersect { points })
            }
            Self::RollForward { header, tip } => Ok(ChainSyncMessage::RollForward {
                header: Box::new(BlockHeader::try_from(header)?),
                tip: Tip::from(tip),
            }),
            Self::RollBackward { point, tip } => Ok(ChainSyncMessage::RollBackward {
                point: Point::from(point),
                tip: Tip::from(tip),
            }),
            Self::IntersectFound { point, tip } => Ok(ChainSyncMessage::IntersectFound {
                point: Point::from(point),
                tip: Tip::from(tip),
            }),
            Self::IntersectNotFound { tip } => Ok(ChainSyncMessage::IntersectNotFound {
                tip: Tip::from(tip),
            }),
        }
    }
}

impl From<&Point> for PointWire {
    fn from(point: &Point) -> Self {
        Self {
            slot: point.slot.0,
            hash: point.hash,
        }
    }
}

impl From<Point> for PointWire {
    fn from(point: Point) -> Self {
        Self::from(&point)
    }
}

impl From<&Tip> for TipWire {
    fn from(tip: &Tip) -> Self {
        Self {
            slot: tip.slot.0,
            hash: tip.hash,
            height: tip.height,
        }
    }
}

impl From<Tip> for TipWire {
    fn from(tip: Tip) -> Self {
        Self::from(&tip)
    }
}

impl From<&OperationalCertificate> for OperationalCertificateWire {
    fn from(cert: &OperationalCertificate) -> Self {
        Self {
            hot_vkey: *cert.hot_vkey.as_bytes(),
            sequence_number: cert.sequence_number,
            kes_period: cert.kes_period,
            sigma: cert.sigma,
        }
    }
}

impl From<&BlockHeader> for BlockHeaderWire {
    fn from(header: &BlockHeader) -> Self {
        Self {
            slot: header.slot.0,
            prev_hash: header.prev_hash,
            issuer_vkey: *header.issuer_vkey.as_bytes(),
            vrf_proof: header.vrf_proof.to_bytes().to_vec(),
            vrf_output: header.vrf_output.to_bytes().to_vec(),
            block_body_hash: header.block_body_hash,
            block_size: header.block_size,
            operational_cert: OperationalCertificateWire::from(&header.operational_cert),
            protocol_magic: header.protocol_magic,
        }
    }
}

impl From<PointWire> for Point {
    fn from(point: PointWire) -> Self {
        Self {
            slot: SlotNo(point.slot),
            hash: point.hash,
        }
    }
}

impl From<TipWire> for Tip {
    fn from(tip: TipWire) -> Self {
        Self {
            slot: SlotNo(tip.slot),
            hash: tip.hash,
            height: tip.height,
        }
    }
}

impl TryFrom<OperationalCertificateWire> for OperationalCertificate {
    type Error = ChainSyncError;

    fn try_from(wire: OperationalCertificateWire) -> Result<Self, Self::Error> {
        let hot_vkey = Ed25519KeyHash::from_bytes(wire.hot_vkey);
        Ok(Self {
            hot_vkey,
            sequence_number: wire.sequence_number,
            kes_period: wire.kes_period,
            sigma: wire.sigma,
        })
    }
}

impl TryFrom<BlockHeaderWire> for BlockHeader {
    type Error = ChainSyncError;

    fn try_from(wire: BlockHeaderWire) -> Result<Self, Self::Error> {
        let vrf_proof = VrfProof::from_bytes(&wire.vrf_proof)
            .map_err(|err| ChainSyncError::SerializationError(err.to_string()))?;
        let vrf_output = VrfOutput::from_bytes(&wire.vrf_output)
            .map_err(|err| ChainSyncError::SerializationError(err.to_string()))?;
        let operational_cert = OperationalCertificate::try_from(wire.operational_cert)?;

        Ok(Self {
            slot: SlotNo(wire.slot),
            prev_hash: wire.prev_hash,
            issuer_vkey: Ed25519KeyHash::from_bytes(wire.issuer_vkey),
            vrf_proof,
            vrf_output,
            block_body_hash: wire.block_body_hash,
            block_size: wire.block_size,
            operational_cert,
            protocol_magic: wire.protocol_magic,
        })
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
    RollForward { header: Box<BlockHeader>, tip: Tip },

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
    inbound_rx: Arc<AsyncMutex<mpsc::UnboundedReceiver<ChainSyncMessage>>>,
    current_tip: Arc<Mutex<Option<Tip>>>,
    chain_points: Arc<Mutex<Vec<Point>>>,
}

impl ChainSyncClient {
    /// Create a new ChainSync client
    pub fn new() -> (
        Self,
        mpsc::UnboundedReceiver<ChainSyncMessage>,
        mpsc::UnboundedSender<ChainSyncMessage>,
    ) {
        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();

        let client = Self {
            state: Arc::new(Mutex::new(ChainSyncClientState::Idle)),
            outbound_tx,
            inbound_rx: Arc::new(AsyncMutex::new(inbound_rx)),
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
                "Cannot request next in current state".to_string(),
            ));
        }

        *state = ChainSyncClientState::WaitingForNext;
        drop(state);

        self.outbound_tx
            .send(ChainSyncMessage::RequestNext)
            .map_err(|_| ChainSyncError::ConnectionClosed)?;

        Ok(())
    }

    /// Request intersection with given points
    pub async fn find_intersect(&self, points: Vec<Point>) -> Result<(), ChainSyncError> {
        let mut state = self.state.lock().unwrap();
        if *state != ChainSyncClientState::Idle {
            return Err(ChainSyncError::InvalidState(
                "Cannot find intersection in current state".to_string(),
            ));
        }

        // Validate points are in descending slot order
        if !self.validate_points_order(&points) {
            return Err(ChainSyncError::InvalidPoints(
                "Points must be in descending slot order".to_string(),
            ));
        }

        *state = ChainSyncClientState::WaitingForIntersection;
        drop(state);

        self.outbound_tx
            .send(ChainSyncMessage::FindIntersect { points })
            .map_err(|_| ChainSyncError::ConnectionClosed)?;

        Ok(())
    }

    /// Process incoming message
    pub async fn handle_message(&self, message: ChainSyncMessage) -> Result<(), ChainSyncError> {
        let mut state = self.state.lock().unwrap();
        let mut tip = self.current_tip.lock().unwrap();
        let mut points = self.chain_points.lock().unwrap();

        match (&*state, message) {
            (
                ChainSyncClientState::WaitingForNext,
                ChainSyncMessage::RollForward {
                    header,
                    tip: new_tip,
                },
            ) => {
                // Validate rollforward
                if let Some(current_tip) = &*tip {
                    if header.slot.0 <= current_tip.slot.0 {
                        return Err(ChainSyncError::InvalidRollForward(
                            "New header slot must be greater than current tip".to_string(),
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

            (
                ChainSyncClientState::WaitingForNext,
                ChainSyncMessage::RollBackward {
                    point,
                    tip: new_tip,
                },
            ) => {
                // Validate rollback point exists in our chain
                if !points.contains(&point) {
                    return Err(ChainSyncError::InvalidRollback(
                        "Rollback point not found in chain".to_string(),
                    ));
                }

                // Remove points after rollback point
                points.retain(|p| p.slot.0 <= point.slot.0);
                *tip = Some(new_tip);
                *state = ChainSyncClientState::Idle;

                Ok(())
            }

            (
                ChainSyncClientState::WaitingForIntersection,
                ChainSyncMessage::IntersectFound {
                    point,
                    tip: new_tip,
                },
            ) => {
                // Update chain to intersection point
                points.clear();
                points.push(point);
                *tip = Some(new_tip);
                *state = ChainSyncClientState::Idle;

                Ok(())
            }

            (
                ChainSyncClientState::WaitingForIntersection,
                ChainSyncMessage::IntersectNotFound { tip: new_tip },
            ) => {
                // No intersection found, start from genesis
                points.clear();
                points.push(Point::genesis());
                *tip = Some(new_tip);
                *state = ChainSyncClientState::Idle;

                Ok(())
            }

            _ => Err(ChainSyncError::UnexpectedMessage(format!(
                "Received unexpected message in state {:?}",
                *state
            ))),
        }
    }

    /// Wait for and handle the next message
    pub async fn receive_message(&self) -> Result<(), ChainSyncError> {
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
    inbound_rx: Arc<AsyncMutex<mpsc::UnboundedReceiver<ChainSyncMessage>>>,
    chain: Arc<Mutex<VecDeque<BlockHeader>>>,
    current_tip: Arc<Mutex<Tip>>,
}

impl ChainSyncServer {
    /// Create a new ChainSync server
    pub fn new(
        initial_chain: Vec<BlockHeader>,
    ) -> (
        Self,
        mpsc::UnboundedReceiver<ChainSyncMessage>,
        mpsc::UnboundedSender<ChainSyncMessage>,
    ) {
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
            inbound_rx: Arc::new(AsyncMutex::new(inbound_rx)),
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
        let current_state = { self.state.lock().unwrap().clone() };

        match (current_state, message) {
            (ChainSyncServerState::Idle, ChainSyncMessage::RequestNext) => {
                {
                    let mut state = self.state.lock().unwrap();
                    *state = ChainSyncServerState::ServingNext;
                }

                let response = self.serve_next().await?;
                self.outbound_tx
                    .send(response)
                    .map_err(|_| ChainSyncError::ConnectionClosed)?;

                {
                    let mut state = self.state.lock().unwrap();
                    *state = ChainSyncServerState::Idle;
                }

                Ok(())
            }

            (ChainSyncServerState::Idle, ChainSyncMessage::FindIntersect { points }) => {
                {
                    let mut state = self.state.lock().unwrap();
                    *state = ChainSyncServerState::ServingIntersection;
                }

                let response = self.find_intersection(points).await?;
                self.outbound_tx
                    .send(response)
                    .map_err(|_| ChainSyncError::ConnectionClosed)?;

                {
                    let mut state = self.state.lock().unwrap();
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
    async fn serve_next(&self) -> Result<ChainSyncMessage, ChainSyncError> {
        let chain = self.chain.lock().unwrap();
        let tip = self.current_tip.lock().unwrap().clone();

        // For simplicity, always return the latest block
        // In a real implementation, this would track client position
        if let Some(header) = chain.back() {
            Ok(ChainSyncMessage::RollForward {
                header: Box::new(header.clone()),
                tip,
            })
        } else {
            // No blocks available
            Ok(ChainSyncMessage::IntersectNotFound { tip })
        }
    }

    /// Find intersection with client points
    async fn find_intersection(
        &self,
        points: Vec<Point>,
    ) -> Result<ChainSyncMessage, ChainSyncError> {
        let chain = self.chain.lock().unwrap();
        let tip = self.current_tip.lock().unwrap().clone();

        // Find the first point that exists in our chain
        for point in points {
            if chain
                .iter()
                .any(|header| header.slot == point.slot && header.block_body_hash == point.hash)
            {
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
    use cardano_crypto::{
        Ed25519KeyHash, VrfOutput, VrfProof, VRF_OUTPUT_LENGTH, VRF_PROOF_LENGTH,
    };

    let mut headers: Vec<BlockHeader> = Vec::new();

    for i in 0..length {
        let hash_bytes = [(i as u8); 32];

        // VrfProof needs 80 bytes
        let mut vrf_proof_bytes = [0u8; VRF_PROOF_LENGTH];
        vrf_proof_bytes[0] = i as u8 + 100;
        vrf_proof_bytes[VRF_PROOF_LENGTH - 1] = i as u8 + 101;

        // VrfOutput needs 64 bytes
        let mut vrf_output_bytes = [0u8; VRF_OUTPUT_LENGTH];
        vrf_output_bytes[0] = i as u8 + 150;
        vrf_output_bytes[VRF_OUTPUT_LENGTH - 1] = i as u8 + 151;

        // Create 20-byte array for Ed25519KeyHash
        let mut key_bytes = [0u8; 20];
        key_bytes[0] = i as u8 + 50;
        key_bytes[19] = i as u8 + 51;

        let mut hot_key_bytes = [0u8; 20];
        hot_key_bytes[0] = i as u8 + 75;
        hot_key_bytes[19] = i as u8 + 76;

        headers.push(BlockHeader {
            slot: SlotNo(i as u64),
            prev_hash: if i == 0 {
                Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap()
            } else {
                headers[i - 1].block_body_hash
            },
            issuer_vkey: Ed25519KeyHash::from_bytes(key_bytes),
            vrf_proof: VrfProof::from_bytes(vrf_proof_bytes).unwrap(),
            vrf_output: VrfOutput::from_bytes(vrf_output_bytes).unwrap(),
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
pub fn validate_message(
    message: &ChainSyncMessage,
    config: &ChainSyncConfig,
) -> Result<(), ChainSyncError> {
    match message {
        ChainSyncMessage::FindIntersect { points } => {
            if points.len() > config.max_intersection_points {
                return Err(ChainSyncError::InvalidPoints(format!(
                    "Too many intersection points: {} > {}",
                    points.len(),
                    config.max_intersection_points
                )));
            }

            // Validate points are in descending order
            for window in points.windows(2) {
                if window[0].slot.0 < window[1].slot.0 {
                    return Err(ChainSyncError::InvalidPoints(
                        "Points must be in descending slot order".to_string(),
                    ));
                }
            }
        }

        ChainSyncMessage::RollForward { header, tip } => {
            if header.slot.0 > tip.slot.0 {
                return Err(ChainSyncError::InvalidRollForward(
                    "Header slot cannot exceed tip slot".to_string(),
                ));
            }
        }

        _ => {} // Other messages don't need validation
    }

    Ok(())
}

struct ServerContext {
    server: Arc<ChainSyncServer>,
    outbound_rx: AsyncMutex<mpsc::UnboundedReceiver<ChainSyncMessage>>,
}

impl ServerContext {
    fn new(chain: &[BlockHeader]) -> Self {
        let (server, outbound_rx, _inbound_tx) = ChainSyncServer::new(chain.to_vec());
        Self {
            server: Arc::new(server),
            outbound_rx: AsyncMutex::new(outbound_rx),
        }
    }
}

/// Protocol handler bridging the ChainSync server with the network multiplexer
#[derive(Clone)]
pub struct ChainSyncProtocolHandler {
    protocol_id: ProtocolId,
    name: &'static str,
    chain: Arc<Vec<BlockHeader>>,
    contexts: Arc<AsyncMutex<HashMap<ConnectionId, Arc<ServerContext>>>>,
}

impl ChainSyncProtocolHandler {
    /// Create a new handler using the provided chain of block headers
    pub fn new(chain: Vec<BlockHeader>) -> Self {
        Self {
            protocol_id: ProtocolId::CHAINSYNC,
            name: "ChainSync",
            chain: Arc::new(chain),
            contexts: Arc::new(AsyncMutex::new(HashMap::new())),
        }
    }

    /// Create a handler backed by a mock chain of the given length (useful for testing)
    pub fn with_mock_chain(length: usize) -> Self {
        Self::new(create_mock_chain(length))
    }

    async fn get_context(&self, connection_id: ConnectionId) -> Arc<ServerContext> {
        let mut contexts = self.contexts.lock().await;
        if let Some(ctx) = contexts.get(&connection_id) {
            return ctx.clone();
        }

        let context = Arc::new(ServerContext::new(self.chain.as_ref()));
        contexts.insert(connection_id, context.clone());
        context
    }

    fn decode_message(&self, payload: &Bytes) -> Result<ChainSyncMessage, ChainSyncError> {
        let wire: ChainSyncWireMessage = minicbor::decode(payload)
            .map_err(|err| ChainSyncError::SerializationError(err.to_string()))?;
        wire.into_domain()
    }

    fn encode_message(&self, message: ChainSyncMessage) -> Result<Bytes, ChainSyncError> {
        let wire = ChainSyncWireMessage::from_domain(message)?;
        let encoded = minicbor::to_vec(&wire)
            .map_err(|err| ChainSyncError::SerializationError(err.to_string()))?;
        Ok(Bytes::from(encoded))
    }
}

impl ProtocolHandler for ChainSyncProtocolHandler {
    fn handle_message(
        &self,
        connection_id: ConnectionId,
        message: Bytes,
    ) -> Pin<Box<dyn std::future::Future<Output = NetworkResult<Option<Bytes>>> + Send>> {
        let handler = self.clone();
        Box::pin(async move { handler.process_message(connection_id, message).await })
    }

    fn protocol_id(&self) -> ProtocolId {
        self.protocol_id
    }

    fn name(&self) -> &str {
        self.name
    }
}
impl ChainSyncProtocolHandler {
    async fn process_message(
        &self,
        connection_id: ConnectionId,
        payload: Bytes,
    ) -> NetworkResult<Option<Bytes>> {
        let message = self
            .decode_message(&payload)
            .map_err(|err| NetworkError::ProtocolError(err.to_string()))?;

        let context = self.get_context(connection_id).await;

        context
            .server
            .handle_message(message)
            .await
            .map_err(|err| NetworkError::ProtocolError(err.to_string()))?;

        let mut outbound = context.outbound_rx.lock().await;
        if let Some(response) = outbound.recv().await {
            let bytes = self
                .encode_message(response)
                .map_err(|err| NetworkError::ProtocolError(err.to_string()))?;
            Ok(Some(bytes))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::ConnectionId;

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

    #[tokio::test]
    async fn test_wire_roundtrip_conversion() {
        let chain = create_mock_chain(3);
        let header = chain.last().cloned().unwrap();
        let tip = Tip::from_header(&header, chain.len() as u64);

        let wire = ChainSyncWireMessage::from_domain(ChainSyncMessage::RollForward {
            header: Box::new(header.clone()),
            tip: tip.clone(),
        })
        .expect("wire encoding should succeed");

        match wire.into_domain().expect("wire decoding should succeed") {
            ChainSyncMessage::RollForward {
                header: decoded_header,
                tip: decoded_tip,
            } => {
                assert_eq!(decoded_header.slot, header.slot);
                assert_eq!(decoded_header.block_body_hash, header.block_body_hash);
                assert_eq!(decoded_tip.slot, tip.slot);
                assert_eq!(decoded_tip.height, tip.height);
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_handler_processes_request_next() {
        let handler = ChainSyncProtocolHandler::with_mock_chain(4);
        let connection_id = ConnectionId::new();

        let payload = Bytes::from(
            minicbor::to_vec(&ChainSyncWireMessage::RequestNext)
                .expect("serialization should succeed"),
        );

        let response = handler
            .handle_message(connection_id, payload)
            .await
            .expect("handler should succeed")
            .expect("response should be present");

        let message: ChainSyncWireMessage =
            minicbor::decode(&response).expect("response should decode");

        match message {
            ChainSyncWireMessage::RollForward { .. } => {}
            other => panic!("unexpected handler response: {:?}", other),
        }
    }
}
