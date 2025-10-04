//! Cardano Network Protocols
//!
//! Implementations of the Cardano P2P mini-protocols for blockchain synchronization,
//! block fetching, transaction submission, and peer management.

pub mod blockfetch;
pub mod chainsync;
pub mod handshake;
pub mod txsubmission;

// Re-export commonly used handshake types
pub use handshake::{
    HandshakeClient, HandshakeError, HandshakeProtocolHandler, HandshakeResult, HandshakeState,
    NetworkMagic, NodeToNodeVersion, NodeToNodeVersionData,
};

// TODO: Implement in subsequent phases
// pub mod keepalive;
