//! Connection Management Tests
//!
//! Comprehensive tests for connection state machine, multiplexer functionality,
//! handshake protocol, and integration scenarios.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::connection::*;
use crate::connection::multiplexer::MultiplexerError;
use crate::{NetworkError};
use crate::diffusion::{PeerId, PeerInfo};

/// Test utilities
mod utils {
    use super::*;

    /// Create test peer info
    pub fn create_test_peer(id: u8) -> PeerInfo {
        let peer_id = PeerId::new([id; 32]);
        let address = format!("127.0.0.1:{}", 3000 + id as u16).parse().unwrap();
        PeerInfo::new(peer_id, address)
    }

    /// Create test TCP listener
    pub async fn create_test_listener() -> (TcpListener, SocketAddr) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        (listener, addr)
    }

    /// Simple echo server for testing
    pub async fn run_echo_server(listener: TcpListener) {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                while let Ok(n) = stream.try_read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    let _ = stream.try_write(&buf[..n]);
                }
            });
        }
    }
}

#[cfg(test)]
mod state_tests {
    use super::*;
    use crate::connection::state::*;

    #[test]
    fn test_connection_state_transitions() {
        let state = ConnectionState::Disconnected;

        // Valid transitions
        assert!(state.can_transition_to(ConnectionState::Connecting));

        // Invalid transitions
        assert!(!state.can_transition_to(ConnectionState::Authenticated));
        assert!(!state.can_transition_to(ConnectionState::Closing));

        // Terminal states
        assert!(ConnectionState::Disconnected.is_terminal());
        assert!(ConnectionState::Failed.is_terminal());
        assert!(!ConnectionState::Connected.is_terminal());
    }

    #[test]
    fn test_state_machine_lifecycle() {
        let connection_id = ConnectionId::new();
        let peer_id = PeerId::new([1u8; 32]);
        let mut machine = ConnectionStateMachine::new(connection_id, peer_id);

        // Initial state
        assert_eq!(machine.current_state(), ConnectionState::Disconnected);

        // Valid transition sequence
        machine.transition(
            ConnectionState::Connecting,
            TransitionReason::UserInitiated
        ).unwrap();

        machine.transition(
            ConnectionState::Connected,
            TransitionReason::TcpEstablished
        ).unwrap();

        machine.transition(
            ConnectionState::Authenticated,
            TransitionReason::HandshakeComplete
        ).unwrap();

        // Check final state and history
        assert_eq!(machine.current_state(), ConnectionState::Authenticated);
        assert_eq!(machine.history().len(), 3);
        assert!(machine.info.is_authenticated());
        assert!(machine.info.connection_duration().is_some());
    }

    #[test]
    fn test_invalid_state_transition() {
        let connection_id = ConnectionId::new();
        let peer_id = PeerId::new([1u8; 32]);
        let mut machine = ConnectionStateMachine::new(connection_id, peer_id);

        // Try invalid transition
        let result = machine.transition(
            ConnectionState::Authenticated,
            TransitionReason::UserInitiated
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            ConnectionError::InvalidState { expected, actual } => {
                assert!(expected.contains("valid transition"));
                assert_eq!(actual, "Authenticated");
            }
            _ => panic!("Unexpected error type"),
        }
    }

    #[test]
    fn test_state_machine_failure() {
        let connection_id = ConnectionId::new();
        let peer_id = PeerId::new([1u8; 32]);
        let mut machine = ConnectionStateMachine::new(connection_id, peer_id);

        // Transition to connecting
        machine.transition(
            ConnectionState::Connecting,
            TransitionReason::UserInitiated
        ).unwrap();

        // Simulate failure
        machine.fail(
            TransitionReason::NetworkError,
            "Connection timeout".to_string()
        );

        assert_eq!(machine.current_state(), ConnectionState::Failed);
        assert_eq!(machine.info.stats.last_error, Some("Connection timeout".to_string()));
    }
}

#[cfg(test)]
mod multiplexer_tests {
    use super::*;
    use crate::connection::multiplexer::*;

    #[test]
    fn test_protocol_id_constants() {
        assert_eq!(ProtocolId::HANDSHAKE.value(), 0);
        assert_eq!(ProtocolId::CHAINSYNC.value(), 2);
        assert_eq!(ProtocolId::BLOCKFETCH.value(), 3);
        assert_eq!(ProtocolId::TXSUBMISSION.value(), 4);
        assert_eq!(ProtocolId::KEEPALIVE.value(), 8);
        assert_eq!(ProtocolId::GOSSIP.value(), 9);
    }

    #[test]
    fn test_message_frame_encoding_roundtrip() {
        let protocol_id = ProtocolId::CHAINSYNC;
        let payload = Bytes::from_static(b"test message payload");
        let frame = MessageFrame::new(protocol_id, payload.clone());

        // Encode frame
        let encoded = frame.encode().unwrap();

        // Decode frame
        let decoded = MessageFrame::decode(encoded).unwrap();

        assert_eq!(decoded.protocol_id, protocol_id);
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn test_frame_size_calculation() {
        let payload = Bytes::from_static(b"hello world");
        let frame = MessageFrame::new(ProtocolId::HANDSHAKE, payload);

        // 4 bytes header + 11 bytes payload = 15 bytes
        assert_eq!(frame.frame_size(), 15);
    }

    #[test]
    fn test_invalid_frame_decoding() {
        // Too short frame
        let invalid_data = Bytes::from_static(b"xx");
        let result = MessageFrame::decode(invalid_data);

        assert!(result.is_err());
        match result.unwrap_err() {
            NetworkError::MultiplexerError(MultiplexerError::InvalidFrameSize { expected, actual }) => {
                assert_eq!(expected, 4);
                assert_eq!(actual, 2);
            }
            _ => panic!("Unexpected error type"),
        }
    }

    #[tokio::test]
    async fn test_echo_protocol_handler() {
        let handler = EchoProtocolHandler::new(
            ProtocolId::CHAINSYNC,
            "TestEcho".to_string(),
        );

        let message = Bytes::from_static(b"echo test");
        let response = handler.handle_message(message.clone()).await.unwrap();

        assert_eq!(response, Some(message));
        assert_eq!(handler.protocol_id(), ProtocolId::CHAINSYNC);
        assert_eq!(handler.name(), "TestEcho");
    }

    #[test]
    fn test_multiplexer_config_defaults() {
        let config = MultiplexerConfig::default();

        assert_eq!(config.max_frame_size, 64 * 1024);
        assert_eq!(config.max_outbound_queue, 1000);
        assert_eq!(config.read_buffer_size, 8192);
        assert_eq!(config.write_buffer_size, 8192);
        assert!(!config.enable_compression);
    }
}

#[cfg(test)]
mod handshake_tests {
    use super::*;
    use crate::connection::handshake::*;

    #[test]
    fn test_protocol_version_ordering() {
        let v1_0 = ProtocolVersion::new(1, 0);
        let v1_1 = ProtocolVersion::new(1, 1);
        let v2_0 = ProtocolVersion::new(2, 0);

        assert!(v1_0 < v1_1);
        assert!(v1_1 < v2_0);
        assert!(v1_0 < v2_0);
    }

    #[test]
    fn test_version_compatibility() {
        let v1_0 = ProtocolVersion::new(1, 0);
        let v1_1 = ProtocolVersion::new(1, 1);
        let v2_0 = ProtocolVersion::new(2, 0);

        // Same major version is compatible
        assert!(v1_0.is_compatible_with(&v1_1));
        assert!(v1_1.is_compatible_with(&v1_0));

        // Different major version is not compatible
        assert!(!v1_0.is_compatible_with(&v2_0));
        assert!(!v2_0.is_compatible_with(&v1_0));
    }

    #[test]
    fn test_version_data_network_magic() {
        let mainnet = VersionData::mainnet();
        let testnet = VersionData::testnet();

        assert_eq!(mainnet.network_magic, 764824073);
        assert_eq!(testnet.network_magic, 1097911063);
        assert_ne!(mainnet.network_magic, testnet.network_magic);

        assert!(!mainnet.initiator_only);
        assert!(!testnet.initiator_only);
        assert_eq!(mainnet.modes, vec![ProtocolMode::Duplex]);
        assert_eq!(testnet.modes, vec![ProtocolMode::Duplex]);
    }

    #[test]
    fn test_protocol_mode_compatibility() {
        let handshake = HandshakeProtocol::new();

        // Duplex modes are always compatible
        assert!(handshake.is_mode_compatible(
            &[ProtocolMode::Duplex],
            &[ProtocolMode::InitiatorOnly]
        ));
        assert!(handshake.is_mode_compatible(
            &[ProtocolMode::InitiatorOnly],
            &[ProtocolMode::Duplex]
        ));

        // Complementary modes are compatible
        assert!(handshake.is_mode_compatible(
            &[ProtocolMode::InitiatorOnly],
            &[ProtocolMode::ResponderOnly]
        ));
        assert!(handshake.is_mode_compatible(
            &[ProtocolMode::ResponderOnly],
            &[ProtocolMode::InitiatorOnly]
        ));

        // Same unidirectional modes are not compatible
        assert!(!handshake.is_mode_compatible(
            &[ProtocolMode::InitiatorOnly],
            &[ProtocolMode::InitiatorOnly]
        ));
        assert!(!handshake.is_mode_compatible(
            &[ProtocolMode::ResponderOnly],
            &[ProtocolMode::ResponderOnly]
        ));
    }

    #[test]
    fn test_handshake_message_serialization() {
        let versions = VersionNegotiation::create_proposal(764824073);
        let message = HandshakeMessage::ProposeVersions { versions };

        // Test CBOR encoding/decoding
        let encoded = minicbor::to_vec(&message).unwrap();
        let decoded: HandshakeMessage = minicbor::decode(&encoded).unwrap();

        match decoded {
            HandshakeMessage::ProposeVersions { versions } => {
                assert!(versions.contains_key(&ProtocolVersion::CURRENT));
                let version_data = &versions[&ProtocolVersion::CURRENT];
                assert_eq!(version_data.network_magic, 764824073);
            }
            _ => panic!("Unexpected message type after roundtrip"),
        }
    }

    #[test]
    fn test_refuse_reason_display() {
        let reasons = vec![
            RefuseReason::VersionMismatch,
            RefuseReason::HandshakeDecodeError("Test error".to_string()),
            RefuseReason::Refused("Test refusal".to_string()),
            RefuseReason::NetworkMismatch { expected: 123, received: 456 },
            RefuseReason::ModeIncompatible,
        ];

        for reason in reasons {
            let display = format!("{}", reason);
            assert!(!display.is_empty());
            // Each reason should have a meaningful display string
            match reason {
                RefuseReason::NetworkMismatch { expected, received } => {
                    assert!(display.contains(&expected.to_string()));
                    assert!(display.contains(&received.to_string()));
                }
                RefuseReason::HandshakeDecodeError(ref msg) => {
                    assert!(display.contains(msg));
                }
                RefuseReason::Refused(ref msg) => {
                    assert!(display.contains(msg));
                }
                _ => {} // Other variants just need non-empty display
            }
        }
    }

    #[test]
    fn test_version_negotiation_utilities() {
        // Test version support check
        let current = ProtocolVersion::CURRENT;
        let minimum = ProtocolVersion::MINIMUM;
        let future = ProtocolVersion::new(99, 0);

        assert!(VersionNegotiation::is_version_supported(&current));
        assert!(VersionNegotiation::is_version_supported(&minimum));
        assert!(!VersionNegotiation::is_version_supported(&future));

        // Test supported range
        let (min, max) = VersionNegotiation::supported_range();
        assert_eq!(min, ProtocolVersion::MINIMUM);
        assert_eq!(max, ProtocolVersion::CURRENT);

        // Test proposal creation
        let proposal = VersionNegotiation::create_proposal(764824073);
        assert!(!proposal.is_empty());
        assert!(proposal.contains_key(&ProtocolVersion::CURRENT));
    }
}

#[cfg(test)]
mod monitor_tests {
    use super::*;
    use crate::connection::monitor::*;

    #[test]
    fn test_health_status_properties() {
        // Test acceptability
        assert!(HealthStatus::Healthy.is_acceptable());
        assert!(HealthStatus::Degraded.is_acceptable());
        assert!(!HealthStatus::Unhealthy.is_acceptable());
        assert!(!HealthStatus::Unknown.is_acceptable());

        // Test attention requirements
        assert!(!HealthStatus::Healthy.needs_attention());
        assert!(HealthStatus::Degraded.needs_attention());
        assert!(HealthStatus::Unhealthy.needs_attention());
        assert!(!HealthStatus::Unknown.needs_attention());
    }

    #[test]
    fn test_connection_metrics() {
        let connection_id = ConnectionId::new();
        let peer_id = PeerId::new([1u8; 32]);
        let mut metrics = ConnectionMetrics::new(connection_id, peer_id);

        // Test initial state
        assert_eq!(metrics.connection_id, connection_id);
        assert_eq!(metrics.peer_id, peer_id);
        assert_eq!(metrics.failed_keepalives, 0);
        assert_eq!(metrics.protocol_errors, 0);
        assert_eq!(metrics.health, HealthStatus::Healthy);
        assert!(metrics.recent_rtt.is_none());

        // Test activity update
        let initial_activity = metrics.last_message_at;
        std::thread::sleep(Duration::from_millis(1));
        metrics.update_activity();
        assert!(metrics.last_message_at > initial_activity);

        // Test keepalive recording
        let sent_at = std::time::Instant::now();
        metrics.record_keepalive_sent();
        assert!(metrics.last_keepalive_sent.is_some());

        std::thread::sleep(Duration::from_millis(1));
        metrics.record_keepalive_response(sent_at);
        assert!(metrics.last_keepalive_response.is_some());
        assert_eq!(metrics.failed_keepalives, 0);
        assert!(metrics.recent_rtt.is_some());

        // Test error recording
        metrics.record_protocol_error();
        assert_eq!(metrics.protocol_errors, 1);

        // Test failure recording
        metrics.record_keepalive_failure();
        assert_eq!(metrics.failed_keepalives, 1);
    }

    #[test]
    fn test_health_status_updates() {
        let connection_id = ConnectionId::new();
        let peer_id = PeerId::new([1u8; 32]);
        let mut metrics = ConnectionMetrics::new(connection_id, peer_id);
        let config = MonitorConfig::default();

        // Initially healthy
        metrics.update_health(&config);
        assert_eq!(metrics.health, HealthStatus::Healthy);

        // Simulate multiple keepalive failures
        for _ in 0..config.max_failed_keepalives {
            metrics.record_keepalive_failure();
        }
        metrics.update_health(&config);
        assert_eq!(metrics.health, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_keepalive_messages() {
        let message = KeepAliveMessage::new(42);
        assert_eq!(message.cookie, 42);

        let response = KeepAliveResponse::new(42);
        assert_eq!(response.cookie, 42);

        // Test that timestamps are set
        assert!(message.timestamp <= std::time::SystemTime::now());
        assert!(response.timestamp <= std::time::SystemTime::now());
    }

    #[tokio::test]
    async fn test_connection_monitor_lifecycle() {
        let config = MonitorConfig::default();
        let monitor = ConnectionMonitor::new(config).await.unwrap();

        let connection_id = ConnectionId::new();
        let peer_id = PeerId::new([1u8; 32]);

        // Initially no connections monitored
        assert!(monitor.get_all_metrics().await.is_empty());
        assert!(monitor.get_health(connection_id).await.is_none());

        // Start monitoring
        monitor.start_monitoring(connection_id, peer_id).await;

        // Check connection is now monitored
        assert_eq!(monitor.get_all_metrics().await.len(), 1);
        assert_eq!(monitor.get_health(connection_id).await, Some(HealthStatus::Healthy));

        // Update activity
        monitor.update_activity(connection_id).await;

        // Record error
        monitor.record_error(connection_id).await;

        // Stop monitoring
        monitor.stop_monitoring(connection_id).await;

        // Check connection is no longer monitored
        assert!(monitor.get_all_metrics().await.is_empty());
        assert!(monitor.get_health(connection_id).await.is_none());
    }

    #[tokio::test]
    async fn test_keepalive_protocol() {
        let config = MonitorConfig::default();
        let monitor = Arc::new(ConnectionMonitor::new(config).await.unwrap());
        let protocol = KeepAliveProtocol::new(monitor.clone());

        let connection_id = ConnectionId::new();
        let peer_id = PeerId::new([1u8; 32]);

        // Start monitoring connection
        monitor.start_monitoring(connection_id, peer_id).await;

        // Test keepalive handling
        let message = KeepAliveMessage::new(123);
        let response = protocol.handle_keepalive(connection_id, message.clone()).await;

        assert_eq!(response.cookie, message.cookie);

        // Test response handling
        protocol.handle_response(connection_id, response).await;

        // Cleanup
        monitor.stop_monitoring(connection_id).await;
    }

    #[test]
    fn test_monitor_config_defaults() {
        let config = MonitorConfig::default();

        assert_eq!(config.keepalive_interval, Duration::from_secs(60));
        assert_eq!(config.keepalive_timeout, Duration::from_secs(30));
        assert_eq!(config.max_failed_keepalives, 3);
        assert_eq!(config.degraded_threshold, Duration::from_secs(120));
        assert_eq!(config.unhealthy_threshold, Duration::from_secs(300));
        assert_eq!(config.health_check_interval, Duration::from_secs(30));
        assert!(config.auto_reconnect);
        assert_eq!(config.reconnect_delay, Duration::from_secs(60));
    }
}

#[cfg(test)]
mod manager_tests {
    use super::*;
    use crate::connection::manager::*;

    #[tokio::test]
    async fn test_connection_manager_creation() {
        let config = ConnectionConfig::default();
        let manager = ConnectionManager::new(config).await.unwrap();

        assert_eq!(manager.connection_count().await, 0);

        let stats = manager.connection_stats().await;
        assert!(stats.is_empty());
    }

    #[test]
    fn test_connection_config_defaults() {
        let config = ConnectionConfig::default();

        assert_eq!(config.limits.max_connections, 200);
        assert_eq!(config.limits.connect_timeout, Duration::from_secs(30));
        assert_eq!(config.multiplexer.max_frame_size, 64 * 1024);
        assert!(config.monitor.auto_reconnect);
        assert!(config.reconnect.enabled);
    }

    #[test]
    fn test_reconnect_config() {
        let config = ReconnectConfig::default();

        assert!(config.enabled);
        assert_eq!(config.initial_delay, Duration::from_secs(1));
        assert_eq!(config.max_delay, Duration::from_secs(300));
        assert_eq!(config.backoff_factor, 2.0);
        assert_eq!(config.max_attempts, 10);
        assert_eq!(config.reset_after, Duration::from_secs(3600));
    }

    #[tokio::test]
    async fn test_connection_id_uniqueness() {
        let id1 = ConnectionId::new();
        let id2 = ConnectionId::new();
        let id3 = ConnectionId::new();

        // Each connection ID should be unique
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::connection::*;

    #[tokio::test]
    async fn test_connection_lifecycle_integration() {
        let peer_info = utils::create_test_peer(1);
        let connection = Connection::new(peer_info.clone(), peer_info.address);

        // Test initial connection state
        assert_eq!(connection.state, ConnectionState::Disconnected);
        assert!(!connection.is_active());

        // Simulate connection progression would happen through manager
        // This test just verifies the Connection struct works correctly
        assert_eq!(connection.peer.peer_id, peer_info.peer_id);
        assert_eq!(connection.address, peer_info.address);
    }

    #[tokio::test]
    async fn test_event_system() {
        let connection_id = ConnectionId::new();
        let peer_id = PeerId::new([1u8; 32]);
        let address = "127.0.0.1:3001".parse().unwrap();

        // Test connection events
        let events = vec![
            ConnectionEvent::Connecting {
                connection_id,
                peer_id,
                address,
            },
            ConnectionEvent::Connected {
                connection_id,
                peer_id,
            },
            ConnectionEvent::Authenticated {
                connection_id,
                peer_id,
                protocol_version: 1000,
            },
            ConnectionEvent::Disconnected {
                connection_id,
                peer_id,
                reason: "Test disconnect".to_string(),
            },
        ];

        // Verify events can be created and contain expected data
        for event in events {
            match event {
                ConnectionEvent::Connecting { connection_id: cid, peer_id: pid, address: addr } => {
                    assert_eq!(cid, connection_id);
                    assert_eq!(pid, peer_id);
                    assert_eq!(addr, address);
                }
                ConnectionEvent::Connected { connection_id: cid, peer_id: pid } => {
                    assert_eq!(cid, connection_id);
                    assert_eq!(pid, peer_id);
                }
                ConnectionEvent::Authenticated { connection_id: cid, peer_id: pid, protocol_version } => {
                    assert_eq!(cid, connection_id);
                    assert_eq!(pid, peer_id);
                    assert_eq!(protocol_version, 1000);
                }
                ConnectionEvent::Disconnected { connection_id: cid, peer_id: pid, reason } => {
                    assert_eq!(cid, connection_id);
                    assert_eq!(pid, peer_id);
                    assert_eq!(reason, "Test disconnect");
                }
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn test_error_propagation() {
        // Test that connection errors properly convert to network errors
        let connection_error = ConnectionError::Timeout;
        let network_error: NetworkError = connection_error.into();

        match network_error {
            NetworkError::ConnectionError(conn_err) => {
                let error_msg = conn_err.to_string();
                assert!(error_msg.contains("timeout"));
            }
            _ => panic!("Unexpected error conversion"),
        }

        // Test multiplexer error propagation
        let multiplexer_error = MultiplexerError::ConnectionClosed;
        let connection_error: ConnectionError = multiplexer_error.into();

        match connection_error {
            ConnectionError::MultiplexerError(err) => {
                assert!(matches!(err, MultiplexerError::ConnectionClosed));
            }
            _ => panic!("Unexpected error conversion"),
        }
    }
}
