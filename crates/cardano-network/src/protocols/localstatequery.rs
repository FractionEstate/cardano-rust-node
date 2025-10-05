//! Local State Query Protocol (node-to-client, protocol #7)
//!
//! Allows clients (wallets, explorers, dApps) to query the current ledger state.
//!
//! ## Protocol Overview
//!
//! This protocol enables clients to query various aspects of the ledger state:
//! - Current protocol parameters
//! - UTxO sets for addresses
//! - Stake pool information
//! - Epoch information
//! - Current tip
//!
//! ## State Machine
//!
//! ```text
//! ┌────────┐
//! │  Idle  │◀──────────────────────────┐
//! └────┬───┘                            │
//!      │ MsgAcquire                    │
//!      ▼                                │
//! ┌──────────┐   MsgResult              │
//! │ Acquired ├──────────────────────────┘
//! └──────────┤
//!            │   MsgReAcquire
//!            └─────────┐
//!                      │
//!            ┌─────────▼
//!            │ Acquiring
//!            └──────────┤
//!                       │ MsgAcquired
//!                       ▼
//!            ┌──────────────┐
//!            │   Acquired   │
//!            └──────────────┘
//! ```
//!
//! ## Messages
//!
//! - **MsgAcquire**: Client acquires a specific ledger point
//! - **MsgAcquired**: Node confirms acquisition (or fails)
//! - **MsgQuery**: Client sends a ledger query
//! - **MsgResult**: Node returns query result
//! - **MsgRelease**: Client releases current point
//! - **MsgReAcquire**: Client acquires a different point without releasing
//! - **MsgDone**: Either party terminates the protocol
//!
//! ## Queries
//!
//! Supported query types (as per Cardano ledger spec):
//! - **GetEpochNo**: Current epoch number
//! - **GetCurrentPParams**: Current protocol parameters
//! - **GetProposedPParamsUpdates**: Proposed parameter updates
//! - **GetStakeDistribution**: Current stake distribution
//! - **GetUTxOByAddress**: UTxO entries for specific addresses
//! - **GetUTxOWhole**: Entire UTxO set (expensive!)
//! - **GetCurrentEpochState**: Full epoch state
//! - **GetCBOR**: Query result in raw CBOR
//! - **GetFilteredDelegationsAndRewardAccounts**: Stake delegation info
//! - **GetGenesisConfig**: Genesis configuration
//! - **GetSystemStart**: System start time
//! - **GetChainBlockNo**: Current block number
//! - **GetChainPoint**: Current chain tip point
//!
//! ## Reference
//!
//! Based on IntersectMBO/ouroboros-network LocalStateQuery module

use crate::{NetworkError, Result};
use bytes::Bytes;
use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Protocol number for LocalStateQuery (node-to-client)
pub const PROTOCOL_NUM: u16 = 7;

/// LocalStateQuery protocol states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    /// Protocol idle, no point acquired
    Idle,
    /// Point acquisition in progress
    Acquiring,
    /// Point acquired, ready for queries
    Acquired,
    /// Protocol terminated
    Done,
}

/// LocalStateQuery protocol messages
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub enum Message {
    /// Client acquires a specific point (client -> node)
    #[n(0)]
    MsgAcquire {
        #[n(0)]
        point: Point,
    },

    /// Node confirms acquisition (node -> client)
    #[n(1)]
    MsgAcquired,

    /// Node reports acquisition failure (node -> client)
    #[n(2)]
    MsgFailure {
        #[n(0)]
        reason: AcquireFailure,
    },

    /// Client sends a query (client -> node)
    #[n(3)]
    MsgQuery {
        #[n(0)]
        query: Query,
    },

    /// Node sends query result (node -> client)
    #[n(4)]
    MsgResult {
        #[n(0)]
        #[cbor(with = "minicbor::bytes")]
        result: Vec<u8>,
    },

    /// Client releases the current point (client -> node)
    #[n(5)]
    MsgRelease,

    /// Client re-acquires a different point without releasing (client -> node)
    #[n(6)]
    MsgReAcquire {
        #[n(0)]
        point: Point,
    },

    /// Either party terminates the protocol
    #[n(7)]
    MsgDone,
}

/// Cardano blockchain point (slot + hash)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub enum Point {
    /// Genesis point (origin of the chain)
    #[n(0)]
    Origin,

    /// Specific block point
    #[n(1)]
    Point {
        #[n(0)]
        slot: u64,
        #[n(1)]
        #[cbor(with = "minicbor::bytes")]
        hash: Vec<u8>,
    },
}

/// Reasons for acquisition failure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub enum AcquireFailure {
    /// Requested point is too old (beyond volatile tip rollback window)
    #[n(0)]
    PointTooOld,

    /// Requested point is not on the current chain
    #[n(1)]
    PointNotOnChain,
}

/// Ledger state queries
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub enum Query {
    /// Get current epoch number
    #[n(0)]
    GetEpochNo,

    /// Get current protocol parameters
    #[n(1)]
    GetCurrentPParams,

    /// Get proposed protocol parameter updates
    #[n(2)]
    GetProposedPParamsUpdates,

    /// Get current stake distribution
    #[n(3)]
    GetStakeDistribution,

    /// Get UTxO by address
    #[n(4)]
    GetUTxOByAddress {
        #[n(0)]
        addresses: Vec<Address>,
    },

    /// Get entire UTxO set (WARNING: expensive!)
    #[n(5)]
    GetUTxOWhole,

    /// Get current epoch state
    #[n(6)]
    GetCurrentEpochState,

    /// Get genesis configuration
    #[n(7)]
    GetGenesisConfig,

    /// Get system start time
    #[n(8)]
    GetSystemStart,

    /// Get current block number
    #[n(9)]
    GetChainBlockNo,

    /// Get current chain tip point
    #[n(10)]
    GetChainPoint,

    /// Get filtered delegations and reward accounts
    #[n(11)]
    GetFilteredDelegationsAndRewardAccounts {
        #[n(0)]
        addresses: Vec<Address>,
    },

    /// Get stake pools
    #[n(12)]
    GetStakePools,

    /// Get stake pool parameters
    #[n(13)]
    GetStakePoolParams {
        #[n(0)]
        pool_ids: Vec<PoolId>,
    },

    /// Get reward provenance (detailed reward calculation info)
    #[n(14)]
    GetRewardProvenance,

    /// Generic query returning raw CBOR
    #[n(15)]
    GetCBOR {
        #[n(0)]
        #[cbor(with = "minicbor::bytes")]
        query_cbor: Vec<u8>,
    },
}

/// Cardano address (simplified representation)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub struct Address {
    #[n(0)]
    #[cbor(with = "minicbor::bytes")]
    pub bytes: Vec<u8>,
}

/// Stake pool ID
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub struct PoolId {
    #[n(0)]
    #[cbor(with = "minicbor::bytes")]
    pub bytes: Vec<u8>,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            State::Idle => write!(f, "Idle"),
            State::Acquiring => write!(f, "Acquiring"),
            State::Acquired => write!(f, "Acquired"),
            State::Done => write!(f, "Done"),
        }
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Point::Origin => write!(f, "Origin"),
            Point::Point { slot, hash } => {
                write!(
                    f,
                    "Point(slot={}, hash={})",
                    slot,
                    hex::encode(&hash[..8.min(hash.len())])
                )
            }
        }
    }
}

impl fmt::Display for AcquireFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AcquireFailure::PointTooOld => write!(f, "Point too old"),
            AcquireFailure::PointNotOnChain => write!(f, "Point not on chain"),
        }
    }
}

/// LocalStateQuery protocol handler
pub struct LocalStateQuery {
    state: State,
}

impl LocalStateQuery {
    /// Create a new LocalStateQuery protocol instance
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
            // Idle -> Acquiring
            (State::Idle, Message::MsgAcquire { .. }) => {
                self.state = State::Acquiring;
                Ok(Some(Message::MsgAcquired))
            }
            // Acquiring -> Acquired (success)
            (State::Acquiring, Message::MsgAcquired) => {
                self.state = State::Acquired;
                Ok(None)
            }
            // Acquiring -> Idle (failure)
            (State::Acquiring, Message::MsgFailure { .. }) => {
                self.state = State::Idle;
                Ok(None)
            }
            // Acquired -> Query
            (State::Acquired, Message::MsgQuery { .. }) => {
                // Stay in Acquired state, return mock result
                Ok(Some(Message::MsgResult {
                    result: vec![0x00], // Placeholder
                }))
            }
            // Acquired -> Result received
            (State::Acquired, Message::MsgResult { .. }) => {
                // Stay in Acquired state
                Ok(None)
            }
            // Acquired -> Idle
            (State::Acquired, Message::MsgRelease) => {
                self.state = State::Idle;
                Ok(None)
            }
            // Acquired -> Acquiring (re-acquire)
            (State::Acquired, Message::MsgReAcquire { .. }) => {
                self.state = State::Acquiring;
                Ok(Some(Message::MsgAcquired))
            }
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

impl Default for LocalStateQuery {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acquire_flow() {
        let mut protocol = LocalStateQuery::new();
        assert_eq!(protocol.state(), State::Idle);

        let acquire_msg = Message::MsgAcquire {
            point: Point::Point {
                slot: 1000,
                hash: vec![0xaa; 32],
            },
        };

        let response = protocol.handle_message(acquire_msg).unwrap();
        assert_eq!(protocol.state(), State::Acquiring);
        assert_eq!(response, Some(Message::MsgAcquired));

        protocol.handle_message(Message::MsgAcquired).unwrap();
        assert_eq!(protocol.state(), State::Acquired);
    }

    #[test]
    fn test_acquire_failure() {
        let mut protocol = LocalStateQuery::new();

        let acquire_msg = Message::MsgAcquire {
            point: Point::Point {
                slot: 999,
                hash: vec![0xbb; 32],
            },
        };

        protocol.handle_message(acquire_msg).unwrap();
        assert_eq!(protocol.state(), State::Acquiring);

        let failure_msg = Message::MsgFailure {
            reason: AcquireFailure::PointTooOld,
        };

        protocol.handle_message(failure_msg).unwrap();
        assert_eq!(protocol.state(), State::Idle);
    }

    #[test]
    fn test_query_flow() {
        let mut protocol = LocalStateQuery::new();

        // Acquire point first
        protocol
            .handle_message(Message::MsgAcquire {
                point: Point::Origin,
            })
            .unwrap();
        protocol.handle_message(Message::MsgAcquired).unwrap();
        assert_eq!(protocol.state(), State::Acquired);

        // Send query
        let query_msg = Message::MsgQuery {
            query: Query::GetEpochNo,
        };

        let response = protocol.handle_message(query_msg).unwrap();
        assert!(matches!(response, Some(Message::MsgResult { .. })));
        assert_eq!(protocol.state(), State::Acquired);
    }

    #[test]
    fn test_release_flow() {
        let mut protocol = LocalStateQuery::new();

        // Acquire and release
        protocol
            .handle_message(Message::MsgAcquire {
                point: Point::Origin,
            })
            .unwrap();
        protocol.handle_message(Message::MsgAcquired).unwrap();
        assert_eq!(protocol.state(), State::Acquired);

        protocol.handle_message(Message::MsgRelease).unwrap();
        assert_eq!(protocol.state(), State::Idle);
    }

    #[test]
    fn test_reacquire_flow() {
        let mut protocol = LocalStateQuery::new();

        // Acquire first point
        protocol
            .handle_message(Message::MsgAcquire {
                point: Point::Origin,
            })
            .unwrap();
        protocol.handle_message(Message::MsgAcquired).unwrap();
        assert_eq!(protocol.state(), State::Acquired);

        // Re-acquire different point
        let reacquire_msg = Message::MsgReAcquire {
            point: Point::Point {
                slot: 2000,
                hash: vec![0xcc; 32],
            },
        };

        let response = protocol.handle_message(reacquire_msg).unwrap();
        assert_eq!(protocol.state(), State::Acquiring);
        assert_eq!(response, Some(Message::MsgAcquired));
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let msg = Message::MsgQuery {
            query: Query::GetCurrentPParams,
        };

        let encoded = LocalStateQuery::encode_message(&msg).unwrap();
        let decoded = LocalStateQuery::decode_message(&encoded).unwrap();

        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_point_display() {
        let origin = Point::Origin;
        assert_eq!(origin.to_string(), "Origin");

        let point = Point::Point {
            slot: 12345,
            hash: vec![0xde, 0xad, 0xbe, 0xef],
        };
        assert!(point.to_string().contains("12345"));
    }
}
