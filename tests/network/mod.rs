//! Network Protocol Stack Tests
//!
//! Integration tests for the Cardano network protocol stack, covering:
//! - ChainSync mini-protocol for blockchain synchronization
//! - BlockFetch mini-protocol for efficient block retrieval
//! - TxSubmission mini-protocol for transaction propagation
//! - P2P peer selection and network topology management

pub mod integration;
pub mod test_blockfetch;
pub mod test_chainsync;
pub mod test_peer_selection;
pub mod test_preview_discovery;
pub mod test_preview_network_sync;
pub mod test_txsubmission;

// Re-export all network protocol tests
pub use integration::*;
pub use test_blockfetch::*;
pub use test_chainsync::*;
pub use test_peer_selection::*;
pub use test_preview_discovery::*;
pub use test_txsubmission::*;
