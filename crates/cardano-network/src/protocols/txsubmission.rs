//! TxSubmission Protocol Implementation
//!
//! The TxSubmission mini-protocol handles transaction propagation across the Cardano network.
//! It allows nodes to exchange lists of transaction IDs in their mempools, request full transaction
//! data for specific IDs, manage flow control and bandwidth usage, and handle transaction validation
//! and mempool management.
//!
//! # Protocol Overview
//!
//! The TxSubmission protocol operates as a bidirectional interaction where both sides can:
//! - Request transaction IDs from each other's mempools
//! - Reply with available transaction ID lists
//! - Request specific transaction data by ID
//! - Reply with full transaction content
//!
//! # State Machine
//!
//! ```text
//! Idle <-> RequestTxIds -> ReplyTxIds -> Idle
//!      <-> RequestTxs -> ReplyTxs -> Idle
//! ```
//!
//! # Flow Control
//!
//! Uses ack/req numbers to manage bandwidth:
//! - `ack`: Number of transaction IDs acknowledged as received
//! - `req`: Number of transaction IDs requested from peer
//!
//! Based on the Cardano Network Protocol Specification.

use cardano_consensus::block_production::Transaction;
use cardano_crypto::Blake2b256Hash;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;

/// Transaction ID - unique identifier for a transaction
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TxId(pub Blake2b256Hash);

impl TxId {
    /// Create a new transaction ID from hash bytes
    pub fn new(bytes: &[u8; 32]) -> Result<Self, String> {
        Blake2b256Hash::from_bytes(bytes)
            .map(Self)
            .map_err(|e| format!("Invalid transaction ID bytes: {}", e))
    }

    /// Create a random transaction ID for testing
    pub fn random(seed: u8) -> Self {
        let mut bytes = [0u8; 32];
        bytes[0] = seed;
        bytes[31] = seed.wrapping_add(1);
        for (i, byte) in bytes.iter_mut().enumerate().take(31).skip(1) {
            *byte = seed.wrapping_add(i as u8);
        }
        // This should never fail since we're using valid 32-byte array
        Self::new(&bytes).expect("Random TxId creation should not fail")
    }

    /// Get the underlying hash
    pub fn hash(&self) -> &Blake2b256Hash {
        &self.0
    }
}

/// Transaction size in bytes
pub type TxSize = u32;

/// Flow control parameters for managing transaction submission bandwidth
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowControl {
    /// Number of transaction IDs acknowledged as received
    pub ack: u16,
    /// Number of transaction IDs requested from peer
    pub req: u16,
}

impl FlowControl {
    /// Create new flow control parameters
    pub fn new(ack: u16, req: u16) -> Self {
        Self { ack, req }
    }

    /// Initial flow control state - request up to 100 tx IDs
    pub fn initial() -> Self {
        Self::new(0, 100)
    }

    /// Update acknowledgment count after receiving transaction IDs
    pub fn update_after_receive(&mut self, received_count: u16) {
        self.ack = self.ack.wrapping_add(received_count);
    }

    /// Check if we should request more transaction IDs
    pub fn should_request_more(&self, current_count: u16) -> bool {
        current_count < self.req
    }

    /// Validate flow control numbers are consistent
    pub fn is_valid(&self) -> bool {
        self.req >= self.ack
    }
}

/// TxSubmission protocol messages (bidirectional)
#[derive(Debug, Clone)]
pub enum TxSubmissionMessage {
    /// Request transaction IDs from peer's mempool
    RequestTxIds { blocking: bool, ack: u16, req: u16 },
    /// Reply with available transaction IDs
    ReplyTxIds { tx_ids: Vec<(TxId, TxSize)> },
    /// Request specific transaction data
    RequestTxs { tx_ids: Vec<TxId> },
    /// Reply with transaction data
    ReplyTxs { txs: Vec<Transaction> },
}

impl PartialEq for TxSubmissionMessage {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                TxSubmissionMessage::RequestTxIds {
                    blocking: b1,
                    ack: a1,
                    req: r1,
                },
                TxSubmissionMessage::RequestTxIds {
                    blocking: b2,
                    ack: a2,
                    req: r2,
                },
            ) => b1 == b2 && a1 == a2 && r1 == r2,
            (
                TxSubmissionMessage::ReplyTxIds { tx_ids: ids1 },
                TxSubmissionMessage::ReplyTxIds { tx_ids: ids2 },
            ) => ids1 == ids2,
            (
                TxSubmissionMessage::RequestTxs { tx_ids: ids1 },
                TxSubmissionMessage::RequestTxs { tx_ids: ids2 },
            ) => ids1 == ids2,
            (
                TxSubmissionMessage::ReplyTxs { txs: txs1 },
                TxSubmissionMessage::ReplyTxs { txs: txs2 },
            ) => {
                // Compare transactions by their input/output counts (simplified)
                txs1.len() == txs2.len()
                    && txs1.iter().zip(txs2.iter()).all(|(tx1, tx2)| {
                        tx1.inputs.len() == tx2.inputs.len()
                            && tx1.outputs.len() == tx2.outputs.len()
                    })
            }
            _ => false,
        }
    }
}

/// TxSubmission protocol states
#[derive(Debug, Clone, PartialEq)]
pub enum TxSubmissionState {
    /// Idle state, ready for new requests
    Idle,
    /// Waiting for peer to reply with transaction IDs
    WaitingForTxIds,
    /// Waiting for peer to reply with transaction data
    WaitingForTxs,
}

/// TxSubmission protocol errors
#[derive(Debug, Clone, PartialEq)]
pub enum TxSubmissionError {
    /// Invalid state transition
    InvalidState {
        current_state: TxSubmissionState,
        message: String,
    },
    /// Flow control violation
    FlowControlViolation {
        expected_ack: u16,
        received_ack: u16,
    },
    /// Empty transaction request
    EmptyRequest,
    /// Transaction not found in mempool
    TransactionNotFound { tx_id: TxId },
    /// Incorrect transaction count in reply
    IncorrectTxCount { expected: usize, received: usize },
    /// Invalid request parameters
    InvalidRequest { message: String },
    /// Network communication error
    NetworkError { message: String },
}

impl std::fmt::Display for TxSubmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxSubmissionError::InvalidState {
                current_state,
                message,
            } => {
                write!(f, "Invalid state {:?}: {}", current_state, message)
            }
            TxSubmissionError::FlowControlViolation {
                expected_ack,
                received_ack,
            } => {
                write!(
                    f,
                    "Flow control violation: expected ack {}, received {}",
                    expected_ack, received_ack
                )
            }
            TxSubmissionError::EmptyRequest => {
                write!(f, "Empty transaction request")
            }
            TxSubmissionError::TransactionNotFound { tx_id } => {
                write!(f, "Transaction not found: {:?}", tx_id.0)
            }
            TxSubmissionError::IncorrectTxCount { expected, received } => {
                write!(
                    f,
                    "Incorrect transaction count: expected {}, received {}",
                    expected, received
                )
            }
            TxSubmissionError::InvalidRequest { message } => {
                write!(f, "Invalid request: {}", message)
            }
            TxSubmissionError::NetworkError { message } => {
                write!(f, "Network error: {}", message)
            }
        }
    }
}

impl std::error::Error for TxSubmissionError {}

/// Mock mempool for managing transactions in the TxSubmission protocol
#[derive(Debug)]
pub struct MockMempool {
    /// Available transactions mapped by ID
    transactions: HashMap<TxId, Transaction>,
    /// Transaction sizes mapped by ID
    tx_sizes: HashMap<TxId, TxSize>,
    /// Set of known transaction IDs (including those we don't have full data for)
    known_tx_ids: HashSet<TxId>,
}

impl Default for MockMempool {
    fn default() -> Self {
        Self::new()
    }
}

impl MockMempool {
    /// Create a new empty mempool
    pub fn new() -> Self {
        Self {
            transactions: HashMap::new(),
            tx_sizes: HashMap::new(),
            known_tx_ids: HashSet::new(),
        }
    }

    /// Add a transaction to the mempool
    pub fn add_transaction(&mut self, tx_id: TxId, tx: Transaction, size: TxSize) {
        self.transactions.insert(tx_id.clone(), tx);
        self.tx_sizes.insert(tx_id.clone(), size);
        self.known_tx_ids.insert(tx_id);
    }

    /// Get a transaction by ID
    pub fn get_transaction(&self, tx_id: &TxId) -> Option<&Transaction> {
        self.transactions.get(tx_id)
    }

    /// Get transaction size by ID
    pub fn get_tx_size(&self, tx_id: &TxId) -> Option<TxSize> {
        self.tx_sizes.get(tx_id).copied()
    }

    /// Check if we have a transaction
    pub fn has_transaction(&self, tx_id: &TxId) -> bool {
        self.known_tx_ids.contains(tx_id)
    }

    /// Get all available transaction IDs and sizes
    pub fn get_available_tx_ids(&self) -> Vec<(TxId, TxSize)> {
        self.tx_sizes
            .iter()
            .map(|(id, &size)| (id.clone(), size))
            .collect()
    }

    /// Find transaction IDs we don't know about from a peer's list
    pub fn get_unknown_tx_ids(&self, peer_tx_ids: &[(TxId, TxSize)]) -> Vec<TxId> {
        peer_tx_ids
            .iter()
            .filter(|(id, _)| !self.known_tx_ids.contains(id))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Mark a transaction ID as known (even if we don't have the full data)
    pub fn mark_known(&mut self, tx_id: TxId) {
        self.known_tx_ids.insert(tx_id);
    }

    /// Get the number of transactions in the mempool
    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    /// Check if the mempool is empty
    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }
}

/// TxSubmission protocol participant (can act as both client and server)
#[derive(Debug)]
pub struct TxSubmissionNode {
    /// Current protocol state
    state: TxSubmissionState,
    /// Flow control parameters
    flow_control: FlowControl,
    /// Local mempool
    mempool: MockMempool,
    /// Pending transaction requests
    pending_tx_requests: Vec<TxId>,
    /// Recently received transaction IDs
    received_tx_ids: Vec<(TxId, TxSize)>,
    /// Message sender
    sender: mpsc::UnboundedSender<TxSubmissionMessage>,
}

impl TxSubmissionNode {
    /// Create a new TxSubmission protocol node
    pub fn new() -> (Self, mpsc::UnboundedReceiver<TxSubmissionMessage>) {
        let (sender, receiver) = mpsc::unbounded_channel();

        let node = Self {
            state: TxSubmissionState::Idle,
            flow_control: FlowControl::initial(),
            mempool: MockMempool::new(),
            pending_tx_requests: Vec::new(),
            received_tx_ids: Vec::new(),
            sender,
        };

        (node, receiver)
    }

    /// Request transaction IDs from peer
    pub async fn request_tx_ids(&mut self, blocking: bool) -> Result<(), TxSubmissionError> {
        match self.state {
            TxSubmissionState::Idle => {
                self.state = TxSubmissionState::WaitingForTxIds;
                let message = TxSubmissionMessage::RequestTxIds {
                    blocking,
                    ack: self.flow_control.ack,
                    req: self.flow_control.req,
                };

                self.sender
                    .send(message)
                    .map_err(|_| TxSubmissionError::NetworkError {
                        message: "Failed to send RequestTxIds".to_string(),
                    })?;

                Ok(())
            }
            _ => Err(TxSubmissionError::InvalidState {
                current_state: self.state.clone(),
                message: "Can only request tx IDs from Idle state".to_string(),
            }),
        }
    }

    /// Request specific transactions by ID
    pub async fn request_transactions(
        &mut self,
        tx_ids: Vec<TxId>,
    ) -> Result<(), TxSubmissionError> {
        match self.state {
            TxSubmissionState::Idle => {
                if tx_ids.is_empty() {
                    return Err(TxSubmissionError::EmptyRequest);
                }

                self.state = TxSubmissionState::WaitingForTxs;
                self.pending_tx_requests = tx_ids.clone();

                let message = TxSubmissionMessage::RequestTxs { tx_ids };
                self.sender
                    .send(message)
                    .map_err(|_| TxSubmissionError::NetworkError {
                        message: "Failed to send RequestTxs".to_string(),
                    })?;

                Ok(())
            }
            _ => Err(TxSubmissionError::InvalidState {
                current_state: self.state.clone(),
                message: "Can only request transactions from Idle state".to_string(),
            }),
        }
    }

    /// Process a message received from peer
    pub async fn receive_message(
        &mut self,
        message: TxSubmissionMessage,
    ) -> Result<(), TxSubmissionError> {
        match message {
            TxSubmissionMessage::RequestTxIds {
                blocking: _,
                ack,
                req,
            } => self.handle_tx_ids_request(ack, req).await,
            TxSubmissionMessage::ReplyTxIds { tx_ids } => self.handle_tx_ids_reply(tx_ids).await,
            TxSubmissionMessage::RequestTxs { tx_ids } => self.handle_txs_request(tx_ids).await,
            TxSubmissionMessage::ReplyTxs { txs } => self.handle_txs_reply(txs).await,
        }
    }

    /// Handle a request for transaction IDs
    async fn handle_tx_ids_request(&mut self, ack: u16, req: u16) -> Result<(), TxSubmissionError> {
        // Validate flow control
        let flow_control = FlowControl::new(ack, req);
        if !flow_control.is_valid() {
            return Err(TxSubmissionError::FlowControlViolation {
                expected_ack: req,
                received_ack: ack,
            });
        }

        // Get available transaction IDs (limited by req count)
        let available_tx_ids = self.mempool.get_available_tx_ids();
        let tx_ids_to_send = available_tx_ids.into_iter().take(req as usize).collect();

        let reply = TxSubmissionMessage::ReplyTxIds {
            tx_ids: tx_ids_to_send,
        };
        self.sender
            .send(reply)
            .map_err(|_| TxSubmissionError::NetworkError {
                message: "Failed to send ReplyTxIds".to_string(),
            })?;

        Ok(())
    }

    /// Handle a reply with transaction IDs
    async fn handle_tx_ids_reply(
        &mut self,
        tx_ids: Vec<(TxId, TxSize)>,
    ) -> Result<(), TxSubmissionError> {
        match self.state {
            TxSubmissionState::WaitingForTxIds => {
                // Update flow control
                self.flow_control.update_after_receive(tx_ids.len() as u16);

                // Store received transaction IDs
                self.received_tx_ids = tx_ids;

                // Return to idle state
                self.state = TxSubmissionState::Idle;
                Ok(())
            }
            _ => Err(TxSubmissionError::InvalidState {
                current_state: self.state.clone(),
                message: "Unexpected ReplyTxIds message".to_string(),
            }),
        }
    }

    /// Handle a request for specific transactions
    async fn handle_txs_request(&mut self, tx_ids: Vec<TxId>) -> Result<(), TxSubmissionError> {
        if tx_ids.is_empty() {
            return Err(TxSubmissionError::EmptyRequest);
        }

        let mut transactions = Vec::new();

        // Collect requested transactions
        for tx_id in &tx_ids {
            if let Some(tx) = self.mempool.get_transaction(tx_id) {
                transactions.push(tx.clone());
            } else {
                return Err(TxSubmissionError::TransactionNotFound {
                    tx_id: tx_id.clone(),
                });
            }
        }

        let reply = TxSubmissionMessage::ReplyTxs { txs: transactions };
        self.sender
            .send(reply)
            .map_err(|_| TxSubmissionError::NetworkError {
                message: "Failed to send ReplyTxs".to_string(),
            })?;

        Ok(())
    }

    /// Handle a reply with transaction data
    async fn handle_txs_reply(&mut self, txs: Vec<Transaction>) -> Result<(), TxSubmissionError> {
        match self.state {
            TxSubmissionState::WaitingForTxs => {
                // Validate transaction count
                if txs.len() != self.pending_tx_requests.len() {
                    return Err(TxSubmissionError::IncorrectTxCount {
                        expected: self.pending_tx_requests.len(),
                        received: txs.len(),
                    });
                }

                // Add received transactions to mempool
                for (tx_id, tx) in self.pending_tx_requests.iter().zip(txs.iter()) {
                    let size = calculate_tx_size(tx); // Simplified size calculation
                    self.mempool
                        .add_transaction(tx_id.clone(), tx.clone(), size);
                }

                // Clear pending requests and return to idle
                self.pending_tx_requests.clear();
                self.state = TxSubmissionState::Idle;
                Ok(())
            }
            _ => Err(TxSubmissionError::InvalidState {
                current_state: self.state.clone(),
                message: "Unexpected ReplyTxs message".to_string(),
            }),
        }
    }

    /// Add a transaction to the local mempool
    pub fn add_transaction(&mut self, tx_id: TxId, tx: Transaction, size: TxSize) {
        self.mempool.add_transaction(tx_id, tx, size);
    }

    /// Get the current state
    pub fn get_state(&self) -> &TxSubmissionState {
        &self.state
    }

    /// Get the flow control parameters
    pub fn get_flow_control(&self) -> &FlowControl {
        &self.flow_control
    }

    /// Get received transaction IDs from last request
    pub fn get_received_tx_ids(&self) -> &[(TxId, TxSize)] {
        &self.received_tx_ids
    }

    /// Get the mempool
    pub fn get_mempool(&self) -> &MockMempool {
        &self.mempool
    }

    /// Find unknown transaction IDs from a list
    pub fn find_unknown_tx_ids(&self, tx_ids: &[(TxId, TxSize)]) -> Vec<TxId> {
        self.mempool.get_unknown_tx_ids(tx_ids)
    }
}

/// Calculate transaction size (simplified for testing)
fn calculate_tx_size(tx: &Transaction) -> TxSize {
    // Use the size field from the transaction if available, otherwise calculate
    tx.size
}

/// Generate a mock transaction for testing
pub fn generate_mock_transaction(
    tx_id: &TxId,
    input_count: usize,
    output_count: usize,
) -> Transaction {
    use cardano_consensus::block_production::{TxInput, TxOutput};

    let inputs = (0..input_count)
        .map(|i| TxInput {
            tx_hash: tx_id.0,
            output_index: i as u32,
        })
        .collect();

    let outputs = (0..output_count)
        .map(|i| TxOutput {
            address: Blake2b256Hash::from_bytes(&[
                (i as u8),
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
                0u8,
            ])
            .unwrap(),
            value: (i + 1) as u64 * 1000000, // 1-n ADA
        })
        .collect();

    Transaction {
        tx_id: tx_id.0,
        inputs,
        outputs,
        fee: 200000, // 0.2 ADA fee
        size: calculate_tx_size_simple(input_count, output_count),
    }
}

/// Simple transaction size calculation
fn calculate_tx_size_simple(input_count: usize, output_count: usize) -> u32 {
    let base_size = 100;
    let input_size = input_count as u32 * 50;
    let output_size = output_count as u32 * 40;
    base_size + input_size + output_size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tx_id_creation() {
        let tx_id = TxId::new(&[1u8; 32]).unwrap();
        assert_eq!(tx_id.hash().as_bytes()[0], 1);

        let random_tx_id = TxId::random(42);
        assert_eq!(random_tx_id.hash().as_bytes()[0], 42);
    }

    #[test]
    fn test_flow_control() {
        let mut flow_control = FlowControl::initial();
        assert_eq!(flow_control.ack, 0);
        assert_eq!(flow_control.req, 100);
        assert!(flow_control.is_valid());

        flow_control.update_after_receive(10);
        assert_eq!(flow_control.ack, 10);
        assert!(flow_control.should_request_more(50));
        assert!(!flow_control.should_request_more(150));
    }

    #[test]
    fn test_mock_mempool() {
        let mut mempool = MockMempool::new();
        let tx_id = TxId::random(1);
        let tx = generate_mock_transaction(&tx_id, 2, 1);

        assert!(mempool.is_empty());
        mempool.add_transaction(tx_id.clone(), tx, 300);
        assert_eq!(mempool.len(), 1);
        assert!(mempool.has_transaction(&tx_id));

        let available = mempool.get_available_tx_ids();
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].0, tx_id);
        assert_eq!(available[0].1, 300);
    }

    #[test]
    fn test_txsubmission_node_creation() {
        let (node, _receiver) = TxSubmissionNode::new();
        assert_eq!(node.get_state(), &TxSubmissionState::Idle);
        assert_eq!(node.get_flow_control().ack, 0);
        assert_eq!(node.get_flow_control().req, 100);
        assert!(node.get_mempool().is_empty());
    }

    #[tokio::test]
    async fn test_invalid_empty_request() {
        let (mut node, _receiver) = TxSubmissionNode::new();

        let result = node.request_transactions(vec![]).await;
        assert_eq!(result, Err(TxSubmissionError::EmptyRequest));
    }

    #[test]
    fn test_mock_transaction_generation() {
        let tx_id = TxId::random(5);
        let tx = generate_mock_transaction(&tx_id, 2, 3);
        assert_eq!(tx.inputs.len(), 2);
        assert_eq!(tx.outputs.len(), 3);
        assert_eq!(tx.fee, 200000);
    }

    #[test]
    fn test_tx_size_calculation() {
        let tx_id = TxId::random(10);
        let tx = generate_mock_transaction(&tx_id, 3, 2);
        let size = calculate_tx_size(&tx);
        // Should match the size field that was set during transaction creation
        let expected_size = calculate_tx_size_simple(3, 2); // 100 + 150 + 80 = 330
        assert_eq!(size, expected_size);
    }
}
