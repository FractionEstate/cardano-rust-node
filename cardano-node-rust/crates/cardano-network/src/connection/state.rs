//! Connection State Management
//!
//! This module implements the connection state machine for Cardano P2P connections,
//! managing the lifecycle from initial connection through authentication and
//! eventual disconnection.

use std::fmt;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use super::{ConnectionError, ConnectionId};
use crate::diffusion::PeerId;

/// Connection states following the Cardano P2P protocol lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionState {
    /// No connection established
    Disconnected,
    /// TCP connection initiated, awaiting establishment
    Connecting,
    /// TCP connection established, handshake pending
    Connected,
    /// Handshake completed, protocols available
    Authenticated,
    /// Connection being gracefully closed
    Closing,
    /// Connection failed or forcibly closed
    Failed,
}

impl ConnectionState {
    /// Check if state allows data transmission
    pub fn can_transmit(&self) -> bool {
        matches!(self, Self::Authenticated)
    }

    /// Check if state is terminal (no further transitions possible)
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Disconnected | Self::Failed)
    }

    /// Check if connection is in progress
    pub fn is_connecting(&self) -> bool {
        matches!(self, Self::Connecting | Self::Connected)
    }

    /// Get all valid next states from current state
    pub fn valid_transitions(&self) -> Vec<ConnectionState> {
        match self {
            Self::Disconnected => vec![Self::Connecting],
            Self::Connecting => vec![Self::Connected, Self::Failed, Self::Disconnected],
            Self::Connected => vec![Self::Authenticated, Self::Failed, Self::Closing],
            Self::Authenticated => vec![Self::Closing, Self::Failed],
            Self::Closing => vec![Self::Disconnected, Self::Failed],
            Self::Failed => vec![Self::Disconnected],
        }
    }

    /// Check if transition to target state is valid
    pub fn can_transition_to(&self, target: ConnectionState) -> bool {
        self.valid_transitions().contains(&target)
    }
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => write!(f, "Disconnected"),
            Self::Connecting => write!(f, "Connecting"),
            Self::Connected => write!(f, "Connected"),
            Self::Authenticated => write!(f, "Authenticated"),
            Self::Closing => write!(f, "Closing"),
            Self::Failed => write!(f, "Failed"),
        }
    }
}

/// State transition event with context
#[derive(Debug, Clone)]
pub struct StateTransition {
    /// Connection identifier
    pub connection_id: ConnectionId,
    /// Peer identifier
    pub peer_id: PeerId,
    /// Previous state
    pub from: ConnectionState,
    /// New state
    pub to: ConnectionState,
    /// Transition timestamp
    pub timestamp: SystemTime,
    /// Transition reason or trigger
    pub reason: TransitionReason,
    /// Optional error if transition due to failure
    pub error: Option<String>,
}

impl StateTransition {
    /// Create a new state transition
    pub fn new(
        connection_id: ConnectionId,
        peer_id: PeerId,
        from: ConnectionState,
        to: ConnectionState,
        reason: TransitionReason,
    ) -> Self {
        Self {
            connection_id,
            peer_id,
            from,
            to,
            timestamp: SystemTime::now(),
            reason,
            error: None,
        }
    }

    /// Create a failure transition with error context
    pub fn with_error(
        connection_id: ConnectionId,
        peer_id: PeerId,
        from: ConnectionState,
        reason: TransitionReason,
        error: String,
    ) -> Self {
        Self {
            connection_id,
            peer_id,
            from,
            to: ConnectionState::Failed,
            timestamp: SystemTime::now(),
            reason,
            error: Some(error),
        }
    }

    /// Check if this is a failure transition
    pub fn is_failure(&self) -> bool {
        self.to == ConnectionState::Failed
    }

    /// Check if this is a successful connection
    pub fn is_success(&self) -> bool {
        self.to == ConnectionState::Authenticated
    }
}

/// Reasons for state transitions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionReason {
    /// User initiated connection
    UserInitiated,
    /// Incoming connection accepted
    IncomingConnection,
    /// TCP connection established
    TcpEstablished,
    /// Handshake completed successfully
    HandshakeComplete,
    /// User requested disconnection
    UserDisconnect,
    /// Graceful peer disconnection
    PeerDisconnect,
    /// Connection timeout occurred
    Timeout,
    /// Protocol violation detected
    ProtocolViolation,
    /// Network error encountered
    NetworkError,
    /// Authentication failed
    AuthenticationFailed,
    /// Resource limits exceeded
    ResourceExhausted,
    /// Peer reputation too low
    ReputationFailure,
}

impl fmt::Display for TransitionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserInitiated => write!(f, "User initiated"),
            Self::IncomingConnection => write!(f, "Incoming connection"),
            Self::TcpEstablished => write!(f, "TCP established"),
            Self::HandshakeComplete => write!(f, "Handshake complete"),
            Self::UserDisconnect => write!(f, "User disconnect"),
            Self::PeerDisconnect => write!(f, "Peer disconnect"),
            Self::Timeout => write!(f, "Timeout"),
            Self::ProtocolViolation => write!(f, "Protocol violation"),
            Self::NetworkError => write!(f, "Network error"),
            Self::AuthenticationFailed => write!(f, "Authentication failed"),
            Self::ResourceExhausted => write!(f, "Resource exhausted"),
            Self::ReputationFailure => write!(f, "Reputation failure"),
        }
    }
}

/// Connection information including state and metadata
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Connection identifier
    pub id: ConnectionId,
    /// Associated peer
    pub peer_id: PeerId,
    /// Current state
    pub state: ConnectionState,
    /// State entry time
    pub state_since: SystemTime,
    /// Connection establishment time
    pub connected_at: Option<SystemTime>,
    /// Authentication completion time
    pub authenticated_at: Option<SystemTime>,
    /// Last state transition
    pub last_transition: Option<StateTransition>,
    /// Connection statistics
    pub stats: ConnectionStats,
}

impl ConnectionInfo {
    /// Create new connection info
    pub fn new(id: ConnectionId, peer_id: PeerId) -> Self {
        Self {
            id,
            peer_id,
            state: ConnectionState::Disconnected,
            state_since: SystemTime::now(),
            connected_at: None,
            authenticated_at: None,
            last_transition: None,
            stats: ConnectionStats::default(),
        }
    }

    /// Update connection state with transition
    pub fn transition_to(
        &mut self,
        new_state: ConnectionState,
        reason: TransitionReason,
    ) -> Result<StateTransition, ConnectionError> {
        // Validate transition
        if !self.state.can_transition_to(new_state) {
            return Err(ConnectionError::InvalidState {
                expected: format!("valid transition from {}", self.state),
                actual: new_state.to_string(),
            });
        }

        // Create transition record
        let transition = StateTransition::new(
            self.id,
            self.peer_id,
            self.state,
            new_state,
            reason,
        );

        // Update state and timestamps
        self.state = new_state;
        self.state_since = SystemTime::now();
        self.last_transition = Some(transition.clone());

        // Update lifecycle timestamps
        match new_state {
            ConnectionState::Connected => {
                self.connected_at = Some(SystemTime::now());
            }
            ConnectionState::Authenticated => {
                self.authenticated_at = Some(SystemTime::now());
            }
            _ => {}
        }

        Ok(transition)
    }

    /// Force transition to failed state with error
    pub fn fail_with_error(
        &mut self,
        reason: TransitionReason,
        error: String,
    ) -> StateTransition {
        let transition = StateTransition::with_error(
            self.id,
            self.peer_id,
            self.state,
            reason,
            error.clone(),
        );

        self.state = ConnectionState::Failed;
        self.state_since = SystemTime::now();
        self.last_transition = Some(transition.clone());
        self.stats.record_error(error);

        transition
    }

    /// Get time spent in current state
    pub fn time_in_state(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.state_since)
            .unwrap_or_default()
    }

    /// Get total connection time if connected
    pub fn connection_duration(&self) -> Option<Duration> {
        self.connected_at.and_then(|connected| {
            SystemTime::now()
                .duration_since(connected)
                .ok()
        })
    }

    /// Check if connection has been authenticated
    pub fn is_authenticated(&self) -> bool {
        self.state == ConnectionState::Authenticated
    }

    /// Check if connection is active
    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            ConnectionState::Connected | ConnectionState::Authenticated
        )
    }
}

/// State machine for tracking connection lifecycle
#[derive(Debug)]
pub struct ConnectionStateMachine {
    /// Current connection information
    pub info: ConnectionInfo,
    /// State transition history (limited size)
    transition_history: Vec<StateTransition>,
    /// Maximum history size
    max_history_size: usize,
}

impl ConnectionStateMachine {
    /// Create new state machine for connection
    pub fn new(connection_id: ConnectionId, peer_id: PeerId) -> Self {
        Self {
            info: ConnectionInfo::new(connection_id, peer_id),
            transition_history: Vec::new(),
            max_history_size: 100,
        }
    }

    /// Transition to new state
    pub fn transition(
        &mut self,
        new_state: ConnectionState,
        reason: TransitionReason,
    ) -> Result<(), ConnectionError> {
        let transition = self.info.transition_to(new_state, reason)?;

        // Add to history
        self.transition_history.push(transition);

        // Limit history size
        if self.transition_history.len() > self.max_history_size {
            self.transition_history.remove(0);
        }

        Ok(())
    }

    /// Force failure with error
    pub fn fail(&mut self, reason: TransitionReason, error: String) {
        let transition = self.info.fail_with_error(reason, error);
        self.transition_history.push(transition);

        // Limit history size
        if self.transition_history.len() > self.max_history_size {
            self.transition_history.remove(0);
        }
    }

    /// Get current state
    pub fn current_state(&self) -> ConnectionState {
        self.info.state
    }

    /// Get transition history
    pub fn history(&self) -> &[StateTransition] {
        &self.transition_history
    }

    /// Get last transition
    pub fn last_transition(&self) -> Option<&StateTransition> {
        self.transition_history.last()
    }

    /// Reset state machine to disconnected
    pub fn reset(&mut self) {
        let transition = StateTransition::new(
            self.info.id,
            self.info.peer_id,
            self.info.state,
            ConnectionState::Disconnected,
            TransitionReason::UserDisconnect,
        );

        self.info.state = ConnectionState::Disconnected;
        self.info.state_since = SystemTime::now();
        self.info.connected_at = None;
        self.info.authenticated_at = None;
        self.info.last_transition = Some(transition.clone());

        self.transition_history.push(transition);
    }
}

/// Connection statistics tracking
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    /// Total bytes transmitted
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Number of messages sent
    pub messages_sent: u64,
    /// Number of messages received
    pub messages_received: u64,
    /// Number of errors encountered
    pub errors: u64,
    /// Last error message
    pub last_error: Option<String>,
    /// Connection attempts made
    pub connection_attempts: u32,
}

impl ConnectionStats {
    /// Record bytes sent
    pub fn record_sent(&mut self, bytes: u64) {
        self.bytes_sent += bytes;
        self.messages_sent += 1;
    }

    /// Record bytes received
    pub fn record_received(&mut self, bytes: u64) {
        self.bytes_received += bytes;
        self.messages_received += 1;
    }

    /// Record error
    pub fn record_error(&mut self, error: String) {
        self.errors += 1;
        self.last_error = Some(error);
    }

    /// Record connection attempt
    pub fn record_connection_attempt(&mut self) {
        self.connection_attempts += 1;
    }

    /// Get total bytes transferred
    pub fn total_bytes(&self) -> u64 {
        self.bytes_sent + self.bytes_received
    }

    /// Get total messages processed
    pub fn total_messages(&self) -> u64 {
        self.messages_sent + self.messages_received
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diffusion::PeerId;

    #[test]
    fn test_connection_state_transitions() {
        let state = ConnectionState::Disconnected;

        // Valid transitions
        assert!(state.can_transition_to(ConnectionState::Connecting));

        // Invalid transitions
        assert!(!state.can_transition_to(ConnectionState::Authenticated));
        assert!(!state.can_transition_to(ConnectionState::Closing));
    }

    #[test]
    fn test_state_machine_lifecycle() {
        let connection_id = ConnectionId::new();
        let peer_id = PeerId::new([1u8; 32]);
        let mut machine = ConnectionStateMachine::new(connection_id, peer_id);

        // Initial state
        assert_eq!(machine.current_state(), ConnectionState::Disconnected);

        // Valid transition sequence
        assert!(machine.transition(
            ConnectionState::Connecting,
            TransitionReason::UserInitiated
        ).is_ok());

        assert!(machine.transition(
            ConnectionState::Connected,
            TransitionReason::TcpEstablished
        ).is_ok());

        assert!(machine.transition(
            ConnectionState::Authenticated,
            TransitionReason::HandshakeComplete
        ).is_ok());

        // Check final state
        assert_eq!(machine.current_state(), ConnectionState::Authenticated);
        assert_eq!(machine.history().len(), 3);
    }

    #[test]
    fn test_invalid_transition() {
        let connection_id = ConnectionId::new();
        let peer_id = PeerId::new([1u8; 32]);
        let mut machine = ConnectionStateMachine::new(connection_id, peer_id);

        // Try invalid transition
        let result = machine.transition(
            ConnectionState::Authenticated,
            TransitionReason::UserInitiated
        );

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConnectionError::InvalidState { .. }));
    }

    #[test]
    fn test_connection_stats() {
        let mut stats = ConnectionStats::default();

        stats.record_sent(100);
        stats.record_received(200);
        stats.record_error("Test error".to_string());

        assert_eq!(stats.total_bytes(), 300);
        assert_eq!(stats.total_messages(), 2);
        assert_eq!(stats.errors, 1);
        assert_eq!(stats.last_error, Some("Test error".to_string()));
    }
}
