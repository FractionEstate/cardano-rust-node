//! Handshake Protocol State Machine
//!
//! Implements the client-side state machine for the Ouroboros handshake protocol.
//! The handshake must complete before any other mini-protocol can communicate.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use bytes::Bytes;
use tracing::{debug, info, warn};

use super::codec::{decode_message, encode_message};
use super::messages::HandshakeMessage;
use super::types::{NetworkMagic, NodeToNodeVersion, NodeToNodeVersionData, VersionTable};
use super::{HandshakeError, HandshakeResult};

/// Handshake protocol state
#[derive(Debug, Clone, PartialEq)]
pub enum HandshakeState {
    /// Initial state - not yet started
    Start,
    /// Waiting for server's AcceptVersion response
    AwaitAccept {
        proposed_versions: VersionTable,
        sent_at: Instant,
    },
    /// Handshake completed successfully
    Done {
        result: HandshakeResult,
        completed_at: Instant,
    },
    /// Handshake failed
    Failed { error: String, failed_at: Instant },
}

impl HandshakeState {
    /// Check if handshake is complete
    pub fn is_done(&self) -> bool {
        matches!(self, Self::Done { .. })
    }

    /// Check if handshake failed
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Get the handshake result if completed
    pub fn result(&self) -> Option<&HandshakeResult> {
        match self {
            Self::Done { result, .. } => Some(result),
            _ => None,
        }
    }
}

impl std::fmt::Display for HandshakeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start => write!(f, "Start"),
            Self::AwaitAccept { .. } => write!(f, "AwaitAccept"),
            Self::Done { result, .. } => {
                write!(
                    f,
                    "Done(version={}, network={})",
                    result.version,
                    result.network_magic().network_name()
                )
            }
            Self::Failed { error, .. } => write!(f, "Failed({})", error),
        }
    }
}

/// Handshake protocol client
pub struct HandshakeClient {
    /// Current state
    state: HandshakeState,
    /// Our supported versions
    supported_versions: VersionTable,
    /// Expected network magic
    expected_network_magic: NetworkMagic,
    /// Handshake timeout
    timeout: Duration,
}

impl HandshakeClient {
    /// Create a new handshake client
    pub fn new(network_magic: NetworkMagic) -> Self {
        let supported_versions = Self::build_version_table(network_magic);

        Self {
            state: HandshakeState::Start,
            supported_versions,
            expected_network_magic: network_magic,
            timeout: Duration::from_secs(30),
        }
    }

    /// Create handshake client with custom timeout
    pub fn with_timeout(network_magic: NetworkMagic, timeout: Duration) -> Self {
        let mut client = Self::new(network_magic);
        client.timeout = timeout;
        client
    }

    /// Build version table with our supported versions
    fn build_version_table(network_magic: NetworkMagic) -> VersionTable {
        let mut table = HashMap::new();

        // Add V14 (Conway base)
        table.insert(
            NodeToNodeVersion::V14,
            NodeToNodeVersionData::new(
                network_magic,
                super::types::DiffusionMode::InitiatorAndResponder,
                super::types::PeerSharing::Disabled,
                false, // query mode disabled
            ),
        );

        // Add V15 (Conway with SRV support - required for modern testnets)
        table.insert(
            NodeToNodeVersion::V15,
            NodeToNodeVersionData::new(
                network_magic,
                super::types::DiffusionMode::InitiatorAndResponder,
                super::types::PeerSharing::Disabled,
                false, // query mode disabled
            ),
        );

        table
    }
    /// Get current state
    pub fn state(&self) -> &HandshakeState {
        &self.state
    }

    /// Get handshake result if completed
    pub fn result(&self) -> Option<&HandshakeResult> {
        self.state.result()
    }

    /// Start the handshake by proposing versions
    pub fn start(&mut self) -> Result<Bytes, HandshakeError> {
        match &self.state {
            HandshakeState::Start => {
                let message = HandshakeMessage::propose_versions(self.supported_versions.clone());
                let encoded = encode_message(&message)?;

                info!(
                    versions = ?self.supported_versions.keys().collect::<Vec<_>>(),
                    network = %self.expected_network_magic.network_name(),
                    "Sending handshake proposal"
                );

                self.state = HandshakeState::AwaitAccept {
                    proposed_versions: self.supported_versions.clone(),
                    sent_at: Instant::now(),
                };

                Ok(encoded)
            }
            _ => Err(HandshakeError::ProtocolViolation(format!(
                "Cannot start handshake from state: {}",
                self.state
            ))),
        }
    }

    /// Handle incoming handshake message
    pub fn handle_message(&mut self, payload: &[u8]) -> Result<Option<Bytes>, HandshakeError> {
        let message = decode_message(payload)?;

        debug!(
            message = message.name(),
            state = %self.state,
            "Received handshake message"
        );

        match (&self.state, message) {
            // Expecting AcceptVersion in AwaitAccept state
            (
                HandshakeState::AwaitAccept { sent_at, .. },
                HandshakeMessage::MsgAcceptVersion {
                    version,
                    version_data,
                },
            ) => {
                let elapsed = sent_at.elapsed();

                // Verify version was in our proposal
                if !self.supported_versions.contains_key(&version) {
                    let error = format!(
                        "Server accepted version {} which we did not propose",
                        version
                    );
                    warn!(%error);
                    self.state = HandshakeState::Failed {
                        error: error.clone(),
                        failed_at: Instant::now(),
                    };
                    return Err(HandshakeError::ProtocolViolation(error));
                }

                // Verify network magic matches
                if version_data.network_magic != self.expected_network_magic {
                    let error = HandshakeError::NetworkMagicMismatch {
                        expected: self.expected_network_magic.value(),
                        received: version_data.network_magic.value(),
                    };
                    warn!(
                        expected = %self.expected_network_magic.network_name(),
                        received = %version_data.network_magic.network_name(),
                        "Network magic mismatch"
                    );
                    self.state = HandshakeState::Failed {
                        error: error.to_string(),
                        failed_at: Instant::now(),
                    };
                    return Err(error);
                }

                // Handshake successful!
                let result = HandshakeResult::new(version, version_data.clone());

                info!(
                    version = %version,
                    network = %version_data.network_magic.network_name(),
                    elapsed_ms = elapsed.as_millis(),
                    "Handshake completed successfully"
                );

                self.state = HandshakeState::Done {
                    result,
                    completed_at: Instant::now(),
                };

                Ok(None) // No response needed
            }

            // Server refused our proposal
            (HandshakeState::AwaitAccept { .. }, HandshakeMessage::MsgRefuse { reason }) => {
                let error = format!("Server refused handshake: {}", reason);
                warn!(%error, reason = ?reason);

                self.state = HandshakeState::Failed {
                    error: error.clone(),
                    failed_at: Instant::now(),
                };

                Err(HandshakeError::Refused {
                    version: NodeToNodeVersion::V15, // Default to latest
                    reason: error,
                })
            }

            // Unexpected message for current state
            (state, message) => {
                let error = HandshakeError::UnexpectedMessage {
                    state: state.to_string(),
                    message: message.name().to_string(),
                };
                warn!(
                    state = %state,
                    message = message.name(),
                    "Unexpected handshake message"
                );

                self.state = HandshakeState::Failed {
                    error: error.to_string(),
                    failed_at: Instant::now(),
                };

                Err(error)
            }
        }
    }

    /// Check if handshake has timed out
    pub fn check_timeout(&mut self) -> Result<(), HandshakeError> {
        match &self.state {
            HandshakeState::AwaitAccept { sent_at, .. } => {
                if sent_at.elapsed() > self.timeout {
                    let error = "Handshake timeout".to_string();
                    warn!(timeout_secs = self.timeout.as_secs(), "Handshake timed out");

                    self.state = HandshakeState::Failed {
                        error: error.clone(),
                        failed_at: Instant::now(),
                    };

                    return Err(HandshakeError::Timeout);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::messages::RefuseReason;
    use super::*;

    #[test]
    fn test_handshake_client_start() {
        let mut client = HandshakeClient::new(NetworkMagic::PREVIEW_TESTNET);
        assert!(matches!(client.state(), HandshakeState::Start));

        let proposal = client.start().expect("Failed to start handshake");
        assert!(!proposal.is_empty());
        assert!(matches!(client.state(), HandshakeState::AwaitAccept { .. }));
    }

    #[test]
    fn test_handshake_success() {
        let mut client = HandshakeClient::new(NetworkMagic::PREVIEW_TESTNET);
        let _proposal = client.start().unwrap();

        // Simulate server accepting V15
        let accept_msg = HandshakeMessage::accept_version(
            NodeToNodeVersion::V15,
            NodeToNodeVersionData::preview_testnet(),
        );
        let encoded = encode_message(&accept_msg).unwrap();

        let response = client.handle_message(&encoded).unwrap();
        assert!(response.is_none()); // No response needed for AcceptVersion
        assert!(client.state().is_done());
        assert!(client.result().is_some());

        let result = client.result().unwrap();
        assert_eq!(result.version, NodeToNodeVersion::V15);
        assert_eq!(result.network_magic(), NetworkMagic::PREVIEW_TESTNET);
    }

    #[test]
    fn test_handshake_network_mismatch() {
        let mut client = HandshakeClient::new(NetworkMagic::MAINNET);
        let _proposal = client.start().unwrap();

        // Simulate server accepting with wrong network magic
        let accept_msg = HandshakeMessage::accept_version(
            NodeToNodeVersion::V15,
            NodeToNodeVersionData::preview_testnet(), // Wrong network!
        );
        let encoded = encode_message(&accept_msg).unwrap();

        let result = client.handle_message(&encoded);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            HandshakeError::NetworkMagicMismatch { .. }
        ));
        assert!(client.state().is_failed());
    }

    #[test]
    fn test_handshake_refuse() {
        let mut client = HandshakeClient::new(NetworkMagic::PREVIEW_TESTNET);
        let _proposal = client.start().unwrap();

        // Simulate server refusing
        let refuse_msg = HandshakeMessage::refuse(RefuseReason::version_mismatch(
            vec![NodeToNodeVersion::V14],
            vec![16, 17], // Server only supports unsupported versions
        ));
        let encoded = encode_message(&refuse_msg).unwrap();

        let result = client.handle_message(&encoded);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            HandshakeError::Refused { .. }
        ));
        assert!(client.state().is_failed());
    }

    #[test]
    fn test_unexpected_message() {
        let mut client = HandshakeClient::new(NetworkMagic::PREVIEW_TESTNET);

        // Try to handle message before starting
        let accept_msg = HandshakeMessage::accept_version(
            NodeToNodeVersion::V15,
            NodeToNodeVersionData::preview_testnet(),
        );
        let encoded = encode_message(&accept_msg).unwrap();

        let result = client.handle_message(&encoded);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            HandshakeError::UnexpectedMessage { .. }
        ));
    }
}
