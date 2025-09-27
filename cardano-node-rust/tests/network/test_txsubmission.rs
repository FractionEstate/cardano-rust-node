//! TxSubmission Protocol Tests
//!
//! Tests for the TxSubmission mini-protocol which handles transaction propagation
//! across the Cardano network. This protocol allows nodes to:
//! - Exchange lists of transaction IDs in their mempools
//! - Request full transaction data for specific IDs
//! - Manage flow control and bandwidth usage
//! - Handle transaction validation and mempool management
//!
//! Based on the Cardano Network Protocol Specification.

use cardano_network::protocols::txsubmission::*;
use cardano_consensus::block_production::{Transaction, TxInput, TxOutput};
use cardano_crypto::Blake2b256Hash;
use std::collections::{HashMap, HashSet, VecDeque};

/// Transaction ID type for testing
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TxId(pub Blake2b256Hash);

impl TxId {
    pub fn new(bytes: &[u8; 32]) -> Self {
        Self(Blake2b256Hash::from_bytes(bytes).unwrap())
    }

    pub fn random(seed: u8) -> Self {
        let mut bytes = [0u8; 32];
        bytes[0] = seed;
        bytes[31] = seed.wrapping_add(1);
        Self::new(&bytes)
    }
}

/// Transaction size in bytes
pub type TxSize = u32;

/// TxSubmission protocol messages
#[derive(Debug, Clone, PartialEq)]
pub enum TxSubmissionMessage {
    // Bidirectional messages (both client and server can send)
    RequestTxIds {
        blocking: bool,
        ack: u16,
        req: u16,
    },
    ReplyTxIds {
        tx_ids: Vec<(TxId, TxSize)>,
    },
    RequestTxs {
        tx_ids: Vec<TxId>,
    },
    ReplyTxs {
        txs: Vec<Transaction>,
    },
}

/// TxSubmission protocol states
#[derive(Debug, Clone, PartialEq)]
pub enum TxSubmissionState {
    Idle,
    WaitingForTxIds,
    WaitingForTxs,
}

/// Flow control numbers for transaction submission
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowControl {
    pub ack: u16,  // Number of tx IDs we acknowledge receiving
    pub req: u16,  // Number of tx IDs we request
}

impl FlowControl {
    pub fn new(ack: u16, req: u16) -> Self {
        Self { ack, req }
    }

    pub fn initial() -> Self {
        Self::new(0, 100) // Request up to 100 tx IDs initially
    }

    /// Update flow control after receiving tx IDs
    pub fn update_after_receive(&mut self, received_count: u16) {
        self.ack = self.ack.wrapping_add(received_count);
    }

    /// Check if we should request more tx IDs
    pub fn should_request_more(&self, current_count: u16) -> bool {
        current_count < self.req
    }
}

/// Mock mempool for testing transaction submission
#[derive(Debug)]
pub struct MockMempool {
    transactions: HashMap<TxId, Transaction>,
    tx_sizes: HashMap<TxId, TxSize>,
    known_tx_ids: HashSet<TxId>,
}

impl MockMempool {
    pub fn new() -> Self {
        Self {
            transactions: HashMap::new(),
            tx_sizes: HashMap::new(),
            known_tx_ids: HashSet::new(),
        }
    }

    pub fn add_transaction(&mut self, tx_id: TxId, tx: Transaction, size: TxSize) {
        self.transactions.insert(tx_id.clone(), tx);
        self.tx_sizes.insert(tx_id.clone(), size);
        self.known_tx_ids.insert(tx_id);
    }

    pub fn get_transaction(&self, tx_id: &TxId) -> Option<&Transaction> {
        self.transactions.get(tx_id)
    }

    pub fn get_tx_size(&self, tx_id: &TxId) -> Option<TxSize> {
        self.tx_sizes.get(tx_id).copied()
    }

    pub fn has_transaction(&self, tx_id: &TxId) -> bool {
        self.known_tx_ids.contains(tx_id)
    }

    pub fn get_available_tx_ids(&self) -> Vec<(TxId, TxSize)> {
        self.tx_sizes.iter()
            .map(|(id, &size)| (id.clone(), size))
            .collect()
    }

    pub fn get_unknown_tx_ids(&self, peer_tx_ids: &[(TxId, TxSize)]) -> Vec<TxId> {
        peer_tx_ids.iter()
            .filter(|(id, _)| !self.known_tx_ids.contains(id))
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn mark_known(&mut self, tx_id: TxId) {
        self.known_tx_ids.insert(tx_id);
    }

    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }
}

/// Mock TxSubmission client for testing
#[derive(Debug)]
pub struct MockTxSubmissionClient {
    state: TxSubmissionState,
    flow_control: FlowControl,
    mempool: MockMempool,
    pending_tx_requests: Vec<TxId>,
    received_tx_ids: Vec<(TxId, TxSize)>,
    message_queue: VecDeque<TxSubmissionMessage>,
}

impl MockTxSubmissionClient {
    pub fn new() -> Self {
        Self {
            state: TxSubmissionState::Idle,
            flow_control: FlowControl::initial(),
            mempool: MockMempool::new(),
            pending_tx_requests: Vec::new(),
            received_tx_ids: Vec::new(),
            message_queue: VecDeque::new(),
        }
    }

    /// Request transaction IDs from peer
    pub fn request_tx_ids(&mut self, blocking: bool) -> Result<(), TxSubmissionError> {
        match self.state {
            TxSubmissionState::Idle => {
                self.state = TxSubmissionState::WaitingForTxIds;
                let message = TxSubmissionMessage::RequestTxIds {
                    blocking,
                    ack: self.flow_control.ack,
                    req: self.flow_control.req,
                };
                self.message_queue.push_back(message);
                Ok(())
            }
            _ => Err(TxSubmissionError::InvalidState {
                current_state: self.state.clone(),
                message: "Can only request tx IDs from Idle state".to_string(),
            }),
        }
    }

    /// Request specific transactions
    pub fn request_transactions(&mut self, tx_ids: Vec<TxId>) -> Result<(), TxSubmissionError> {
        match self.state {
            TxSubmissionState::Idle => {
                if tx_ids.is_empty() {
                    return Err(TxSubmissionError::EmptyRequest);
                }

                self.state = TxSubmissionState::WaitingForTxs;
                self.pending_tx_requests = tx_ids.clone();
                let message = TxSubmissionMessage::RequestTxs { tx_ids };
                self.message_queue.push_back(message);
                Ok(())
            }
            _ => Err(TxSubmissionError::InvalidState {
                current_state: self.state.clone(),
                message: "Can only request transactions from Idle state".to_string(),
            }),
        }
    }

    /// Handle reply from peer
    pub fn receive_message(&mut self, message: TxSubmissionMessage) -> Result<(), TxSubmissionError> {
        match (&self.state, &message) {
            (TxSubmissionState::WaitingForTxIds, TxSubmissionMessage::ReplyTxIds { tx_ids }) => {
                self.handle_tx_ids_reply(tx_ids.clone())?;
                self.state = TxSubmissionState::Idle;
                Ok(())
            }
            (TxSubmissionState::WaitingForTxs, TxSubmissionMessage::ReplyTxs { txs }) => {
                self.handle_txs_reply(txs.clone())?;
                self.state = TxSubmissionState::Idle;
                Ok(())
            }
            _ => Err(TxSubmissionError::InvalidState {
                current_state: self.state.clone(),
                message: format!("Unexpected message: {:?}", message),
            }),
        }
    }

    fn handle_tx_ids_reply(&mut self, tx_ids: Vec<(TxId, TxSize)>) -> Result<(), TxSubmissionError> {
        // Validate flow control
        if tx_ids.len() > self.flow_control.req as usize {
            return Err(TxSubmissionError::FlowControlViolation {
                requested: self.flow_control.req,
                received: tx_ids.len() as u16,
            });
        }

        // Update flow control
        self.flow_control.update_after_receive(tx_ids.len() as u16);

        // Store received tx IDs
        self.received_tx_ids.extend(tx_ids);

        Ok(())
    }

    fn handle_txs_reply(&mut self, txs: Vec<Transaction>) -> Result<(), TxSubmissionError> {
        // Validate that we receive the transactions we requested
        if txs.len() != self.pending_tx_requests.len() {
            return Err(TxSubmissionError::IncorrectTxCount {
                requested: self.pending_tx_requests.len(),
                received: txs.len(),
            });
        }

        // Add transactions to our mempool
        for (i, tx) in txs.iter().enumerate() {
            let tx_id = &self.pending_tx_requests[i];
            let tx_size = self.estimate_transaction_size(tx);
            self.mempool.add_transaction(tx_id.clone(), tx.clone(), tx_size);
        }

        self.pending_tx_requests.clear();
        Ok(())
    }

    fn estimate_transaction_size(&self, _tx: &Transaction) -> TxSize {
        // Simplified size estimation for testing
        250 // Average transaction size in bytes
    }

    /// Get transactions we don't have yet
    pub fn get_missing_transactions(&self) -> Vec<TxId> {
        self.mempool.get_unknown_tx_ids(&self.received_tx_ids)
    }

    /// Send reply to peer's request
    pub fn send_reply(&mut self, request: TxSubmissionMessage) -> Result<TxSubmissionMessage, TxSubmissionError> {
        match request {
            TxSubmissionMessage::RequestTxIds { blocking: _, ack, req } => {
                // Update our understanding of peer's flow control
                let available_ids = self.mempool.get_available_tx_ids();
                let tx_ids_to_send = available_ids.into_iter().take(req as usize).collect();

                Ok(TxSubmissionMessage::ReplyTxIds { tx_ids: tx_ids_to_send })
            }
            TxSubmissionMessage::RequestTxs { tx_ids } => {
                let mut txs = Vec::new();
                for tx_id in &tx_ids {
                    if let Some(tx) = self.mempool.get_transaction(tx_id) {
                        txs.push(tx.clone());
                    } else {
                        return Err(TxSubmissionError::TransactionNotFound {
                            tx_id: tx_id.clone(),
                        });
                    }
                }
                Ok(TxSubmissionMessage::ReplyTxs { txs })
            }
            _ => Err(TxSubmissionError::InvalidRequest {
                message: format!("Cannot reply to {:?}", request),
            }),
        }
    }

    // Getters for testing
    pub fn get_state(&self) -> &TxSubmissionState {
        &self.state
    }

    pub fn get_flow_control(&self) -> &FlowControl {
        &self.flow_control
    }

    pub fn get_mempool(&self) -> &MockMempool {
        &self.mempool
    }

    pub fn get_received_tx_ids(&self) -> &[(TxId, TxSize)] {
        &self.received_tx_ids
    }

    pub fn has_pending_requests(&self) -> bool {
        !self.pending_tx_requests.is_empty()
    }
}

/// TxSubmission protocol errors
#[derive(Debug, thiserror::Error)]
pub enum TxSubmissionError {
    #[error("Invalid state transition: current state {current_state:?}, message: {message}")]
    InvalidState {
        current_state: TxSubmissionState,
        message: String,
    },

    #[error("Flow control violation: requested {requested}, received {received}")]
    FlowControlViolation { requested: u16, received: u16 },

    #[error("Empty request not allowed")]
    EmptyRequest,

    #[error("Incorrect transaction count: requested {requested}, received {received}")]
    IncorrectTxCount { requested: usize, received: usize },

    #[error("Transaction not found: {tx_id:?}")]
    TransactionNotFound { tx_id: TxId },

    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },

    #[error("Mempool error: {0}")]
    MempoolError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create test transactions
    fn create_test_transaction(inputs: Vec<TxInput>, outputs: Vec<TxOutput>) -> Transaction {
        Transaction {
            inputs,
            outputs,
            fee: 100000, // 0.1 ADA
            ttl: None,
            certificates: Vec::new(),
            withdrawals: HashMap::new(),
            auxiliary_data: None,
            witness_set: None,
        }
    }

    fn create_simple_transaction() -> Transaction {
        create_test_transaction(Vec::new(), Vec::new())
    }

    #[test]
    fn test_txsubmission_state_machine_idle_to_waiting_ids() {
        let mut client = MockTxSubmissionClient::new();

        // Should start in Idle state
        assert_eq!(client.get_state(), &TxSubmissionState::Idle);

        // Can request tx IDs from Idle
        assert!(client.request_tx_ids(false).is_ok());
        assert_eq!(client.get_state(), &TxSubmissionState::WaitingForTxIds);
    }

    #[test]
    fn test_txsubmission_state_machine_idle_to_waiting_txs() {
        let mut client = MockTxSubmissionClient::new();

        let tx_ids = vec![
            TxId::random(1),
            TxId::random(2),
            TxId::random(3),
        ];

        // Can request transactions from Idle
        assert!(client.request_transactions(tx_ids).is_ok());
        assert_eq!(client.get_state(), &TxSubmissionState::WaitingForTxs);
        assert!(client.has_pending_requests());
    }

    #[test]
    fn test_txsubmission_empty_request_rejected() {
        let mut client = MockTxSubmissionClient::new();

        // Empty transaction request should be rejected
        assert!(client.request_transactions(Vec::new()).is_err());
        assert_eq!(client.get_state(), &TxSubmissionState::Idle);
    }

    #[test]
    fn test_txsubmission_invalid_state_transitions() {
        let mut client = MockTxSubmissionClient::new();

        // Request tx IDs (goes to WaitingForTxIds)
        client.request_tx_ids(false).unwrap();

        // Should not be able to make another request while waiting
        assert!(client.request_tx_ids(false).is_err());
        assert!(client.request_transactions(vec![TxId::random(1)]).is_err());
    }

    #[test]
    fn test_txsubmission_tx_ids_reply_success() {
        let mut client = MockTxSubmissionClient::new();

        // Request tx IDs
        client.request_tx_ids(false).unwrap();

        // Receive reply with some tx IDs
        let tx_ids = vec![
            (TxId::random(1), 200),
            (TxId::random(2), 300),
            (TxId::random(3), 250),
        ];

        let reply = TxSubmissionMessage::ReplyTxIds { tx_ids: tx_ids.clone() };
        assert!(client.receive_message(reply).is_ok());

        // Should be back in Idle state
        assert_eq!(client.get_state(), &TxSubmissionState::Idle);

        // Should have received the tx IDs
        assert_eq!(client.get_received_tx_ids().len(), 3);

        // Flow control should be updated
        let flow_control = client.get_flow_control();
        assert_eq!(flow_control.ack, 3); // Acknowledged 3 tx IDs
    }

    #[test]
    fn test_txsubmission_flow_control_violation() {
        let mut client = MockTxSubmissionClient::new();

        // Set up flow control to request only 2 tx IDs
        client.flow_control = FlowControl::new(0, 2);
        client.request_tx_ids(false).unwrap();

        // Peer sends more tx IDs than requested (should fail)
        let tx_ids = vec![
            (TxId::random(1), 200),
            (TxId::random(2), 300),
            (TxId::random(3), 250), // This exceeds the request
        ];

        let reply = TxSubmissionMessage::ReplyTxIds { tx_ids };
        assert!(client.receive_message(reply).is_err());
    }

    #[test]
    fn test_txsubmission_transaction_request_and_reply() {
        let mut client = MockTxSubmissionClient::new();

        let tx_ids = vec![
            TxId::random(1),
            TxId::random(2),
        ];

        // Request specific transactions
        client.request_transactions(tx_ids.clone()).unwrap();

        // Create matching transactions
        let txs = vec![
            create_simple_transaction(),
            create_simple_transaction(),
        ];

        let reply = TxSubmissionMessage::ReplyTxs { txs: txs.clone() };
        assert!(client.receive_message(reply).is_ok());

        // Should be back in Idle state
        assert_eq!(client.get_state(), &TxSubmissionState::Idle);
        assert!(!client.has_pending_requests());

        // Transactions should be in mempool
        assert_eq!(client.get_mempool().len(), 2);
        for tx_id in &tx_ids {
            assert!(client.get_mempool().has_transaction(tx_id));
        }
    }

    #[test]
    fn test_txsubmission_incorrect_tx_count() {
        let mut client = MockTxSubmissionClient::new();

        let tx_ids = vec![TxId::random(1), TxId::random(2)];
        client.request_transactions(tx_ids).unwrap();

        // Peer sends wrong number of transactions
        let txs = vec![create_simple_transaction()]; // Only 1 instead of 2

        let reply = TxSubmissionMessage::ReplyTxs { txs };
        assert!(client.receive_message(reply).is_err());
    }

    #[test]
    fn test_txsubmission_bidirectional_communication() {
        // Test that both peers can send requests and replies
        let mut client1 = MockTxSubmissionClient::new();
        let mut client2 = MockTxSubmissionClient::new();

        // Client2 has some transactions
        let tx1 = create_simple_transaction();
        let tx2 = create_simple_transaction();
        let tx_id1 = TxId::random(1);
        let tx_id2 = TxId::random(2);

        client2.mempool.add_transaction(tx_id1.clone(), tx1.clone(), 200);
        client2.mempool.add_transaction(tx_id2.clone(), tx2.clone(), 300);

        // Client1 requests tx IDs from Client2
        client1.request_tx_ids(false).unwrap();

        let request = TxSubmissionMessage::RequestTxIds {
            blocking: false,
            ack: 0,
            req: 10,
        };

        // Client2 sends reply
        let reply = client2.send_reply(request).unwrap();

        // Client1 receives reply
        client1.receive_message(reply).unwrap();

        // Client1 should now know about the tx IDs
        let missing_txs = client1.get_missing_transactions();
        assert_eq!(missing_txs.len(), 2);

        // Client1 requests the actual transactions
        client1.request_transactions(missing_txs).unwrap();

        let tx_request = TxSubmissionMessage::RequestTxs {
            tx_ids: vec![tx_id1.clone(), tx_id2.clone()],
        };

        // Client2 sends the transactions
        let tx_reply = client2.send_reply(tx_request).unwrap();

        // Client1 receives the transactions
        client1.receive_message(tx_reply).unwrap();

        // Client1 should now have both transactions
        assert_eq!(client1.get_mempool().len(), 2);
        assert!(client1.get_mempool().has_transaction(&tx_id1));
        assert!(client1.get_mempool().has_transaction(&tx_id2));
    }

    #[test]
    fn test_txsubmission_flow_control_behavior() {
        // Test proper flow control behavior
        let mut client = MockTxSubmissionClient::new();

        // Initial flow control
        let initial_fc = client.get_flow_control();
        assert_eq!(initial_fc.ack, 0);
        assert_eq!(initial_fc.req, 100);

        // Request tx IDs
        client.request_tx_ids(false).unwrap();

        // Receive some tx IDs
        let tx_ids = vec![
            (TxId::random(1), 200),
            (TxId::random(2), 300),
        ];

        let reply = TxSubmissionMessage::ReplyTxIds { tx_ids: tx_ids.clone() };
        client.receive_message(reply).unwrap();

        // Flow control should be updated
        let updated_fc = client.get_flow_control();
        assert_eq!(updated_fc.ack, 2); // Acknowledged 2 tx IDs
        assert_eq!(updated_fc.req, 100); // Request limit unchanged

        // Should still want to request more if we have room
        assert!(updated_fc.should_request_more(2));
        assert!(!updated_fc.should_request_more(100));
    }

    #[test]
    fn test_txsubmission_mempool_operations() {
        let mut mempool = MockMempool::new();

        // Initially empty
        assert!(mempool.is_empty());
        assert_eq!(mempool.len(), 0);

        // Add some transactions
        let tx1 = create_simple_transaction();
        let tx2 = create_simple_transaction();
        let tx_id1 = TxId::random(1);
        let tx_id2 = TxId::random(2);

        mempool.add_transaction(tx_id1.clone(), tx1.clone(), 200);
        mempool.add_transaction(tx_id2.clone(), tx2.clone(), 300);

        // Should have 2 transactions
        assert!(!mempool.is_empty());
        assert_eq!(mempool.len(), 2);

        // Should be able to retrieve transactions
        assert!(mempool.has_transaction(&tx_id1));
        assert!(mempool.has_transaction(&tx_id2));
        assert_eq!(mempool.get_tx_size(&tx_id1), Some(200));
        assert_eq!(mempool.get_tx_size(&tx_id2), Some(300));

        // Should get available tx IDs
        let available = mempool.get_available_tx_ids();
        assert_eq!(available.len(), 2);

        // Should identify unknown tx IDs
        let peer_tx_ids = vec![
            (tx_id1.clone(), 200), // Known
            (TxId::random(99), 150), // Unknown
        ];
        let unknown = mempool.get_unknown_tx_ids(&peer_tx_ids);
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0], TxId::random(99));
    }

    #[test]
    fn test_txsubmission_blocking_vs_nonblocking() {
        let mut client = MockTxSubmissionClient::new();

        // Test non-blocking request
        client.request_tx_ids(false).unwrap();

        // Should create non-blocking message
        let message = &client.message_queue[0];
        if let TxSubmissionMessage::RequestTxIds { blocking, .. } = message {
            assert!(!blocking);
        } else {
            panic!("Expected RequestTxIds message");
        }

        // Reset for blocking test
        client.state = TxSubmissionState::Idle;
        client.message_queue.clear();

        // Test blocking request
        client.request_tx_ids(true).unwrap();

        let message = &client.message_queue[0];
        if let TxSubmissionMessage::RequestTxIds { blocking, .. } = message {
            assert!(blocking);
        } else {
            panic!("Expected RequestTxIds message");
        }
    }

    #[test]
    fn test_txsubmission_large_mempool_simulation() {
        // Test handling of large mempools
        let mut client1 = MockTxSubmissionClient::new();
        let mut client2 = MockTxSubmissionClient::new();

        // Client2 has a large mempool (1000 transactions)
        for i in 0..1000 {
            let tx = create_simple_transaction();
            let tx_id = TxId::new(&[(i as u8), (i >> 8) as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            client2.mempool.add_transaction(tx_id, tx, 250);
        }

        // Client1 requests tx IDs with limited flow control
        client1.flow_control = FlowControl::new(0, 50); // Only request 50 at a time
        client1.request_tx_ids(false).unwrap();

        let request = TxSubmissionMessage::RequestTxIds {
            blocking: false,
            ack: 0,
            req: 50,
        };

        // Client2 should only send 50 tx IDs
        let reply = client2.send_reply(request).unwrap();

        if let TxSubmissionMessage::ReplyTxIds { tx_ids } = reply {
            assert_eq!(tx_ids.len(), 50);
        } else {
            panic!("Expected ReplyTxIds");
        }
    }

    #[test]
    fn test_txsubmission_protocol_invariants() {
        // Test important protocol invariants
        let mut client = MockTxSubmissionClient::new();

        // Invariant 1: State transitions are deterministic
        assert_eq!(client.get_state(), &TxSubmissionState::Idle);

        client.request_tx_ids(false).unwrap();
        assert_eq!(client.get_state(), &TxSubmissionState::WaitingForTxIds);

        let reply = TxSubmissionMessage::ReplyTxIds { tx_ids: Vec::new() };
        client.receive_message(reply).unwrap();
        assert_eq!(client.get_state(), &TxSubmissionState::Idle);

        // Invariant 2: Flow control numbers are consistent
        let fc = client.get_flow_control();
        assert!(fc.ack <= fc.req, "Ack should not exceed req");

        // Invariant 3: Request/reply matching
        let tx_ids = vec![TxId::random(1), TxId::random(2)];
        client.request_transactions(tx_ids.clone()).unwrap();

        let txs = vec![create_simple_transaction(), create_simple_transaction()];
        let reply = TxSubmissionMessage::ReplyTxs { txs };

        assert!(client.receive_message(reply).is_ok());

        // All requested transactions should be in mempool
        for tx_id in &tx_ids {
            assert!(client.get_mempool().has_transaction(tx_id));
        }
    }

    #[test]
    fn test_txsubmission_error_recovery() {
        // Test error handling and recovery
        let mut client = MockTxSubmissionClient::new();

        // Test recovery from flow control violation
        client.flow_control = FlowControl::new(0, 2);
        client.request_tx_ids(false).unwrap();

        let bad_reply = TxSubmissionMessage::ReplyTxIds {
            tx_ids: vec![
                (TxId::random(1), 200),
                (TxId::random(2), 300),
                (TxId::random(3), 250), // Exceeds request
            ],
        };

        // Should fail
        assert!(client.receive_message(bad_reply).is_err());

        // State should remain WaitingForTxIds (no state change on error)
        assert_eq!(client.get_state(), &TxSubmissionState::WaitingForTxIds);

        // Should be able to recover with correct reply
        let good_reply = TxSubmissionMessage::ReplyTxIds {
            tx_ids: vec![
                (TxId::random(1), 200),
                (TxId::random(2), 300),
            ],
        };

        assert!(client.receive_message(good_reply).is_ok());
        assert_eq!(client.get_state(), &TxSubmissionState::Idle);
    }

    #[test]
    fn test_txsubmission_concurrent_operations() {
        // Simulate concurrent transaction submission between multiple peers
        let mut clients: Vec<MockTxSubmissionClient> = (0..3).map(|_| MockTxSubmissionClient::new()).collect();

        // Each client has different transactions
        for (i, client) in clients.iter_mut().enumerate() {
            for j in 0..5 {
                let tx = create_simple_transaction();
                let tx_id = TxId::random((i * 10 + j) as u8);
                client.mempool.add_transaction(tx_id, tx, 250);
            }
        }

        // Simulate round-robin information exchange
        for i in 0..clients.len() {
            for j in 0..clients.len() {
                if i != j {
                    // Client i requests tx IDs from client j
                    let request = TxSubmissionMessage::RequestTxIds {
                        blocking: false,
                        ack: 0,
                        req: 10,
                    };

                    let reply = clients[j].send_reply(request).unwrap();

                    // Client i processes the reply
                    clients[i].receive_message(reply).unwrap();
                }
            }
        }

        // Each client should now know about transactions from other clients
        for (i, client) in clients.iter().enumerate() {
            let received_count = client.get_received_tx_ids().len();
            // Should have received tx IDs from the other 2 clients (5 each)
            assert_eq!(received_count, 10, "Client {} received {} tx IDs", i, received_count);
        }
    }
}
