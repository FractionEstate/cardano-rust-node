//! ChainSync Protocol Tests
//!
//! Tests for the ChainSync mini-protocol which handles blockchain synchronization
//! between Cardano nodes. This protocol allows nodes to:
//! - Find intersection points in their chains
//! - Request the next block headers in sequence
//! - Handle chain forks via rollback/rollforward
//! - Maintain consensus across the network
//!
//! Based on the Cardano Network Protocol Specification.

use cardano_network::protocols::chainsync::*;
use cardano_consensus::block_production::{BlockHeader, ForgedBlock};
use cardano_crypto::Blake2b256Hash;
use std::collections::VecDeque;

/// Test data structures and helpers

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Point {
    pub slot: u64,
    pub hash: Blake2b256Hash,
}

impl Point {
    pub fn genesis() -> Self {
        Self {
            slot: 0,
            hash: Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap(),
        }
    }

    pub fn new(slot: u64, hash_bytes: &[u8; 32]) -> Self {
        Self {
            slot,
            hash: Blake2b256Hash::from_bytes(hash_bytes).unwrap(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tip {
    pub slot: u64,
    pub hash: Blake2b256Hash,
    pub height: u64,
}

impl Tip {
    pub fn new(slot: u64, height: u64, hash_bytes: &[u8; 32]) -> Self {
        Self {
            slot,
            height,
            hash: Blake2b256Hash::from_bytes(hash_bytes).unwrap(),
        }
    }
}

/// ChainSync protocol messages as specified in the P2P protocol
#[derive(Debug, Clone, PartialEq)]
pub enum ChainSyncMessage {
    // Client -> Server messages
    RequestNext,
    FindIntersect {
        points: Vec<Point>,
    },

    // Server -> Client messages
    RollForward {
        header: BlockHeader,
        tip: Tip,
    },
    RollBackward {
        point: Point,
        tip: Tip,
    },
    IntersectFound {
        point: Point,
        tip: Tip,
    },
    IntersectNotFound {
        tip: Tip,
    },
}

/// ChainSync protocol state machine states
#[derive(Debug, Clone, PartialEq)]
pub enum ChainSyncState {
    Idle,
    WaitingForNext,
    WaitingForIntersect,
}

/// Mock ChainSync client for testing
#[derive(Debug)]
pub struct MockChainSyncClient {
    state: ChainSyncState,
    chain: Vec<Point>,
    current_tip: Option<Tip>,
    message_queue: VecDeque<ChainSyncMessage>,
}

impl MockChainSyncClient {
    pub fn new() -> Self {
        Self {
            state: ChainSyncState::Idle,
            chain: vec![Point::genesis()],
            current_tip: None,
            message_queue: VecDeque::new(),
        }
    }

    pub fn with_chain(chain: Vec<Point>) -> Self {
        Self {
            state: ChainSyncState::Idle,
            chain,
            current_tip: None,
            message_queue: VecDeque::new(),
        }
    }

    /// Send a message to the server
    pub fn send_message(&mut self, message: ChainSyncMessage) -> Result<(), ChainSyncError> {
        match (&self.state, &message) {
            (ChainSyncState::Idle, ChainSyncMessage::RequestNext) => {
                self.state = ChainSyncState::WaitingForNext;
                self.message_queue.push_back(message);
                Ok(())
            }
            (ChainSyncState::Idle, ChainSyncMessage::FindIntersect { points }) => {
                self.validate_intersect_points(points)?;
                self.state = ChainSyncState::WaitingForIntersect;
                self.message_queue.push_back(message);
                Ok(())
            }
            _ => Err(ChainSyncError::InvalidState {
                current_state: self.state.clone(),
                message: format!("{:?}", message),
            }),
        }
    }

    /// Receive a message from the server
    pub fn receive_message(&mut self, message: ChainSyncMessage) -> Result<(), ChainSyncError> {
        match (&self.state, message) {
            (ChainSyncState::WaitingForNext, ChainSyncMessage::RollForward { header, tip }) => {
                self.handle_roll_forward(header, tip)?;
                self.state = ChainSyncState::Idle;
                Ok(())
            }
            (ChainSyncState::WaitingForNext, ChainSyncMessage::RollBackward { point, tip }) => {
                self.handle_roll_backward(point, tip)?;
                self.state = ChainSyncState::Idle;
                Ok(())
            }
            (ChainSyncState::WaitingForIntersect, ChainSyncMessage::IntersectFound { point, tip }) => {
                self.handle_intersect_found(point, tip);
                self.state = ChainSyncState::Idle;
                Ok(())
            }
            (ChainSyncState::WaitingForIntersect, ChainSyncMessage::IntersectNotFound { tip }) => {
                self.handle_intersect_not_found(tip);
                self.state = ChainSyncState::Idle;
                Ok(())
            }
            _ => Err(ChainSyncError::InvalidState {
                current_state: self.state.clone(),
                message: format!("{:?}", message),
            }),
        }
    }

    fn validate_intersect_points(&self, points: &[Point]) -> Result<(), ChainSyncError> {
        // Points must be in descending slot order as per protocol spec
        for window in points.windows(2) {
            if window[0].slot < window[1].slot {
                return Err(ChainSyncError::InvalidIntersectPoints(
                    "Points must be in descending slot order".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn handle_roll_forward(&mut self, header: BlockHeader, tip: Tip) -> Result<(), ChainSyncError> {
        // Validate that this advances the chain
        if let Some(current_tip) = &self.current_tip {
            if header.slot <= current_tip.slot {
                return Err(ChainSyncError::InvalidRollForward(
                    "Roll forward must advance slot number".to_string(),
                ));
            }
        }

        // Add new point to our chain
        let new_point = Point {
            slot: header.slot,
            hash: header.hash(),
        };
        self.chain.push(new_point);
        self.current_tip = Some(tip);

        Ok(())
    }

    fn handle_roll_backward(&mut self, point: Point, tip: Tip) -> Result<(), ChainSyncError> {
        // Find the rollback point in our chain
        if let Some(pos) = self.chain.iter().position(|p| p.hash == point.hash) {
            // Roll back to this point
            self.chain.truncate(pos + 1);
            self.current_tip = Some(tip);
            Ok(())
        } else {
            Err(ChainSyncError::InvalidRollBackward(
                "Rollback point not found in chain".to_string(),
            ))
        }
    }

    fn handle_intersect_found(&mut self, point: Point, tip: Tip) {
        // Set our chain position to the intersection point
        if let Some(pos) = self.chain.iter().position(|p| p.hash == point.hash) {
            self.chain.truncate(pos + 1);
        }
        self.current_tip = Some(tip);
    }

    fn handle_intersect_not_found(&mut self, tip: Tip) {
        // No intersection found - we may need to roll back further
        self.current_tip = Some(tip);
    }

    pub fn get_state(&self) -> &ChainSyncState {
        &self.state
    }

    pub fn get_chain(&self) -> &[Point] {
        &self.chain
    }

    pub fn get_tip(&self) -> Option<&Tip> {
        self.current_tip.as_ref()
    }
}

/// ChainSync protocol error types
#[derive(Debug, thiserror::Error)]
pub enum ChainSyncError {
    #[error("Invalid state transition: current state {current_state:?}, message: {message}")]
    InvalidState {
        current_state: ChainSyncState,
        message: String,
    },

    #[error("Invalid intersect points: {0}")]
    InvalidIntersectPoints(String),

    #[error("Invalid roll forward: {0}")]
    InvalidRollForward(String),

    #[error("Invalid roll backward: {0}")]
    InvalidRollBackward(String),
}

/// Test cases for ChainSync protocol

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create test block headers
    fn create_test_header(slot: u64, prev_hash: Blake2b256Hash) -> BlockHeader {
        BlockHeader {
            slot,
            prev_hash: Some(prev_hash),
            // Simplified for testing - in real implementation would have more fields
            ..Default::default()
        }
    }

    #[test]
    fn test_chainsync_state_machine_idle_to_waiting_next() {
        let mut client = MockChainSyncClient::new();

        // Should start in Idle state
        assert_eq!(client.get_state(), &ChainSyncState::Idle);

        // Can send RequestNext from Idle
        assert!(client.send_message(ChainSyncMessage::RequestNext).is_ok());
        assert_eq!(client.get_state(), &ChainSyncState::WaitingForNext);
    }

    #[test]
    fn test_chainsync_state_machine_idle_to_waiting_intersect() {
        let mut client = MockChainSyncClient::new();
        let points = vec![
            Point::new(100, &[1u8; 32]),
            Point::new(90, &[2u8; 32]),
            Point::new(80, &[3u8; 32]),
        ];

        // Can send FindIntersect from Idle
        assert!(client.send_message(ChainSyncMessage::FindIntersect {
            points: points.clone()
        }).is_ok());
        assert_eq!(client.get_state(), &ChainSyncState::WaitingForIntersect);
    }

    #[test]
    fn test_chainsync_invalid_state_transitions() {
        let mut client = MockChainSyncClient::new();

        // Put client in WaitingForNext state
        client.send_message(ChainSyncMessage::RequestNext).unwrap();

        // Should not be able to send another RequestNext while waiting
        assert!(client.send_message(ChainSyncMessage::RequestNext).is_err());

        // Should not be able to send FindIntersect while waiting for next
        let points = vec![Point::new(100, &[1u8; 32])];
        assert!(client.send_message(ChainSyncMessage::FindIntersect { points }).is_err());
    }

    #[test]
    fn test_chainsync_roll_forward_advances_chain() {
        let mut client = MockChainSyncClient::new();

        // Request next block
        client.send_message(ChainSyncMessage::RequestNext).unwrap();

        // Receive roll forward
        let header = create_test_header(1, Point::genesis().hash);
        let tip = Tip::new(1, 1, &[1u8; 32]);

        assert!(client.receive_message(ChainSyncMessage::RollForward {
            header,
            tip: tip.clone(),
        }).is_ok());

        // Should be back in Idle state
        assert_eq!(client.get_state(), &ChainSyncState::Idle);

        // Chain should have advanced
        assert_eq!(client.get_chain().len(), 2);
        assert_eq!(client.get_tip(), Some(&tip));
    }

    #[test]
    fn test_chainsync_roll_backward_reverts_chain() {
        let initial_chain = vec![
            Point::genesis(),
            Point::new(1, &[1u8; 32]),
            Point::new(2, &[2u8; 32]),
            Point::new(3, &[3u8; 32]),
        ];
        let mut client = MockChainSyncClient::with_chain(initial_chain.clone());

        // Request next block
        client.send_message(ChainSyncMessage::RequestNext).unwrap();

        // Receive roll backward to slot 1
        let rollback_point = Point::new(1, &[1u8; 32]);
        let tip = Tip::new(5, 5, &[5u8; 32]);

        assert!(client.receive_message(ChainSyncMessage::RollBackward {
            point: rollback_point,
            tip: tip.clone(),
        }).is_ok());

        // Chain should be rolled back
        assert_eq!(client.get_chain().len(), 2); // genesis + slot 1
        assert_eq!(client.get_tip(), Some(&tip));
    }

    #[test]
    fn test_chainsync_intersect_points_validation() {
        let mut client = MockChainSyncClient::new();

        // Points in wrong order (ascending instead of descending)
        let invalid_points = vec![
            Point::new(80, &[3u8; 32]),
            Point::new(90, &[2u8; 32]),  // Invalid: should be descending
            Point::new(100, &[1u8; 32]),
        ];

        assert!(client.send_message(ChainSyncMessage::FindIntersect {
            points: invalid_points
        }).is_err());

        // Valid points in descending order
        let valid_points = vec![
            Point::new(100, &[1u8; 32]),
            Point::new(90, &[2u8; 32]),
            Point::new(80, &[3u8; 32]),
        ];

        assert!(client.send_message(ChainSyncMessage::FindIntersect {
            points: valid_points
        }).is_ok());
    }

    #[test]
    fn test_chainsync_intersect_found_truncates_chain() {
        let initial_chain = vec![
            Point::genesis(),
            Point::new(1, &[1u8; 32]),
            Point::new(2, &[2u8; 32]),
            Point::new(3, &[3u8; 32]),
        ];
        let mut client = MockChainSyncClient::with_chain(initial_chain.clone());

        // Find intersection
        let points = vec![Point::new(2, &[2u8; 32])];
        client.send_message(ChainSyncMessage::FindIntersect { points }).unwrap();

        // Receive intersect found at slot 2
        let intersect_point = Point::new(2, &[2u8; 32]);
        let tip = Tip::new(10, 10, &[10u8; 32]);

        assert!(client.receive_message(ChainSyncMessage::IntersectFound {
            point: intersect_point,
            tip: tip.clone(),
        }).is_ok());

        // Chain should be truncated at intersection point
        assert_eq!(client.get_chain().len(), 3); // genesis + slot 1 + slot 2
        assert_eq!(client.get_tip(), Some(&tip));
    }

    #[test]
    fn test_chainsync_intersect_not_found() {
        let mut client = MockChainSyncClient::new();

        // Find intersection
        let points = vec![Point::new(100, &[100u8; 32])];
        client.send_message(ChainSyncMessage::FindIntersect { points }).unwrap();

        // Receive intersect not found
        let tip = Tip::new(200, 200, &[200u8; 32]);

        assert!(client.receive_message(ChainSyncMessage::IntersectNotFound {
            tip: tip.clone(),
        }).is_ok());

        // Should update tip but not modify chain
        assert_eq!(client.get_chain().len(), 1); // Still just genesis
        assert_eq!(client.get_tip(), Some(&tip));
    }

    #[test]
    fn test_chainsync_invalid_roll_forward_non_advancing() {
        let mut client = MockChainSyncClient::new();
        client.current_tip = Some(Tip::new(5, 5, &[5u8; 32]));

        client.send_message(ChainSyncMessage::RequestNext).unwrap();

        // Try to roll forward to earlier slot (should fail)
        let header = create_test_header(3, Point::genesis().hash);
        let tip = Tip::new(5, 5, &[5u8; 32]);

        assert!(client.receive_message(ChainSyncMessage::RollForward {
            header,
            tip,
        }).is_err());
    }

    #[test]
    fn test_chainsync_invalid_roll_backward_unknown_point() {
        let mut client = MockChainSyncClient::new();

        client.send_message(ChainSyncMessage::RequestNext).unwrap();

        // Try to roll back to point not in our chain
        let unknown_point = Point::new(999, &[999u8; 32]);
        let tip = Tip::new(5, 5, &[5u8; 32]);

        assert!(client.receive_message(ChainSyncMessage::RollBackward {
            point: unknown_point,
            tip,
        }).is_err());
    }

    #[test]
    fn test_chainsync_complex_fork_scenario() {
        // Simulate a complex fork scenario where the client needs to:
        // 1. Find intersection with server
        // 2. Roll back to intersection point
        // 3. Roll forward on new branch

        let initial_chain = vec![
            Point::genesis(),
            Point::new(1, &[1u8; 32]),
            Point::new(2, &[2u8; 32]),
            Point::new(3, &[3u8; 32]),  // This will be the fork point
            Point::new(4, &[4u8; 32]),  // These blocks will be replaced
            Point::new(5, &[5u8; 32]),
        ];
        let mut client = MockChainSyncClient::with_chain(initial_chain);

        // 1. Find intersection
        let intersect_points = vec![
            Point::new(5, &[5u8; 32]),  // Our current tip (not on server)
            Point::new(4, &[4u8; 32]),  // Our previous block (not on server)
            Point::new(3, &[3u8; 32]),  // Common ancestor (on server)
        ];

        client.send_message(ChainSyncMessage::FindIntersect {
            points: intersect_points
        }).unwrap();

        // Server finds intersection at slot 3
        let intersection = Point::new(3, &[3u8; 32]);
        let server_tip = Tip::new(7, 7, &[77u8; 32]);

        client.receive_message(ChainSyncMessage::IntersectFound {
            point: intersection,
            tip: server_tip.clone(),
        }).unwrap();

        // Chain should be truncated to intersection point
        assert_eq!(client.get_chain().len(), 4); // genesis + slots 1,2,3

        // 2. Request next blocks on the server's chain
        client.send_message(ChainSyncMessage::RequestNext).unwrap();

        // Server sends alternative block at slot 4
        let alt_header_4 = create_test_header(4, Point::new(3, &[3u8; 32]).hash);
        client.receive_message(ChainSyncMessage::RollForward {
            header: alt_header_4,
            tip: server_tip.clone(),
        }).unwrap();

        // Continue to get more blocks
        client.send_message(ChainSyncMessage::RequestNext).unwrap();
        let alt_header_5 = create_test_header(5, Blake2b256Hash::from_bytes(&[44u8; 32]).unwrap());
        client.receive_message(ChainSyncMessage::RollForward {
            header: alt_header_5,
            tip: server_tip.clone(),
        }).unwrap();

        // Final chain should have the alternative fork
        assert_eq!(client.get_chain().len(), 6); // genesis + 1,2,3,4',5'
        assert_eq!(client.get_tip(), Some(&server_tip));
    }

    #[test]
    fn test_chainsync_message_serialization_properties() {
        // Test that ChainSync messages have correct properties for serialization
        let point = Point::new(100, &[1u8; 32]);
        let tip = Tip::new(200, 150, &[2u8; 32]);

        // Test message variants
        let messages = vec![
            ChainSyncMessage::RequestNext,
            ChainSyncMessage::FindIntersect {
                points: vec![point.clone()]
            },
            ChainSyncMessage::IntersectFound {
                point: point.clone(),
                tip: tip.clone()
            },
            ChainSyncMessage::IntersectNotFound {
                tip: tip.clone()
            },
        ];

        // All messages should implement required traits
        for message in messages {
            // Clone
            let cloned = message.clone();
            assert_eq!(message, cloned);

            // Debug
            let debug_str = format!("{:?}", message);
            assert!(!debug_str.is_empty());

            // PartialEq
            assert_eq!(message, message);
        }
    }

    #[test]
    fn test_chainsync_protocol_invariants() {
        // Test that the protocol maintains important invariants:
        // 1. State transitions are deterministic
        // 2. Invalid messages are rejected
        // 3. Chain integrity is maintained

        let mut client = MockChainSyncClient::new();

        // Invariant 1: State transitions are deterministic
        assert_eq!(client.get_state(), &ChainSyncState::Idle);

        client.send_message(ChainSyncMessage::RequestNext).unwrap();
        assert_eq!(client.get_state(), &ChainSyncState::WaitingForNext);

        // Invariant 2: Invalid messages are rejected
        assert!(client.send_message(ChainSyncMessage::RequestNext).is_err());

        // Invariant 3: Chain integrity - genesis should always be first
        assert_eq!(client.get_chain()[0], Point::genesis());

        // Add some blocks and verify chain ordering
        let header1 = create_test_header(1, Point::genesis().hash);
        let tip1 = Tip::new(1, 1, &[1u8; 32]);

        client.receive_message(ChainSyncMessage::RollForward {
            header: header1,
            tip: tip1,
        }).unwrap();

        // Chain should maintain slot ordering
        let chain = client.get_chain();
        for window in chain.windows(2) {
            assert!(window[0].slot <= window[1].slot,
                "Chain slots should be in non-decreasing order");
        }
    }
}
