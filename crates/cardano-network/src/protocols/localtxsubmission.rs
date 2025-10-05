//! Local Transaction Submission Protocol (node-to-client, protocol #6)
//!
//! Allows clients (wallets, CLI) to submit transactions to a node's mempool.
//!
//! ## Protocol Overview
//!
//! This is a simple request-response protocol where the client submits a transaction
//! and the node responds with either acceptance or rejection.
//!
//! ## State Machine
//!
//! ```text
//! ┌────────┐
//! │  Idle  │◀──────────────────┐
//! └────┬───┘                   │
//!      │ MsgSubmitTx          │
//!      ▼                       │
//! ┌──────────┐   MsgAcceptTx  │
//! │ Busy     ├────────────────┘
//! └──────────┤
//!            │   MsgRejectTx
//!            └────────────────┐
//!                             │
//! ┌────────┐   MsgDone        │
//! │  Done  │◀─────────────────┘
//! └────────┘
//! ```
//!
//! ## Messages
//!
//! - **MsgSubmitTx**: Client submits a serialized transaction
//! - **MsgAcceptTx**: Node accepts the transaction into mempool
//! - **MsgRejectTx**: Node rejects with reason (validation error, fee too low, etc.)
//! - **MsgDone**: Client/Node terminates the protocol
//!
//! ## Reference
//!
//! Based on IntersectMBO/ouroboros-network LocalTxSubmission module

use crate::{NetworkError, Result};
use bytes::Bytes;
use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Protocol number for LocalTxSubmission (node-to-client)
pub const PROTOCOL_NUM: u16 = 6;

/// LocalTxSubmission protocol states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Idle, ready to accept submissions
    Idle,
    /// Processing a transaction submission
    Busy,
    /// Protocol terminated
    Done,
}

/// LocalTxSubmission protocol messages
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub enum Message {
    /// Client submits a transaction to the mempool
    ///
    /// Payload: serialized CBOR transaction bytes
    #[n(0)]
    MsgSubmitTx {
        #[n(0)]
        era: TxEra,
        #[n(1)]
        #[cbor(with = "minicbor::bytes")]
        tx_bytes: Vec<u8>,
    },

    /// Node accepts the transaction
    #[n(1)]
    MsgAcceptTx,

    /// Node rejects the transaction with a reason
    #[n(2)]
    MsgRejectTx {
        #[n(0)]
        reason: RejectReason,
    },

    /// Terminate the protocol
    #[n(3)]
    MsgDone,
}

/// Transaction era identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub enum TxEra {
    #[n(0)]
    Byron,
    #[n(1)]
    Shelley,
    #[n(2)]
    Allegra,
    #[n(3)]
    Mary,
    #[n(4)]
    Alonzo,
    #[n(5)]
    Babbage,
    #[n(6)]
    Conway,
}

/// Rejection reasons for transactions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub enum RejectReason {
    /// Transaction deserialization failed
    #[n(0)]
    DeserializationError {
        #[n(0)]
        error: String,
    },

    /// Transaction validation failed
    #[n(1)]
    ValidationError {
        #[n(0)]
        error: String,
    },

    /// Insufficient transaction fee
    #[n(2)]
    InsufficientFee {
        #[n(0)]
        required: u64,
        #[n(1)]
        provided: u64,
    },

    /// Missing input UTxOs
    #[n(3)]
    MissingUTxO {
        #[n(0)]
        tx_ids: Vec<String>,
    },

    /// Mempool is full
    #[n(4)]
    MempoolFull,

    /// Transaction already in mempool
    #[n(5)]
    AlreadyInMempool,

    /// Invalid transaction witness
    #[n(6)]
    InvalidWitness {
        #[n(0)]
        error: String,
    },

    /// Expired transaction (TTL exceeded)
    #[n(7)]
    Expired {
        #[n(0)]
        ttl: u64,
        #[n(1)]
        current_slot: u64,
    },

    /// Other error
    #[n(8)]
    Other {
        #[n(0)]
        error: String,
    },
}

impl fmt::Display for RejectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RejectReason::DeserializationError { error } => {
                write!(f, "Deserialization error: {}", error)
            }
            RejectReason::ValidationError { error } => {
                write!(f, "Validation error: {}", error)
            }
            RejectReason::InsufficientFee { required, provided } => {
                write!(
                    f,
                    "Insufficient fee: required {}, provided {}",
                    required, provided
                )
            }
            RejectReason::MissingUTxO { tx_ids } => {
                write!(f, "Missing UTxOs: {}", tx_ids.join(", "))
            }
            RejectReason::MempoolFull => write!(f, "Mempool is full"),
            RejectReason::AlreadyInMempool => write!(f, "Transaction already in mempool"),
            RejectReason::InvalidWitness { error } => {
                write!(f, "Invalid witness: {}", error)
            }
            RejectReason::Expired { ttl, current_slot } => {
                write!(
                    f,
                    "Transaction expired: TTL {}, current slot {}",
                    ttl, current_slot
                )
            }
            RejectReason::Other { error } => write!(f, "Error: {}", error),
        }
    }
}

/// LocalTxSubmission protocol handler
pub struct LocalTxSubmission {
    state: State,
}

impl LocalTxSubmission {
    /// Create a new LocalTxSubmission protocol instance
    pub fn new() -> Self {
        Self { state: State::Idle }
    }

    /// Get the current protocol state
    pub fn state(&self) -> State {
        self.state
    }

    /// Handle an incoming message
    pub fn handle_message(&mut self, msg: Message) -> Result<Option<Message>> {
        match (&self.state, &msg) {
            (State::Idle, Message::MsgSubmitTx { .. }) => {
                self.state = State::Busy;
                // In a real implementation, this would:
                // 1. Deserialize the transaction
                // 2. Validate the transaction
                // 3. Check mempool capacity
                // 4. Add to mempool if valid
                // For now, we'll accept all transactions
                Ok(Some(Message::MsgAcceptTx))
            }
            (State::Busy, Message::MsgAcceptTx) => {
                self.state = State::Idle;
                Ok(None)
            }
            (State::Busy, Message::MsgRejectTx { .. }) => {
                self.state = State::Idle;
                Ok(None)
            }
            (_, Message::MsgDone) => {
                self.state = State::Done;
                Ok(None)
            }
            _ => Err(NetworkError::ProtocolError(format!(
                "Invalid message {:?} in state {:?}",
                msg, self.state
            ))),
        }
    }

    /// Encode a message to bytes
    pub fn encode_message(msg: &Message) -> Result<Bytes> {
        let vec = minicbor::to_vec(msg)
            .map_err(|e| NetworkError::EncodingError(format!("CBOR encode error: {}", e)))?;
        Ok(Bytes::from(vec))
    }

    /// Decode a message from bytes
    pub fn decode_message(bytes: &[u8]) -> Result<Message> {
        minicbor::decode(bytes)
            .map_err(|e| NetworkError::ProtocolError(format!("CBOR decode error: {}", e)))
    }
}

impl Default for LocalTxSubmission {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_submit_accept_flow() {
        let mut protocol = LocalTxSubmission::new();
        assert_eq!(protocol.state(), State::Idle);

        let submit_msg = Message::MsgSubmitTx {
            era: TxEra::Babbage,
            tx_bytes: b"fake_tx_bytes".to_vec(),
        };

        let response = protocol.handle_message(submit_msg).unwrap();
        assert_eq!(protocol.state(), State::Busy);
        assert_eq!(response, Some(Message::MsgAcceptTx));

        protocol.handle_message(Message::MsgAcceptTx).unwrap();
        assert_eq!(protocol.state(), State::Idle);
    }

    #[test]
    fn test_submit_reject_flow() {
        let mut protocol = LocalTxSubmission::new();

        let submit_msg = Message::MsgSubmitTx {
            era: TxEra::Conway,
            tx_bytes: b"invalid_tx".to_vec(),
        };

        protocol.handle_message(submit_msg).unwrap();
        assert_eq!(protocol.state(), State::Busy);

        let reject_msg = Message::MsgRejectTx {
            reason: RejectReason::ValidationError {
                error: "Invalid signature".to_string(),
            },
        };

        protocol.handle_message(reject_msg).unwrap();
        assert_eq!(protocol.state(), State::Idle);
    }

    #[test]
    fn test_done_from_idle() {
        let mut protocol = LocalTxSubmission::new();
        protocol.handle_message(Message::MsgDone).unwrap();
        assert_eq!(protocol.state(), State::Done);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let msg = Message::MsgSubmitTx {
            era: TxEra::Alonzo,
            tx_bytes: b"test_transaction".to_vec(),
        };

        let encoded = LocalTxSubmission::encode_message(&msg).unwrap();
        let decoded = LocalTxSubmission::decode_message(&encoded).unwrap();

        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_reject_reason_display() {
        let reason = RejectReason::InsufficientFee {
            required: 1000000,
            provided: 500000,
        };
        assert_eq!(
            reason.to_string(),
            "Insufficient fee: required 1000000, provided 500000"
        );

        let reason = RejectReason::MempoolFull;
        assert_eq!(reason.to_string(), "Mempool is full");
    }
}
