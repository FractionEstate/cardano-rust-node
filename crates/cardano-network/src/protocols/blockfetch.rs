//! BlockFetch Protocol Implementation
//!
//! The BlockFetch mini-protocol handles efficient retrieval of block content between Cardano nodes.
//! It allows nodes to request blocks within a specific range, stream blocks in batches for efficiency,
//! and handle bandwidth and flow control.
//!
//! # Protocol Overview
//!
//! The BlockFetch protocol operates as a client-server interaction where:
//! - Client requests a range of blocks by specifying start and end points
//! - Server responds with either a batch of blocks or NoBlocks if unavailable
//! - Blocks are streamed in order within the requested range
//!
//! # State Machine
//!
//! ```text
//! Idle -> RequestRange -> StartBatch -> Block* -> BatchDone -> Idle
//!                      |              |
//!                      -> NoBlocks ----> Idle
//! ```
//!
//! Based on the Cardano Network Protocol Specification.

use cardano_consensus::block_production::BlockBody;
use cardano_consensus::ouroboros::SlotNo;
use cardano_crypto::Blake2b256Hash;
use std::collections::HashMap;
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
}

/// A range of blocks to fetch, from one point to another
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainRange {
    pub from: Point,
    pub to: Point,
}

impl ChainRange {
    /// Create a new chain range with validation
    pub fn new(from: Point, to: Point) -> Result<Self, BlockFetchError> {
        if from.slot.0 > to.slot.0 {
            return Err(BlockFetchError::InvalidRange {
                from: from.slot.0,
                to: to.slot.0,
            });
        }
        Ok(Self { from, to })
    }

    /// Check if the range is valid (from <= to)
    pub fn is_valid(&self) -> bool {
        self.from.slot.0 <= self.to.slot.0
    }

    /// Check if a point is contained within this range
    pub fn contains(&self, point: &Point) -> bool {
        point.slot.0 >= self.from.slot.0 && point.slot.0 <= self.to.slot.0
    }

    /// Calculate the number of blocks in this range
    pub fn size(&self) -> u64 {
        if self.to.slot.0 >= self.from.slot.0 {
            self.to.slot.0 - self.from.slot.0 + 1
        } else {
            0
        }
    }
}

/// BlockFetch protocol messages
#[derive(Debug, Clone)]
pub enum BlockFetchMessage {
    /// Client requests a range of blocks
    RequestRange { range: ChainRange },
    /// Client signals it's done with requests
    ClientDone,
    /// Server signals start of a batch
    StartBatch,
    /// Server sends a block in the requested range
    Block { body: BlockBody },
    /// Server signals end of the current batch
    BatchDone,
    /// Server indicates no blocks are available for the requested range
    NoBlocks,
}

impl PartialEq for BlockFetchMessage {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                BlockFetchMessage::RequestRange { range: r1 },
                BlockFetchMessage::RequestRange { range: r2 },
            ) => r1 == r2,
            (BlockFetchMessage::ClientDone, BlockFetchMessage::ClientDone) => true,
            (BlockFetchMessage::StartBatch, BlockFetchMessage::StartBatch) => true,
            (BlockFetchMessage::Block { body: b1 }, BlockFetchMessage::Block { body: b2 }) => {
                // Compare block bodies by their transaction count and fees
                b1.transactions.len() == b2.transactions.len()
                    && b1.total_fee == b2.total_fee
                    && b1.total_size == b2.total_size
            }
            (BlockFetchMessage::BatchDone, BlockFetchMessage::BatchDone) => true,
            (BlockFetchMessage::NoBlocks, BlockFetchMessage::NoBlocks) => true,
            _ => false,
        }
    }
}

/// BlockFetch protocol states
#[derive(Debug, Clone, PartialEq)]
pub enum BlockFetchState {
    /// Idle state, ready to accept new requests
    Idle,
    /// Waiting for server to start sending batch
    WaitingForBatch,
    /// Currently receiving blocks from server
    ReceivingBlocks,
    /// Batch completed successfully
    BatchComplete,
}

/// BlockFetch protocol errors
#[derive(Debug, Clone, PartialEq)]
pub enum BlockFetchError {
    /// Invalid range (from > to)
    InvalidRange { from: u64, to: u64 },
    /// Invalid state transition
    InvalidState {
        current_state: BlockFetchState,
        message: String,
    },
    /// Requested block not found
    BlockNotFound { point: Point },
    /// Too many blocks received for range
    TooManyBlocks { expected: u64, received: u64 },
    /// Incomplete range (missing blocks)
    IncompleteRange { expected: u64, received: u64 },
    /// Protocol violation
    ProtocolViolation { message: String },
    /// Network error
    NetworkError { message: String },
}

impl std::fmt::Display for BlockFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockFetchError::InvalidRange { from, to } => {
                write!(f, "Invalid range: from slot {} > to slot {}", from, to)
            }
            BlockFetchError::InvalidState {
                current_state,
                message,
            } => {
                write!(f, "Invalid state {:?}: {}", current_state, message)
            }
            BlockFetchError::BlockNotFound { point } => {
                write!(f, "Block not found at slot {}", point.slot.0)
            }
            BlockFetchError::TooManyBlocks { expected, received } => {
                write!(
                    f,
                    "Too many blocks: expected {}, received {}",
                    expected, received
                )
            }
            BlockFetchError::IncompleteRange { expected, received } => {
                write!(
                    f,
                    "Incomplete range: expected {} blocks, received {}",
                    expected, received
                )
            }
            BlockFetchError::ProtocolViolation { message } => {
                write!(f, "Protocol violation: {}", message)
            }
            BlockFetchError::NetworkError { message } => {
                write!(f, "Network error: {}", message)
            }
        }
    }
}

impl std::error::Error for BlockFetchError {}

/// BlockFetch client implementation
#[derive(Debug)]
pub struct BlockFetchClient {
    state: BlockFetchState,
    current_range: Option<ChainRange>,
    received_blocks: Vec<BlockBody>,
    sender: mpsc::UnboundedSender<BlockFetchMessage>,
}

impl BlockFetchClient {
    /// Create a new BlockFetch client
    pub fn new() -> (Self, mpsc::UnboundedReceiver<BlockFetchMessage>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let client = Self {
            state: BlockFetchState::Idle,
            current_range: None,
            received_blocks: Vec::new(),
            sender,
        };

        (client, receiver)
    }

    /// Request a range of blocks from the server
    pub async fn request_range(&mut self, range: ChainRange) -> Result<(), BlockFetchError> {
        match self.state {
            BlockFetchState::Idle => {
                // Validate the range
                if !range.is_valid() {
                    return Err(BlockFetchError::InvalidRange {
                        from: range.from.slot.0,
                        to: range.to.slot.0,
                    });
                }

                self.state = BlockFetchState::WaitingForBatch;
                self.current_range = Some(range.clone());
                self.received_blocks.clear();

                // Send the request
                self.sender
                    .send(BlockFetchMessage::RequestRange { range })
                    .map_err(|_| BlockFetchError::NetworkError {
                        message: "Failed to send request".to_string(),
                    })?;

                Ok(())
            }
            _ => Err(BlockFetchError::InvalidState {
                current_state: self.state.clone(),
                message: "Can only request range from Idle state".to_string(),
            }),
        }
    }

    /// Signal that the client is done with requests
    pub async fn client_done(&mut self) -> Result<(), BlockFetchError> {
        if self.state == BlockFetchState::Idle {
            self.sender
                .send(BlockFetchMessage::ClientDone)
                .map_err(|_| BlockFetchError::NetworkError {
                    message: "Failed to send ClientDone".to_string(),
                })?;
            Ok(())
        } else {
            Err(BlockFetchError::InvalidState {
                current_state: self.state.clone(),
                message: "Can only send ClientDone from Idle state".to_string(),
            })
        }
    }

    /// Process a message received from the server
    pub async fn receive_message(
        &mut self,
        message: BlockFetchMessage,
    ) -> Result<(), BlockFetchError> {
        match (&self.state, message) {
            (BlockFetchState::WaitingForBatch, BlockFetchMessage::StartBatch) => {
                self.state = BlockFetchState::ReceivingBlocks;
                Ok(())
            }
            (BlockFetchState::WaitingForBatch, BlockFetchMessage::NoBlocks) => {
                self.state = BlockFetchState::Idle;
                self.current_range = None;
                Ok(())
            }
            (BlockFetchState::ReceivingBlocks, BlockFetchMessage::Block { body }) => {
                self.validate_block_in_range(&body)?;
                self.received_blocks.push(body);
                Ok(())
            }
            (BlockFetchState::ReceivingBlocks, BlockFetchMessage::BatchDone) => {
                self.validate_batch_completeness()?;
                self.state = BlockFetchState::BatchComplete;
                Ok(())
            }
            _ => Err(BlockFetchError::InvalidState {
                current_state: self.state.clone(),
                message: "Unexpected message".to_string(),
            }),
        }
    }

    /// Validate that a received block is within the requested range
    fn validate_block_in_range(&self, body: &BlockBody) -> Result<(), BlockFetchError> {
        if let Some(range) = &self.current_range {
            // Check if we've received too many blocks
            if self.received_blocks.len() as u64 >= range.size() {
                return Err(BlockFetchError::TooManyBlocks {
                    expected: range.size(),
                    received: self.received_blocks.len() as u64 + 1,
                });
            }

            if body.total_size == 0 {
                return Err(BlockFetchError::ProtocolViolation {
                    message: "Received block with zero size".to_string(),
                });
            }
        }
        Ok(())
    }

    /// Validate that the batch is complete
    fn validate_batch_completeness(&self) -> Result<(), BlockFetchError> {
        if let Some(range) = &self.current_range {
            let expected_blocks = range.size();
            let received_blocks = self.received_blocks.len() as u64;

            if received_blocks != expected_blocks {
                return Err(BlockFetchError::IncompleteRange {
                    expected: expected_blocks,
                    received: received_blocks,
                });
            }
        }
        Ok(())
    }

    /// Reset the client for the next request
    pub fn reset_for_next_request(&mut self) {
        if self.state == BlockFetchState::BatchComplete {
            self.state = BlockFetchState::Idle;
            self.current_range = None;
            self.received_blocks.clear();
        }
    }

    /// Get the current state
    pub fn get_state(&self) -> &BlockFetchState {
        &self.state
    }

    /// Get the received blocks
    pub fn get_received_blocks(&self) -> &[BlockBody] {
        &self.received_blocks
    }

    /// Get the current range
    pub fn get_current_range(&self) -> Option<&ChainRange> {
        self.current_range.as_ref()
    }
}

/// BlockFetch server implementation
#[derive(Debug)]
pub struct BlockFetchServer {
    available_blocks: HashMap<u64, BlockBody>, // slot -> block
    current_request: Option<ChainRange>,
    sender: mpsc::UnboundedSender<BlockFetchMessage>,
}

impl BlockFetchServer {
    /// Create a new BlockFetch server
    pub fn new() -> (Self, mpsc::UnboundedReceiver<BlockFetchMessage>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let server = Self {
            available_blocks: HashMap::new(),
            current_request: None,
            sender,
        };

        (server, receiver)
    }

    /// Add a block to the server's available blocks
    pub fn add_block(&mut self, slot: u64, block: BlockBody) {
        self.available_blocks.insert(slot, block);
    }

    /// Process a request from the client
    pub async fn handle_request(&mut self, range: ChainRange) -> Result<(), BlockFetchError> {
        // Validate the range
        if !range.is_valid() {
            return Err(BlockFetchError::InvalidRange {
                from: range.from.slot.0,
                to: range.to.slot.0,
            });
        }

        self.current_request = Some(range.clone());

        // Collect blocks in the requested range
        let mut blocks_in_range = Vec::new();
        for slot in range.from.slot.0..=range.to.slot.0 {
            if let Some(block) = self.available_blocks.get(&slot) {
                blocks_in_range.push(block.clone());
            }
        }

        if blocks_in_range.is_empty() {
            // No blocks available, send NoBlocks
            self.sender.send(BlockFetchMessage::NoBlocks).map_err(|_| {
                BlockFetchError::NetworkError {
                    message: "Failed to send NoBlocks".to_string(),
                }
            })?;
        } else {
            // Start the batch
            self.sender
                .send(BlockFetchMessage::StartBatch)
                .map_err(|_| BlockFetchError::NetworkError {
                    message: "Failed to send StartBatch".to_string(),
                })?;

            // Send all blocks
            for block in &blocks_in_range {
                self.sender
                    .send(BlockFetchMessage::Block {
                        body: block.clone(),
                    })
                    .map_err(|_| BlockFetchError::NetworkError {
                        message: "Failed to send Block".to_string(),
                    })?;
            }

            // End the batch
            self.sender
                .send(BlockFetchMessage::BatchDone)
                .map_err(|_| BlockFetchError::NetworkError {
                    message: "Failed to send BatchDone".to_string(),
                })?;
        }

        self.current_request = None;
        Ok(())
    }

    /// Get the current request being processed
    pub fn get_current_request(&self) -> Option<&ChainRange> {
        self.current_request.as_ref()
    }

    /// Get the number of available blocks
    pub fn get_available_blocks_count(&self) -> usize {
        self.available_blocks.len()
    }

    /// Check if a block is available at the given slot
    pub fn has_block_at_slot(&self, slot: u64) -> bool {
        self.available_blocks.contains_key(&slot)
    }
}

/// Mock block generator for testing
pub fn generate_mock_block_body(slot: u64) -> BlockBody {
    // Create a mock block body with minimal data for testing
    // In a real implementation, this would be a proper BlockBody with transactions
    BlockBody {
        transactions: Vec::new(),
        total_fee: slot * 1000, // Mock fee based on slot for uniqueness
        total_size: 1000 + slot as u32, // Mock size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_creation() {
        let genesis = Point::genesis();
        assert_eq!(genesis.slot.0, 0);

        let point = Point::new(42, &[1u8; 32]);
        assert_eq!(point.slot.0, 42);
    }

    #[test]
    fn test_chain_range_validation() {
        let from = Point::new(10, &[1u8; 32]);
        let to = Point::new(20, &[2u8; 32]);

        let range = ChainRange::new(from.clone(), to.clone()).unwrap();
        assert!(range.is_valid());
        assert_eq!(range.size(), 11); // 10 through 20 inclusive

        // Test invalid range
        let invalid_range = ChainRange::new(to, from);
        assert!(invalid_range.is_err());
    }

    #[test]
    fn test_chain_range_contains() {
        let from = Point::new(10, &[1u8; 32]);
        let to = Point::new(20, &[2u8; 32]);
        let range = ChainRange::new(from, to).unwrap();

        let point_in_range = Point::new(15, &[3u8; 32]);
        let point_out_of_range = Point::new(25, &[4u8; 32]);

        assert!(range.contains(&point_in_range));
        assert!(!range.contains(&point_out_of_range));
    }

    #[test]
    fn test_blockfetch_client_creation() {
        let (client, _receiver) = BlockFetchClient::new();
        assert_eq!(client.get_state(), &BlockFetchState::Idle);
        assert!(client.get_current_range().is_none());
        assert!(client.get_received_blocks().is_empty());
    }

    #[test]
    fn test_blockfetch_server_creation() {
        let (server, _receiver) = BlockFetchServer::new();
        assert_eq!(server.get_available_blocks_count(), 0);
        assert!(server.get_current_request().is_none());
    }

    #[tokio::test]
    async fn test_invalid_range_request() {
        let (_client, _receiver) = BlockFetchClient::new();

        let from = Point::new(20, &[1u8; 32]);
        let to = Point::new(10, &[2u8; 32]);
        let invalid_range = ChainRange::new(from, to).unwrap_err();

        match invalid_range {
            BlockFetchError::InvalidRange { from: 20, to: 10 } => {}
            _ => panic!("Expected InvalidRange error"),
        }
    }

    #[test]
    fn test_mock_block_generation() {
        let block = generate_mock_block_body(42);
        assert_eq!(block.transactions.len(), 0);
        assert_eq!(block.total_fee, 42000); // slot * 1000
        assert_eq!(block.total_size, 1042); // 1000 + slot
    }
}
