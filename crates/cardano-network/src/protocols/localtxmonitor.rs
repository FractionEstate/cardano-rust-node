//! Local Transaction Monitor Protocol (node-to-client, protocol #9)
//!
//! Allows clients to monitor the node's mempool for transaction status changes.
//!
//! ## Protocol Overview
//!
//! This protocol enables clients to:
//! - Acquire a snapshot of the mempool
//! - Query for specific transactions
//! - Monitor when transactions are added/removed
//! - Track transaction lifecycle (mempool → block)
//!
//! ## State Machine
//!
//! ```text
//! ┌────────┐
//! │  Idle  │◀──────────────────────────┐
//! └────┬───┘                            │
//!      │ MsgAcquire                    │
//!      ▼                                │
//! ┌──────────┐   MsgReleased            │
//! │ Acquired ├──────────────────────────┘
//! └──────────┤
//!            │   MsgNextTx / MsgHasTx
//!            └─────────┐
//!                      │
//!            ┌─────────▼
//!            │  Query
//!            └──────────┤
//!                       │ MsgReply*
//!                       ▼
//!            ┌──────────────┐
//!            │   Acquired   │
//!            └──────────────┘
//! ```
//!
//! ## Messages
//!
//! - **MsgAcquire**: Client acquires a mempool snapshot
//! - **MsgAcquired**: Node confirms acquisition with snapshot slot
//! - **MsgRelease**: Client releases current snapshot
//! - **MsgNextTx**: Client requests next transaction in snapshot
//! - **MsgReplyNextTx**: Node returns next transaction (or None if end)
//! - **MsgHasTx**: Client queries if specific tx is in mempool
//! - **MsgReplyHasTx**: Node responds with Yes/No
//! - **MsgGetSizes**: Client requests mempool size stats
//! - **MsgReplyGetSizes**: Node returns mempool capacity/size/txs
//! - **MsgDone**: Either party terminates the protocol
//!
//! ## Use Cases
//!
//! 1. **Wallet monitoring**: Track submitted transaction until confirmed
//! 2. **Explorer indexing**: Monitor all mempool activity
//! 3. **MEV monitoring**: Watch for specific transaction patterns
//! 4. **Network analytics**: Analyze mempool pressure and propagation
//!
//! ## Reference
//!
//! Based on IntersectMBO/ouroboros-network LocalTxMonitor module

use crate::{NetworkError, Result};
use bytes::Bytes;
use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Protocol number for LocalTxMonitor (node-to-client)
pub const PROTOCOL_NUM: u16 = 9;

/// LocalTxMonitor protocol states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    /// Protocol idle, no snapshot acquired
    Idle,
    /// Snapshot acquired, ready for queries
    Acquired,
    /// Protocol terminated
    Done,
}

/// LocalTxMonitor protocol messages
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub enum Message {
    /// Client acquires a mempool snapshot (client -> node)
    #[n(0)]
    MsgAcquire,

    /// Node confirms acquisition with slot number (node -> client)
    #[n(1)]
    MsgAcquired {
        #[n(0)]
        slot: u64,
    },

    /// Client releases the current snapshot (client -> node)
    #[n(2)]
    MsgRelease,

    /// Node confirms release (node -> client)
    #[n(3)]
    MsgReleased,

    /// Client requests next transaction in snapshot (client -> node)
    #[n(4)]
    MsgNextTx,

    /// Node returns next transaction or signals end (node -> client)
    #[n(5)]
    MsgReplyNextTx {
        #[n(0)]
        tx: Option<Transaction>,
    },

    /// Client checks if specific transaction is in mempool (client -> node)
    #[n(6)]
    MsgHasTx {
        #[n(0)]
        tx_id: TxId,
    },

    /// Node responds whether transaction exists (node -> client)
    #[n(7)]
    MsgReplyHasTx {
        #[n(0)]
        has: bool,
    },

    /// Client requests mempool size statistics (client -> node)
    #[n(8)]
    MsgGetSizes,

    /// Node returns mempool statistics (node -> client)
    #[n(9)]
    MsgReplyGetSizes {
        #[n(0)]
        sizes: MempoolSizes,
    },

    /// Either party terminates the protocol
    #[n(10)]
    MsgDone,
}

/// Transaction representation (simplified)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub struct Transaction {
    /// Transaction ID (hash)
    #[n(0)]
    pub id: TxId,

    /// Transaction era (Byron, Shelley, etc.)
    #[n(1)]
    pub era: TxEra,

    /// CBOR-encoded transaction bytes
    #[n(2)]
    #[cbor(with = "minicbor::bytes")]
    pub bytes: Vec<u8>,

    /// Transaction size in bytes
    #[n(3)]
    pub size: u32,
}

/// Transaction ID (32-byte Blake2b-256 hash)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Encode, Decode)]
pub struct TxId {
    #[n(0)]
    #[cbor(with = "minicbor::bytes")]
    pub hash: Vec<u8>,
}

/// Transaction era
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

/// Mempool size statistics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub struct MempoolSizes {
    /// Maximum mempool capacity in bytes
    #[n(0)]
    pub capacity: u32,

    /// Current mempool size in bytes
    #[n(1)]
    pub size: u32,

    /// Number of transactions in mempool
    #[n(2)]
    pub num_txs: u32,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            State::Idle => write!(f, "Idle"),
            State::Acquired => write!(f, "Acquired"),
            State::Done => write!(f, "Done"),
        }
    }
}

impl fmt::Display for TxId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.hash))
    }
}

impl fmt::Display for TxEra {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TxEra::Byron => write!(f, "Byron"),
            TxEra::Shelley => write!(f, "Shelley"),
            TxEra::Allegra => write!(f, "Allegra"),
            TxEra::Mary => write!(f, "Mary"),
            TxEra::Alonzo => write!(f, "Alonzo"),
            TxEra::Babbage => write!(f, "Babbage"),
            TxEra::Conway => write!(f, "Conway"),
        }
    }
}

/// LocalTxMonitor protocol handler
pub struct LocalTxMonitor {
    state: State,
}

impl LocalTxMonitor {
    /// Create a new LocalTxMonitor protocol instance
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
            // Idle -> Acquired
            (State::Idle, Message::MsgAcquire) => {
                self.state = State::Acquired;
                Ok(Some(Message::MsgAcquired { slot: 0 }))
            }
            // Acquired -> Idle
            (State::Acquired, Message::MsgRelease) => {
                self.state = State::Idle;
                Ok(Some(Message::MsgReleased))
            }
            // Acquired -> Query NextTx
            (State::Acquired, Message::MsgNextTx) => Ok(Some(Message::MsgReplyNextTx { tx: None })),
            // Acquired -> Query HasTx
            (State::Acquired, Message::MsgHasTx { .. }) => {
                Ok(Some(Message::MsgReplyHasTx { has: false }))
            }
            // Acquired -> Query GetSizes
            (State::Acquired, Message::MsgGetSizes) => {
                Ok(Some(Message::MsgReplyGetSizes {
                    sizes: MempoolSizes {
                        capacity: 5_242_880, // 5 MB default
                        size: 0,
                        num_txs: 0,
                    },
                }))
            }
            // Handle server responses
            (State::Acquired, Message::MsgAcquired { .. }) => {
                self.state = State::Acquired;
                Ok(None)
            }
            (State::Idle, Message::MsgReleased) => {
                self.state = State::Idle;
                Ok(None)
            }
            (State::Acquired, Message::MsgReplyNextTx { .. }) => Ok(None),
            (State::Acquired, Message::MsgReplyHasTx { .. }) => Ok(None),
            (State::Acquired, Message::MsgReplyGetSizes { .. }) => Ok(None),
            // Any state -> Done
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

impl Default for LocalTxMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acquire_release_flow() {
        let mut protocol = LocalTxMonitor::new();
        assert_eq!(protocol.state(), State::Idle);

        // Acquire snapshot
        let response = protocol.handle_message(Message::MsgAcquire).unwrap();
        assert_eq!(protocol.state(), State::Acquired);
        assert!(matches!(response, Some(Message::MsgAcquired { .. })));

        // Release snapshot
        let response = protocol.handle_message(Message::MsgRelease).unwrap();
        assert_eq!(protocol.state(), State::Idle);
        assert_eq!(response, Some(Message::MsgReleased));
    }

    #[test]
    fn test_next_tx_query() {
        let mut protocol = LocalTxMonitor::new();

        // Acquire first
        protocol.handle_message(Message::MsgAcquire).unwrap();
        assert_eq!(protocol.state(), State::Acquired);

        // Query next tx
        let response = protocol.handle_message(Message::MsgNextTx).unwrap();
        assert!(matches!(
            response,
            Some(Message::MsgReplyNextTx { tx: None })
        ));
        assert_eq!(protocol.state(), State::Acquired);
    }

    #[test]
    fn test_has_tx_query() {
        let mut protocol = LocalTxMonitor::new();

        // Acquire first
        protocol.handle_message(Message::MsgAcquire).unwrap();
        assert_eq!(protocol.state(), State::Acquired);

        // Query has tx
        let tx_id = TxId {
            hash: vec![0xaa; 32],
        };
        let response = protocol
            .handle_message(Message::MsgHasTx { tx_id })
            .unwrap();
        assert_eq!(response, Some(Message::MsgReplyHasTx { has: false }));
    }

    #[test]
    fn test_get_sizes_query() {
        let mut protocol = LocalTxMonitor::new();

        // Acquire first
        protocol.handle_message(Message::MsgAcquire).unwrap();

        // Query sizes
        let response = protocol.handle_message(Message::MsgGetSizes).unwrap();
        assert!(matches!(response, Some(Message::MsgReplyGetSizes { .. })));
    }

    #[test]
    fn test_done_from_acquired() {
        let mut protocol = LocalTxMonitor::new();
        protocol.handle_message(Message::MsgAcquire).unwrap();
        assert_eq!(protocol.state(), State::Acquired);

        protocol.handle_message(Message::MsgDone).unwrap();
        assert_eq!(protocol.state(), State::Done);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let msg = Message::MsgHasTx {
            tx_id: TxId {
                hash: vec![0xde, 0xad, 0xbe, 0xef],
            },
        };

        let encoded = LocalTxMonitor::encode_message(&msg).unwrap();
        let decoded = LocalTxMonitor::decode_message(&encoded).unwrap();

        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_tx_id_display() {
        let tx_id = TxId {
            hash: vec![0xaa, 0xbb, 0xcc, 0xdd],
        };
        assert_eq!(tx_id.to_string(), "aabbccdd");
    }

    #[test]
    fn test_mempool_sizes() {
        let sizes = MempoolSizes {
            capacity: 5_242_880,
            size: 1_048_576,
            num_txs: 42,
        };
        assert_eq!(sizes.capacity, 5_242_880);
        assert_eq!(sizes.size, 1_048_576);
        assert_eq!(sizes.num_txs, 42);
    }
}
