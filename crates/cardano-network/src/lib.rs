//! Cardano P2P Networking and Protocols
//!
//! Implements Cardano's networking protocols for peer-to-peer communication,
//! including connection management, protocol multiplexing, and peer selection.
//!
//! ## Main Components
//!
//! - **Protocols**: Implementation of Cardano mini-protocols (ChainSync, BlockFetch, TxSubmission)
//! - **Diffusion**: Peer selection, reputation management, and gossip protocols
//! - **Connection**: Connection lifecycle management, multiplexing, and health monitoring
//!
//! ## Usage
//!
//! ```rust
//! use cardano_network::{
//!     connection::{ConnectionManager, ConnectionConfig},
//!     diffusion::{PeerSelector, SelectionConfig},
//! };
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create connection manager
//! let config = ConnectionConfig::default();
//! let mut manager = ConnectionManager::new(config).await?;
//!
//! // Start connection management
//! manager.start().await?;
//!
//! // Connect to peers
//! let peer_addr = "127.0.0.1:3001".parse()?;
//! manager.connect_to_peer(peer_addr).await?;
//! # Ok(())
//! # }
//! ```

pub mod connection;
pub mod diffusion;
pub mod discovery;
pub mod protocols;
pub mod services;
pub mod topology;

/// Network error types
#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Connection error: {0}")]
    ConnectionError(#[from] connection::ConnectionError),

    #[error("Peer selection error: {0}")]
    PeerSelectionError(String),

    #[error("Handshake error: {0}")]
    HandshakeError(#[from] protocols::handshake::HandshakeError),

    #[error("Multiplexer error: {0}")]
    MultiplexerError(#[from] connection::multiplexer::MultiplexerError),

    #[error("Monitor error: {0}")]
    MonitorError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Encoding error: {0}")]
    EncodingError(String),
}

pub type Result<T> = std::result::Result<T, NetworkError>;

// Re-export commonly used types for convenience
pub use connection::{
    ConnectionConfig, ConnectionEvent, ConnectionId, ConnectionManager, ConnectionState, ProtocolId,
};
pub use diffusion::{PeerId, PeerInfo, PeerSelector, SelectionConfig};
pub use protocols::{blockfetch, chainsync, txsubmission};
