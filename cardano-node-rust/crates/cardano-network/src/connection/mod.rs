//! Connection Management for Cardano P2P Network
//!
//! This module provides comprehensive connection management for the Cardano P2P network,
//! including connection lifecycle, protocol multiplexing, handshake negotiation, and
//! health monitoring.
//!
//! ## Architecture
//!
//! The connection management system consists of several key components:
//!
//! - **ConnectionManager**: Orchestrates peer connections and integrates with peer selection
//! - **ConnectionMultiplexer**: Handles multiple mini-protocols over single TCP connections
//! - **HandshakeProtocol**: Manages version negotiation and capability agreement
//! - **ConnectionMonitor**: Provides health monitoring and automatic reconnection
//!
//! ## Usage
//!
//! ```rust
//! use cardano_network::connection::{ConnectionManager, ConnectionConfig};
//!
//! let config = ConnectionConfig::default();
//! let mut manager = ConnectionManager::new(config).await?;
//!
//! // Start connection management
//! manager.start().await?;
//!
//! // Connect to a peer
//! let peer_addr = "127.0.0.1:3001".parse()?;
//! manager.connect_to_peer(peer_addr).await?;
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

use crate::diffusion::{PeerId, PeerInfo};
use crate::protocols::{chainsync, blockfetch, txsubmission};
use crate::{NetworkError, Result};

pub mod manager;
pub mod multiplexer;
pub mod handshake;
pub mod monitor;
pub mod state;

#[cfg(test)]
mod tests;

/// Re-exports for convenience
pub use manager::{ConnectionManager, ConnectionConfig};
pub use multiplexer::{ConnectionMultiplexer, ProtocolId, MultiplexerError};
pub use handshake::{HandshakeProtocol, VersionNegotiation, HandshakeError};
pub use monitor::{ConnectionMonitor, HealthStatus, MonitorConfig};
pub use state::{ConnectionState, ConnectionInfo, StateTransition};

/// Unique identifier for a network connection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(u64);

impl ConnectionId {
    /// Generate a new unique connection ID
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

/// Network connection representation
#[derive(Debug)]
pub struct Connection {
    /// Unique connection identifier
    pub id: ConnectionId,
    /// Connected peer information
    pub peer: PeerInfo,
    /// Network address of the peer
    pub address: SocketAddr,
    /// Current connection state
    pub state: ConnectionState,
    /// Connection establishment time
    pub established_at: Instant,
    /// Last activity timestamp
    pub last_activity: Instant,
    /// TCP stream handle
    pub stream: Option<TcpStream>,
    /// Active protocol multiplexer
    pub multiplexer: Option<ConnectionMultiplexer>,
}

impl Connection {
    /// Create a new connection instance
    pub fn new(peer: PeerInfo, address: SocketAddr) -> Self {
        Self {
            id: ConnectionId::new(),
            peer,
            address,
            state: ConnectionState::Disconnected,
            established_at: Instant::now(),
            last_activity: Instant::now(),
            stream: None,
            multiplexer: None,
        }
    }

    /// Check if connection is active
    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            ConnectionState::Connected | ConnectionState::Authenticated
        )
    }

    /// Update last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Get connection uptime
    pub fn uptime(&self) -> Duration {
        self.last_activity.duration_since(self.established_at)
    }
}

/// Connection event types for monitoring and management
#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    /// Connection attempt started
    Connecting {
        connection_id: ConnectionId,
        peer_id: PeerId,
        address: SocketAddr,
    },
    /// Connection successfully established
    Connected {
        connection_id: ConnectionId,
        peer_id: PeerId,
    },
    /// Handshake completed successfully
    Authenticated {
        connection_id: ConnectionId,
        peer_id: PeerId,
        protocol_version: u32,
    },
    /// Connection disconnected
    Disconnected {
        connection_id: ConnectionId,
        peer_id: PeerId,
        reason: String,
    },
    /// Connection error occurred
    Error {
        connection_id: ConnectionId,
        peer_id: PeerId,
        error: ConnectionError,
    },
    /// Protocol message received
    MessageReceived {
        connection_id: ConnectionId,
        protocol_id: ProtocolId,
        message_size: usize,
    },
    /// Protocol message sent
    MessageSent {
        connection_id: ConnectionId,
        protocol_id: ProtocolId,
        message_size: usize,
    },
}

/// Connection-specific error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConnectionError {
    #[error("TCP connection failed: {0}")]
    TcpError(String),

    #[error("Handshake failed: {0}")]
    HandshakeError(HandshakeError),

    #[error("Multiplexer error: {0}")]
    MultiplexerError(MultiplexerError),

    #[error("Connection timeout")]
    Timeout,

    #[error("Protocol violation: {0}")]
    ProtocolViolation(String),

    #[error("Connection closed by peer")]
    ClosedByPeer,

    #[error("Invalid connection state: expected {expected}, got {actual}")]
    InvalidState { expected: String, actual: String },

    #[error("Peer address resolution failed: {0}")]
    AddressResolution(String),
}

// Manual conversions for types that don't implement Clone
impl From<std::io::Error> for ConnectionError {
    fn from(error: std::io::Error) -> Self {
        Self::TcpError(error.to_string())
    }
}

impl From<HandshakeError> for ConnectionError {
    fn from(error: HandshakeError) -> Self {
        Self::HandshakeError(error)
    }
}

/// Connection statistics for monitoring and debugging
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Total messages sent
    pub messages_sent: u64,
    /// Total messages received
    pub messages_received: u64,
    /// Connection establishment time
    pub connect_time: Option<Duration>,
    /// Last error encountered
    pub last_error: Option<String>,
    /// Number of reconnection attempts
    pub reconnect_attempts: u32,
}

impl ConnectionStats {
    /// Update statistics for sent message
    pub fn record_sent(&mut self, bytes: u64) {
        self.bytes_sent += bytes;
        self.messages_sent += 1;
    }

    /// Update statistics for received message
    pub fn record_received(&mut self, bytes: u64) {
        self.bytes_received += bytes;
        self.messages_received += 1;
    }

    /// Record connection error
    pub fn record_error(&mut self, error: String) {
        self.last_error = Some(error);
    }

    /// Record reconnection attempt
    pub fn record_reconnect_attempt(&mut self) {
        self.reconnect_attempts += 1;
    }
}

/// Global connection registry for tracking active connections
type ConnectionRegistry = Arc<RwLock<HashMap<ConnectionId, Connection>>>;

/// Event handler for connection events
pub trait ConnectionEventHandler: Send + Sync {
    /// Handle a connection event
    fn handle_event(&self, event: ConnectionEvent) -> impl std::future::Future<Output = ()> + Send;
}

/// Connection management configuration
#[derive(Debug, Clone)]
pub struct ConnectionLimits {
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Maximum connections per IP address
    pub max_connections_per_ip: usize,
    /// Connection timeout duration
    pub connect_timeout: Duration,
    /// Handshake timeout duration
    pub handshake_timeout: Duration,
    /// Keepalive interval
    pub keepalive_interval: Duration,
    /// Maximum idle time before disconnection
    pub max_idle_time: Duration,
}

impl Default for ConnectionLimits {
    fn default() -> Self {
        Self {
            max_connections: 200,
            max_connections_per_ip: 10,
            connect_timeout: Duration::from_secs(30),
            handshake_timeout: Duration::from_secs(60),
            keepalive_interval: Duration::from_secs(60),
            max_idle_time: Duration::from_secs(300),
        }
    }
}
