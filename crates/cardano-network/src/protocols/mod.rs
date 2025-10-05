//! Cardano Network Protocols
//!
//! Implementations of the Cardano P2P mini-protocols for blockchain synchronization,
//! block fetching, transaction submission, and peer management.

// Node-to-Node protocols
pub mod blockfetch;
pub mod chainsync;
pub mod handshake;
pub mod keepalive;
pub mod peersharing;
pub mod txsubmission;

// Node-to-Client protocols
pub mod localstatequery;
pub mod localtxmonitor;
pub mod localtxsubmission;

// Re-export commonly used handshake types
pub use handshake::{
    HandshakeClient, HandshakeError, HandshakeProtocolHandler, HandshakeResult, HandshakeState,
    NetworkMagic, NodeToNodeVersion, NodeToNodeVersionData,
};
