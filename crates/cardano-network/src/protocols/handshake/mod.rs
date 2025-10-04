//! Handshake Protocol - Node-to-Node Version Negotiation
//!
//! Implements the Ouroboros handshake mini-protocol for negotiating protocol
//! versions and network parameters before other mini-protocols can communicate.
//!
//! ## Protocol Flow
//!
//! ```text
//! Client                                Server
//!   │                                      │
//!   ├─── MsgProposeVersions ───────────→  │
//!   │    {V_14: vdata, V_15: vdata}       │
//!   │                                      │
//!   │  ←─── MsgAcceptVersion ───────────┤
//!   │       V_15, vdata                   │
//!   │                                      │
//!   └─── (Handshake Complete) ───────────┘
//! ```
//!
//! ## References
//!
//! - [Handshake Protocol Type](https://ouroboros-network.cardano.intersectmbo.org/ouroboros-network-framework/Ouroboros-Network-Protocol-Handshake-Type.html)
//! - Cardano Network Specification, Section "Handshake Protocol"

pub mod codec;
pub mod handler;
pub mod messages;
pub mod state;
pub mod types;

pub use handler::HandshakeProtocolHandler;
pub use messages::{HandshakeMessage, RefuseReason};
pub use state::{HandshakeClient, HandshakeState};
pub use types::{
    DiffusionMode, NetworkMagic, NodeToNodeVersion, NodeToNodeVersionData, PeerSharing,
    VersionTable,
};

/// Handshake protocol errors
#[derive(Debug, thiserror::Error)]
pub enum HandshakeError {
    #[error("Version mismatch: no compatible versions (proposed: {proposed:?}, server_unknown: {unknown_tags:?})")]
    VersionMismatch {
        proposed: Vec<NodeToNodeVersion>,
        unknown_tags: Vec<i64>,
    },

    #[error("Handshake decode error for version {version:?}: {error}")]
    HandshakeDecodeError {
        version: NodeToNodeVersion,
        error: String,
    },

    #[error("Server refused version {version:?}: {reason}")]
    Refused {
        version: NodeToNodeVersion,
        reason: String,
    },

    #[error("Network magic mismatch: expected {expected}, got {received}")]
    NetworkMagicMismatch { expected: u32, received: u32 },

    #[error("Protocol violation: {0}")]
    ProtocolViolation(String),

    #[error("CBOR encoding error: {0}")]
    EncodingError(String),

    #[error("CBOR decoding error: {0}")]
    DecodingError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Unexpected message in state {state:?}: {message}")]
    UnexpectedMessage { state: String, message: String },

    #[error("Timeout waiting for handshake response")]
    Timeout,
}

/// Handshake result containing negotiated version and version data
#[derive(Debug, Clone, PartialEq)]
pub struct HandshakeResult {
    /// Negotiated protocol version
    pub version: NodeToNodeVersion,
    /// Negotiated version-specific data
    pub version_data: NodeToNodeVersionData,
}

impl HandshakeResult {
    /// Create a new handshake result
    pub fn new(version: NodeToNodeVersion, version_data: NodeToNodeVersionData) -> Self {
        Self {
            version,
            version_data,
        }
    }

    /// Get the network magic from version data
    pub fn network_magic(&self) -> NetworkMagic {
        self.version_data.network_magic
    }

    /// Check if this is a mainnet connection
    pub fn is_mainnet(&self) -> bool {
        self.version_data.network_magic == NetworkMagic::MAINNET
    }

    /// Check if this is a preview testnet connection
    pub fn is_preview_testnet(&self) -> bool {
        self.version_data.network_magic == NetworkMagic::PREVIEW_TESTNET
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_detection() {
        let mainnet_result =
            HandshakeResult::new(NodeToNodeVersion::V15, NodeToNodeVersionData::mainnet());
        assert!(mainnet_result.is_mainnet());
        assert!(!mainnet_result.is_preview_testnet());

        let testnet_result = HandshakeResult::new(
            NodeToNodeVersion::V15,
            NodeToNodeVersionData::preview_testnet(),
        );
        assert!(!testnet_result.is_mainnet());
        assert!(testnet_result.is_preview_testnet());
    }
}
