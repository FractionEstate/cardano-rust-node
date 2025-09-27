//! BlockFetch Protocol Tests
//!
//! Tests for the BlockFetch mini-protocol which handles efficient retrieval
//! of block content between Cardano nodes. This protocol allows nodes to:
//! - Request blocks within a specific range
//! - Stream blocks in batches for efficiency
//! - Handle block validation and integrity checking
//! - Manage bandwidth and flow control
//!
//! Based on the Cardano Network Protocol Specification.

use cardano_network::protocols::blockfetch::*;
use cardano_consensus::block_production::{BlockHeader, BlockBody, ForgedBlock};
use cardano_crypto::Blake2b256Hash;
use std::collections::VecDeque;

/// Test data structures for BlockFetch protocol

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Point {
    pub slot: u64,
    pub hash: Blake2b256Hash,
}

impl Point {
    pub fn new(slot: u64, hash_bytes: &[u8; 32]) -> Self {
        Self {
            slot,
            hash: Blake2b256Hash::from_bytes(hash_bytes).unwrap(),
        }
    }

    pub fn genesis() -> Self {
        Self::new(0, &[0u8; 32])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChainRange {
    pub from: Point,
    pub to: Point,
}

impl ChainRange {
    pub fn new(from: Point, to: Point) -> Result<Self, BlockFetchError> {
        if from.slot > to.slot {
            return Err(BlockFetchError::InvalidRange {
                from: from.slot,
                to: to.slot,
            });
        }
        Ok(Self { from, to })
    }

    pub fn is_valid(&self) -> bool {
        self.from.slot <= self.to.slot
    }

    pub fn contains(&self, point: &Point) -> bool {
        point.slot >= self.from.slot && point.slot <= self.to.slot
    }

    pub fn size(&self) -> u64 {
        if self.to.slot >= self.from.slot {
            self.to.slot - self.from.slot + 1
        } else {
            0
        }
    }
}

/// BlockFetch protocol messages
#[derive(Debug, Clone, PartialEq)]
pub enum BlockFetchMessage {
    // Client -> Server messages
    RequestRange {
        range: ChainRange,
    },
    ClientDone,

    // Server -> Client messages
    StartBatch,
    Block {
        body: BlockBody,
    },
    BatchDone,
    NoBlocks,
}

/// BlockFetch protocol states
#[derive(Debug, Clone, PartialEq)]
pub enum BlockFetchState {
    Idle,
    WaitingForBatch,
    ReceivingBlocks,
    BatchComplete,
}

/// Mock BlockFetch client for testing
#[derive(Debug)]
pub struct MockBlockFetchClient {
    state: BlockFetchState,
    current_range: Option<ChainRange>,
    received_blocks: Vec<BlockBody>,
    message_queue: VecDeque<BlockFetchMessage>,
}

impl MockBlockFetchClient {
    pub fn new() -> Self {
        Self {
            state: BlockFetchState::Idle,
            current_range: None,
            received_blocks: Vec::new(),
            message_queue: VecDeque::new(),
        }
    }

    /// Send a range request to the server
    pub fn request_range(&mut self, range: ChainRange) -> Result<(), BlockFetchError> {
        match self.state {
            BlockFetchState::Idle => {
                range.is_valid().then(|| ()).ok_or_else(|| {
                    BlockFetchError::InvalidRange {
                        from: range.from.slot,
                        to: range.to.slot,
                    }
                })?;

                self.state = BlockFetchState::WaitingForBatch;
                self.current_range = Some(range.clone());
                self.message_queue.push_back(BlockFetchMessage::RequestRange { range });
                Ok(())
            }
            _ => Err(BlockFetchError::InvalidState {
                current_state: self.state.clone(),
                message: "Can only request range from Idle state".to_string(),
            }),
        }
    }

    /// Signal client is done with requests
    pub fn client_done(&mut self) -> Result<(), BlockFetchError> {
        if self.state == BlockFetchState::Idle {
            self.message_queue.push_back(BlockFetchMessage::ClientDone);
            Ok(())
        } else {
            Err(BlockFetchError::InvalidState {
                current_state: self.state.clone(),
                message: "Can only send ClientDone from Idle state".to_string(),
            })
        }
    }

    /// Receive a message from the server
    pub fn receive_message(&mut self, message: BlockFetchMessage) -> Result<(), BlockFetchError> {
        match (&self.state, &message) {
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
                self.validate_block_in_range(body)?;
                self.received_blocks.push(body.clone());
                Ok(())
            }
            (BlockFetchState::ReceivingBlocks, BlockFetchMessage::BatchDone) => {
                self.validate_batch_completeness()?;
                self.state = BlockFetchState::BatchComplete;
                Ok(())
            }
            _ => Err(BlockFetchError::InvalidState {
                current_state: self.state.clone(),
                message: format!("Unexpected message: {:?}", message),
            }),
        }
    }

    fn validate_block_in_range(&self, body: &BlockBody) -> Result<(), BlockFetchError> {
        if let Some(range) = &self.current_range {
            // In a real implementation, we'd extract the slot from the block body
            // For testing, we'll use a simplified validation
            if self.received_blocks.len() as u64 >= range.size() {
                return Err(BlockFetchError::TooManyBlocks {
                    expected: range.size(),
                    received: self.received_blocks.len() as u64 + 1,
                });
            }
        }
        Ok(())
    }

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

    /// Reset state for next batch
    pub fn reset_for_next_request(&mut self) {
        if self.state == BlockFetchState::BatchComplete {
            self.state = BlockFetchState::Idle;
            self.current_range = None;
            self.received_blocks.clear();
        }
    }

    // Getters for testing
    pub fn get_state(&self) -> &BlockFetchState {
        &self.state
    }

    pub fn get_received_blocks(&self) -> &[BlockBody] {
        &self.received_blocks
    }

    pub fn get_current_range(&self) -> Option<&ChainRange> {
        self.current_range.as_ref()
    }
}

/// Mock BlockFetch server for testing
#[derive(Debug)]
pub struct MockBlockFetchServer {
    available_blocks: std::collections::HashMap<u64, BlockBody>, // slot -> block
    current_request: Option<ChainRange>,
    blocks_to_send: VecDeque<BlockBody>,
}

impl MockBlockFetchServer {
    pub fn new() -> Self {
        Self {
            available_blocks: std::collections::HashMap::new(),
            current_request: None,
            blocks_to_send: VecDeque::new(),
        }
    }

    pub fn add_block(&mut self, slot: u64, body: BlockBody) {
        self.available_blocks.insert(slot, body);
    }

    /// Handle a range request from client
    pub fn handle_request(&mut self, range: ChainRange) -> BlockFetchMessage {
        self.current_request = Some(range.clone());

        // Check if we have blocks in this range
        let mut found_blocks = Vec::new();
        for slot in range.from.slot..=range.to.slot {
            if let Some(body) = self.available_blocks.get(&slot) {
                found_blocks.push(body.clone());
            }
        }

        if found_blocks.is_empty() {
            self.current_request = None;
            BlockFetchMessage::NoBlocks
        } else {
            // Prepare blocks for streaming
            self.blocks_to_send = found_blocks.into();
            BlockFetchMessage::StartBatch
        }
    }

    /// Get next block in the stream
    pub fn next_block(&mut self) -> Option<BlockFetchMessage> {
        if let Some(body) = self.blocks_to_send.pop_front() {
            Some(BlockFetchMessage::Block { body })
        } else if self.current_request.is_some() {
            // All blocks sent, finish batch
            self.current_request = None;
            Some(BlockFetchMessage::BatchDone)
        } else {
            None
        }
    }

    pub fn has_blocks_to_send(&self) -> bool {
        !self.blocks_to_send.is_empty()
    }

    pub fn is_batch_active(&self) -> bool {
        self.current_request.is_some()
    }
}

/// BlockFetch protocol errors
#[derive(Debug, thiserror::Error)]
pub enum BlockFetchError {
    #[error("Invalid range: from slot {from} to slot {to}")]
    InvalidRange { from: u64, to: u64 },

    #[error("Invalid state transition: current state {current_state:?}, message: {message}")]
    InvalidState {
        current_state: BlockFetchState,
        message: String,
    },

    #[error("Too many blocks received: expected {expected}, received {received}")]
    TooManyBlocks { expected: u64, received: u64 },

    #[error("Incomplete range: expected {expected} blocks, received {received}")]
    IncompleteRange { expected: u64, received: u64 },

    #[error("Block validation failed: {0}")]
    BlockValidation(String),

    #[error("Server error: {0}")]
    ServerError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use cardano_consensus::block_production::Transaction;

    /// Helper to create test block bodies
    fn create_test_block_body(transactions: Vec<Transaction>) -> BlockBody {
        BlockBody {
            transactions,
            certificates: Vec::new(),
            withdrawals: std::collections::HashMap::new(),
            auxiliary_data: None,
        }
    }

    fn create_empty_block_body() -> BlockBody {
        create_test_block_body(Vec::new())
    }

    #[test]
    fn test_blockfetch_state_machine_idle_to_waiting() {
        let mut client = MockBlockFetchClient::new();

        // Should start in Idle state
        assert_eq!(client.get_state(), &BlockFetchState::Idle);

        // Can request range from Idle
        let range = ChainRange::new(
            Point::new(1, &[1u8; 32]),
            Point::new(5, &[5u8; 32])
        ).unwrap();

        assert!(client.request_range(range).is_ok());
        assert_eq!(client.get_state(), &BlockFetchState::WaitingForBatch);
    }

    #[test]
    fn test_blockfetch_invalid_range_request() {
        let mut client = MockBlockFetchClient::new();

        // Range with from > to should fail
        let invalid_range = ChainRange {
            from: Point::new(10, &[10u8; 32]),
            to: Point::new(5, &[5u8; 32]),
        };

        assert!(client.request_range(invalid_range).is_err());
        assert_eq!(client.get_state(), &BlockFetchState::Idle);
    }

    #[test]
    fn test_blockfetch_successful_batch_flow() {
        let mut client = MockBlockFetchClient::new();

        // Request a range
        let range = ChainRange::new(
            Point::new(1, &[1u8; 32]),
            Point::new(3, &[3u8; 32])
        ).unwrap();

        client.request_range(range).unwrap();
        assert_eq!(client.get_state(), &BlockFetchState::WaitingForBatch);

        // Receive StartBatch
        client.receive_message(BlockFetchMessage::StartBatch).unwrap();
        assert_eq!(client.get_state(), &BlockFetchState::ReceivingBlocks);

        // Receive blocks
        for i in 1..=3 {
            let body = create_empty_block_body();
            client.receive_message(BlockFetchMessage::Block { body }).unwrap();
        }

        // Receive BatchDone
        client.receive_message(BlockFetchMessage::BatchDone).unwrap();
        assert_eq!(client.get_state(), &BlockFetchState::BatchComplete);

        // Should have received 3 blocks
        assert_eq!(client.get_received_blocks().len(), 3);
    }

    #[test]
    fn test_blockfetch_no_blocks_available() {
        let mut client = MockBlockFetchClient::new();

        // Request a range
        let range = ChainRange::new(
            Point::new(100, &[100u8; 32]),
            Point::new(105, &[105u8; 32])
        ).unwrap();

        client.request_range(range).unwrap();

        // Receive NoBlocks (server has no blocks in range)
        client.receive_message(BlockFetchMessage::NoBlocks).unwrap();
        assert_eq!(client.get_state(), &BlockFetchState::Idle);
        assert_eq!(client.get_received_blocks().len(), 0);
    }

    #[test]
    fn test_blockfetch_invalid_state_transitions() {
        let mut client = MockBlockFetchClient::new();

        // Can't receive StartBatch without requesting range
        assert!(client.receive_message(BlockFetchMessage::StartBatch).is_err());

        // Can't receive Block without StartBatch
        let body = create_empty_block_body();
        assert!(client.receive_message(BlockFetchMessage::Block { body }).is_err());

        // Can't send multiple requests simultaneously
        let range = ChainRange::new(
            Point::new(1, &[1u8; 32]),
            Point::new(3, &[3u8; 32])
        ).unwrap();

        client.request_range(range.clone()).unwrap();
        assert!(client.request_range(range).is_err());
    }

    #[test]
    fn test_blockfetch_server_mock() {
        let mut server = MockBlockFetchServer::new();

        // Add some test blocks
        for i in 1..=5 {
            let body = create_empty_block_body();
            server.add_block(i, body);
        }

        // Request range 2-4
        let range = ChainRange::new(
            Point::new(2, &[2u8; 32]),
            Point::new(4, &[4u8; 32])
        ).unwrap();

        // Server should start batch
        let response = server.handle_request(range);
        assert_eq!(response, BlockFetchMessage::StartBatch);
        assert!(server.is_batch_active());

        // Server should provide blocks 2, 3, 4
        let mut blocks_received = 0;
        while let Some(message) = server.next_block() {
            match message {
                BlockFetchMessage::Block { .. } => {
                    blocks_received += 1;
                }
                BlockFetchMessage::BatchDone => {
                    break;
                }
                _ => panic!("Unexpected message from server"),
            }
        }

        assert_eq!(blocks_received, 3);
        assert!(!server.is_batch_active());
    }

    #[test]
    fn test_blockfetch_server_no_blocks_in_range() {
        let mut server = MockBlockFetchServer::new();

        // Add blocks 1-5 but request range 10-15
        for i in 1..=5 {
            let body = create_empty_block_body();
            server.add_block(i, body);
        }

        let range = ChainRange::new(
            Point::new(10, &[10u8; 32]),
            Point::new(15, &[15u8; 32])
        ).unwrap();

        // Server should respond with NoBlocks
        let response = server.handle_request(range);
        assert_eq!(response, BlockFetchMessage::NoBlocks);
        assert!(!server.is_batch_active());
    }

    #[test]
    fn test_blockfetch_partial_range_coverage() {
        let mut server = MockBlockFetchServer::new();

        // Add blocks at slots 1, 3, 5 (gaps at 2, 4)
        for i in &[1, 3, 5] {
            let body = create_empty_block_body();
            server.add_block(*i, body);
        }

        // Request range 1-5
        let range = ChainRange::new(
            Point::new(1, &[1u8; 32]),
            Point::new(5, &[5u8; 32])
        ).unwrap();

        let response = server.handle_request(range);
        assert_eq!(response, BlockFetchMessage::StartBatch);

        // Should only get blocks 1, 3, 5 (not 2, 4)
        let mut blocks_received = 0;
        while let Some(message) = server.next_block() {
            match message {
                BlockFetchMessage::Block { .. } => {
                    blocks_received += 1;
                }
                BlockFetchMessage::BatchDone => {
                    break;
                }
                _ => panic!("Unexpected message from server"),
            }
        }

        assert_eq!(blocks_received, 3); // Only available blocks
    }

    #[test]
    fn test_blockfetch_range_utilities() {
        // Test ChainRange utility methods
        let range = ChainRange::new(
            Point::new(10, &[10u8; 32]),
            Point::new(20, &[20u8; 32])
        ).unwrap();

        assert!(range.is_valid());
        assert_eq!(range.size(), 11); // Inclusive range

        // Test containment
        assert!(range.contains(&Point::new(10, &[10u8; 32])));
        assert!(range.contains(&Point::new(15, &[15u8; 32])));
        assert!(range.contains(&Point::new(20, &[20u8; 32])));
        assert!(!range.contains(&Point::new(9, &[9u8; 32])));
        assert!(!range.contains(&Point::new(21, &[21u8; 32])));

        // Test invalid range
        assert!(ChainRange::new(
            Point::new(20, &[20u8; 32]),
            Point::new(10, &[10u8; 32])
        ).is_err());
    }

    #[test]
    fn test_blockfetch_client_done_signal() {
        let mut client = MockBlockFetchClient::new();

        // Should be able to send ClientDone from Idle
        assert!(client.client_done().is_ok());

        // Start a request
        let range = ChainRange::new(
            Point::new(1, &[1u8; 32]),
            Point::new(3, &[3u8; 32])
        ).unwrap();

        client.request_range(range).unwrap();

        // Should not be able to send ClientDone while request is active
        assert!(client.client_done().is_err());
    }

    #[test]
    fn test_blockfetch_reset_between_requests() {
        let mut client = MockBlockFetchClient::new();

        // Complete first request
        let range1 = ChainRange::new(
            Point::new(1, &[1u8; 32]),
            Point::new(2, &[2u8; 32])
        ).unwrap();

        client.request_range(range1).unwrap();
        client.receive_message(BlockFetchMessage::StartBatch).unwrap();

        for _ in 1..=2 {
            let body = create_empty_block_body();
            client.receive_message(BlockFetchMessage::Block { body }).unwrap();
        }

        client.receive_message(BlockFetchMessage::BatchDone).unwrap();
        assert_eq!(client.get_state(), &BlockFetchState::BatchComplete);

        // Reset for next request
        client.reset_for_next_request();
        assert_eq!(client.get_state(), &BlockFetchState::Idle);
        assert_eq!(client.get_received_blocks().len(), 0);

        // Should be able to make another request
        let range2 = ChainRange::new(
            Point::new(5, &[5u8; 32]),
            Point::new(7, &[7u8; 32])
        ).unwrap();

        assert!(client.request_range(range2).is_ok());
    }

    #[test]
    fn test_blockfetch_error_handling() {
        // Test various error conditions
        let mut client = MockBlockFetchClient::new();

        // Invalid range error
        let invalid_range = ChainRange {
            from: Point::new(10, &[10u8; 32]),
            to: Point::new(5, &[5u8; 32]),
        };

        match client.request_range(invalid_range) {
            Err(BlockFetchError::InvalidRange { from: 10, to: 5 }) => {},
            _ => panic!("Expected InvalidRange error"),
        }

        // Invalid state error
        let valid_range = ChainRange::new(
            Point::new(1, &[1u8; 32]),
            Point::new(3, &[3u8; 32])
        ).unwrap();

        client.request_range(valid_range).unwrap();

        match client.request_range(ChainRange::new(
            Point::new(5, &[5u8; 32]),
            Point::new(7, &[7u8; 32])
        ).unwrap()) {
            Err(BlockFetchError::InvalidState { .. }) => {},
            _ => panic!("Expected InvalidState error"),
        }
    }

    #[test]
    fn test_blockfetch_large_range_streaming() {
        // Test handling of large block ranges
        let mut server = MockBlockFetchServer::new();
        let mut client = MockBlockFetchClient::new();

        // Add 1000 blocks
        for i in 1..=1000 {
            let body = create_empty_block_body();
            server.add_block(i, body);
        }

        // Request large range
        let range = ChainRange::new(
            Point::new(1, &[1u8; 32]),
            Point::new(1000, &[255u8; 32])
        ).unwrap();

        assert_eq!(range.size(), 1000);

        client.request_range(range.clone()).unwrap();

        // Server starts batch
        let response = server.handle_request(range);
        assert_eq!(response, BlockFetchMessage::StartBatch);
        client.receive_message(response).unwrap();

        // Stream all blocks
        let mut blocks_streamed = 0;
        while let Some(message) = server.next_block() {
            client.receive_message(message).unwrap();
            if client.get_state() == &BlockFetchState::BatchComplete {
                blocks_streamed = client.get_received_blocks().len();
                break;
            }
        }

        assert_eq!(blocks_streamed, 1000);
    }

    #[test]
    fn test_blockfetch_protocol_properties() {
        // Test important protocol properties and invariants

        // Property 1: Range validation is consistent
        let valid_ranges = vec![
            (1, 1),   // Single block
            (1, 100), // Large range
            (50, 50), // Single block at higher slot
        ];

        for (from, to) in valid_ranges {
            let range = ChainRange::new(
                Point::new(from, &[from as u8; 32]),
                Point::new(to, &[to as u8; 32])
            );
            assert!(range.is_ok(), "Range {}-{} should be valid", from, to);
            let range = range.unwrap();
            assert!(range.is_valid());
            assert_eq!(range.size(), to - from + 1);
        }

        // Property 2: State machine is deterministic
        let mut client = MockBlockFetchClient::new();
        assert_eq!(client.get_state(), &BlockFetchState::Idle);

        let range = ChainRange::new(
            Point::new(1, &[1u8; 32]),
            Point::new(1, &[1u8; 32])
        ).unwrap();

        client.request_range(range).unwrap();
        assert_eq!(client.get_state(), &BlockFetchState::WaitingForBatch);

        // Property 3: Message ordering is enforced
        let body = create_empty_block_body();

        // Can't receive Block before StartBatch
        assert!(client.receive_message(BlockFetchMessage::Block { body: body.clone() }).is_err());

        // Must receive StartBatch first
        client.receive_message(BlockFetchMessage::StartBatch).unwrap();
        assert_eq!(client.get_state(), &BlockFetchState::ReceivingBlocks);

        // Now can receive blocks
        assert!(client.receive_message(BlockFetchMessage::Block { body }).is_ok());
    }

    #[test]
    fn test_blockfetch_concurrent_server_simulation() {
        // Simulate server handling multiple overlapping ranges
        // (In practice, servers may serve multiple clients concurrently)

        let mut server = MockBlockFetchServer::new();

        // Add blocks 1-20
        for i in 1..=20 {
            let body = create_empty_block_body();
            server.add_block(i, body);
        }

        // Simulate first client requesting 1-10
        let range1 = ChainRange::new(
            Point::new(1, &[1u8; 32]),
            Point::new(10, &[10u8; 32])
        ).unwrap();

        let response1 = server.handle_request(range1);
        assert_eq!(response1, BlockFetchMessage::StartBatch);

        // Stream blocks for first client
        let mut blocks_for_client1 = 0;
        while server.has_blocks_to_send() {
            if let Some(BlockFetchMessage::Block { .. }) = server.next_block() {
                blocks_for_client1 += 1;
            }
        }

        // Finish first batch
        if let Some(BlockFetchMessage::BatchDone) = server.next_block() {
            // Batch completed
        }

        assert_eq!(blocks_for_client1, 10);

        // Now server can handle second client requesting 15-20
        let range2 = ChainRange::new(
            Point::new(15, &[15u8; 32]),
            Point::new(20, &[20u8; 32])
        ).unwrap();

        let response2 = server.handle_request(range2);
        assert_eq!(response2, BlockFetchMessage::StartBatch);

        let mut blocks_for_client2 = 0;
        while let Some(message) = server.next_block() {
            if let BlockFetchMessage::Block { .. } = message {
                blocks_for_client2 += 1;
            } else if let BlockFetchMessage::BatchDone = message {
                break;
            }
        }

        assert_eq!(blocks_for_client2, 6); // Slots 15-20
    }
}
