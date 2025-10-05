//! Handshake Protocol Implementation
//!
//! This module implements the Cardano P2P handshake protocol for version negotiation
//! and capability agreement between peers.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use bytes::{BufMut, BytesMut};
use minicbor::{Decode, Encode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::debug;

/// Protocol version for Cardano P2P
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct ProtocolVersion {
    /// Major version number
    #[n(0)]
    pub major: u16,
    /// Minor version number
    #[n(1)]
    pub minor: u16,
}
impl ProtocolVersion {
    /// Create new protocol version
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// Current supported version
    pub const CURRENT: Self = Self::new(1, 0);

    /// Minimum supported version
    pub const MINIMUM: Self = Self::new(1, 0);

    /// Check if version is compatible
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        // For now, only exact major version match
        self.major == other.major
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Version-specific data exchanged during handshake
#[derive(Debug, Clone, Encode, Decode)]
pub struct VersionData {
    /// Network magic number (mainnet/testnet identifier)
    #[n(0)]
    pub network_magic: u32,
    /// Whether this node can only initiate connections
    #[n(1)]
    pub initiator_only: bool,
    /// Supported protocol modes
    #[n(2)]
    pub modes: Vec<ProtocolMode>,
}

impl VersionData {
    /// Create version data for mainnet
    pub fn mainnet() -> Self {
        Self {
            network_magic: 764824073, // Cardano mainnet magic
            initiator_only: false,
            modes: vec![ProtocolMode::Duplex],
        }
    }

    /// Create version data for testnet
    pub fn testnet() -> Self {
        Self {
            network_magic: 1097911063, // Cardano testnet magic
            initiator_only: false,
            modes: vec![ProtocolMode::Duplex],
        }
    }
}

/// Protocol communication modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum ProtocolMode {
    /// Initiator only (outbound connections)
    #[n(0)]
    InitiatorOnly,
    /// Responder only (inbound connections)
    #[n(1)]
    ResponderOnly,
    /// Full duplex (both directions)
    #[n(2)]
    Duplex,
}
/// Handshake messages
#[derive(Debug, Clone, Encode, Decode)]
pub enum HandshakeMessage {
    /// Propose supported protocol versions
    #[n(0)]
    ProposeVersions {
        #[n(0)]
        versions: HashMap<ProtocolVersion, VersionData>,
    },
    /// Accept a proposed version
    #[n(1)]
    AcceptVersion {
        #[n(0)]
        version: ProtocolVersion,
        #[n(1)]
        version_data: VersionData,
    },
    /// Refuse handshake with reason
    #[n(2)]
    Refuse {
        #[n(0)]
        reason: RefuseReason,
    },
}

/// Reasons for handshake refusal
#[derive(Debug, Clone, Encode, Decode)]
pub enum RefuseReason {
    /// No compatible versions
    #[n(0)]
    VersionMismatch,
    /// Handshake decode error
    #[n(1)]
    HandshakeDecodeError(#[n(0)] String),
    /// Generic refusal
    #[n(2)]
    Refused(#[n(0)] String),
    /// Network magic mismatch
    #[n(3)]
    NetworkMismatch {
        #[n(0)]
        expected: u32,
        #[n(1)]
        received: u32,
    },
    /// Protocol mode incompatible
    #[n(4)]
    ModeIncompatible,
    /// Protocol violation
    #[n(5)]
    ProtocolViolation(#[n(0)] String),
}

impl std::fmt::Display for RefuseReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VersionMismatch => write!(f, "Version mismatch"),
            Self::HandshakeDecodeError(msg) => write!(f, "Decode error: {}", msg),
            Self::Refused(msg) => write!(f, "Refused: {}", msg),
            Self::NetworkMismatch { expected, received } => {
                write!(
                    f,
                    "Network mismatch: expected {}, received {}",
                    expected, received
                )
            }
            Self::ModeIncompatible => write!(f, "Protocol mode incompatible"),
            Self::ProtocolViolation(msg) => write!(f, "Protocol violation: {}", msg),
        }
    }
}

/// Handshake error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum HandshakeError {
    #[error("Handshake timeout")]
    Timeout,

    #[error("Version negotiation failed: {0}")]
    VersionNegotiationFailed(RefuseReason),

    #[error("Encoding error: {0}")]
    EncodingError(String),

    #[error("Decoding error: {0}")]
    DecodingError(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Protocol violation: {0}")]
    ProtocolViolation(String),

    #[error("Network mismatch")]
    NetworkMismatch,

    #[error("No supported versions")]
    NoSupportedVersions,
}

impl From<std::io::Error> for HandshakeError {
    fn from(error: std::io::Error) -> Self {
        Self::IoError(error.to_string())
    }
}

impl From<minicbor::encode::Error<std::io::Error>> for HandshakeError {
    fn from(error: minicbor::encode::Error<std::io::Error>) -> Self {
        Self::EncodingError(error.to_string())
    }
}

// Additional From implementation for other encode error types
impl From<minicbor::encode::Error<std::convert::Infallible>> for HandshakeError {
    fn from(error: minicbor::encode::Error<std::convert::Infallible>) -> Self {
        Self::EncodingError(format!("{:?}", error))
    }
}

impl From<minicbor::decode::Error> for HandshakeError {
    fn from(error: minicbor::decode::Error) -> Self {
        Self::DecodingError(error.to_string())
    }
}

/// Version negotiation result
#[derive(Debug, Clone)]
pub struct NegotiationResult {
    /// Agreed protocol version
    pub version: ProtocolVersion,
    /// Peer's version data
    pub peer_version_data: VersionData,
    /// Our version data
    pub local_version_data: VersionData,
    /// Handshake completion time
    pub handshake_duration: Duration,
}

/// Handshake protocol implementation
pub struct HandshakeProtocol {
    /// Supported versions and data
    supported_versions: HashMap<ProtocolVersion, VersionData>,
    /// Handshake timeout
    timeout: Duration,
}

impl HandshakeProtocol {
    /// Create new handshake protocol
    pub fn new() -> Self {
        let mut supported_versions = HashMap::new();
        supported_versions.insert(ProtocolVersion::CURRENT, VersionData::mainnet());

        Self {
            supported_versions,
            timeout: Duration::from_secs(30),
        }
    }

    /// Create handshake for testnet
    pub fn testnet() -> Self {
        let mut supported_versions = HashMap::new();
        supported_versions.insert(ProtocolVersion::CURRENT, VersionData::testnet());

        Self {
            supported_versions,
            timeout: Duration::from_secs(30),
        }
    }

    /// Add supported version
    pub fn add_version(&mut self, version: ProtocolVersion, data: VersionData) {
        self.supported_versions.insert(version, data);
    }

    /// Set handshake timeout
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Perform handshake as initiator
    pub async fn perform_handshake(
        &self,
        stream: &mut TcpStream,
    ) -> std::result::Result<u32, HandshakeError> {
        let start_time = Instant::now();

        // Send ProposeVersions
        let propose_msg = HandshakeMessage::ProposeVersions {
            versions: self.supported_versions.clone(),
        };

        self.send_message(stream, &propose_msg).await?;

        // Receive response
        let response = self.receive_message(stream).await?;

        match response {
            HandshakeMessage::AcceptVersion {
                version,
                version_data,
            } => {
                // Validate accepted version
                if let Some(our_data) = self.supported_versions.get(&version) {
                    // Check network compatibility
                    if our_data.network_magic != version_data.network_magic {
                        return Err(HandshakeError::NetworkMismatch);
                    }

                    // Check protocol mode compatibility
                    if !self.is_mode_compatible(&version_data.modes, &our_data.modes) {
                        let refuse_msg = HandshakeMessage::Refuse {
                            reason: RefuseReason::ModeIncompatible,
                        };
                        self.send_message(stream, &refuse_msg).await?;
                        return Err(HandshakeError::VersionNegotiationFailed(
                            RefuseReason::ModeIncompatible,
                        ));
                    }

                    let handshake_duration = start_time.elapsed();
                    debug!(
                        elapsed_ms = handshake_duration.as_millis(),
                        "Handshake completed with version {}.{}", version.major, version.minor
                    );
                    Ok(version.major as u32 * 1000 + version.minor as u32)
                } else {
                    // Version not in our supported list
                    let refuse_msg = HandshakeMessage::Refuse {
                        reason: RefuseReason::VersionMismatch,
                    };
                    self.send_message(stream, &refuse_msg).await?;
                    Err(HandshakeError::VersionNegotiationFailed(
                        RefuseReason::VersionMismatch,
                    ))
                }
            }
            HandshakeMessage::Refuse { reason } => {
                Err(HandshakeError::VersionNegotiationFailed(reason))
            }
            HandshakeMessage::ProposeVersions { .. } => {
                // Received proposal when we sent proposal - protocol violation
                Err(HandshakeError::ProtocolViolation(
                    "Received ProposeVersions in response to ProposeVersions".to_string(),
                ))
            }
        }
    }

    /// Handle handshake as responder
    pub async fn handle_handshake(
        &self,
        stream: &mut TcpStream,
    ) -> std::result::Result<NegotiationResult, HandshakeError> {
        let start_time = Instant::now();

        // Receive ProposeVersions
        let proposal = self.receive_message(stream).await?;

        match proposal {
            HandshakeMessage::ProposeVersions { versions } => {
                // Find best compatible version
                if let Some((best_version, peer_data)) = self.select_best_version(&versions)? {
                    // Get our version data safely
                    let our_data = self
                        .supported_versions
                        .get(&best_version)
                        .ok_or_else(|| {
                            HandshakeError::ProtocolViolation(format!(
                                "Best version {} not found in supported versions (internal error)",
                                best_version
                            ))
                        })?
                        .clone();

                    // Send AcceptVersion
                    let accept_msg = HandshakeMessage::AcceptVersion {
                        version: best_version,
                        version_data: our_data.clone(),
                    };

                    self.send_message(stream, &accept_msg).await?;

                    let handshake_duration = start_time.elapsed();

                    Ok(NegotiationResult {
                        version: best_version,
                        peer_version_data: peer_data,
                        local_version_data: our_data,
                        handshake_duration,
                    })
                } else {
                    // No compatible versions
                    let refuse_msg = HandshakeMessage::Refuse {
                        reason: RefuseReason::VersionMismatch,
                    };
                    self.send_message(stream, &refuse_msg).await?;
                    Err(HandshakeError::NoSupportedVersions)
                }
            }
            _ => {
                // Invalid initial message
                let refuse_msg = HandshakeMessage::Refuse {
                    reason: RefuseReason::ProtocolViolation(
                        "Expected ProposeVersions as first message".to_string(),
                    ),
                };
                self.send_message(stream, &refuse_msg).await?;
                Err(HandshakeError::ProtocolViolation(
                    "Expected ProposeVersions as first message".to_string(),
                ))
            }
        }
    }

    /// Send handshake message
    async fn send_message(
        &self,
        writer: &mut TcpStream,
        message: &HandshakeMessage,
    ) -> std::result::Result<(), HandshakeError> {
        // Encode message to CBOR
        let encoded_data = minicbor::to_vec(message).map_err(HandshakeError::from)?;

        // Frame format: [length: u32][data: bytes]
        let mut frame = BytesMut::with_capacity(4 + encoded_data.len());
        frame.put_u32(encoded_data.len() as u32);
        frame.put_slice(&encoded_data);

        writer.write_all(&frame).await?;
        writer.flush().await?;
        Ok(())
    }

    /// Receive handshake message
    async fn receive_message(
        &self,
        reader: &mut TcpStream,
    ) -> std::result::Result<HandshakeMessage, HandshakeError> {
        // Read frame length
        let mut length_buf = [0u8; 4];
        reader.read_exact(&mut length_buf).await?;
        let message_length = u32::from_be_bytes(length_buf) as usize;

        // Validate message length
        if message_length > 64 * 1024 {
            return Err(HandshakeError::ProtocolViolation(format!(
                "Message too large: {} bytes",
                message_length
            )));
        }

        // Read message data
        let mut message_buf = vec![0u8; message_length];
        reader.read_exact(&mut message_buf).await?;

        // Decode CBOR message
        let message = minicbor::decode(&message_buf)
            .map_err(|e| HandshakeError::DecodingError(e.to_string()))?;

        Ok(message)
    }

    /// Select best compatible version from proposals
    fn select_best_version(
        &self,
        proposed_versions: &HashMap<ProtocolVersion, VersionData>,
    ) -> std::result::Result<Option<(ProtocolVersion, VersionData)>, HandshakeError> {
        let mut compatible_versions = Vec::new();

        // Find all compatible versions
        for (proposed_version, proposed_data) in proposed_versions {
            if let Some(our_data) = self.supported_versions.get(proposed_version) {
                // Check network compatibility
                if our_data.network_magic == proposed_data.network_magic {
                    // Check mode compatibility
                    if self.is_mode_compatible(&proposed_data.modes, &our_data.modes) {
                        compatible_versions.push((*proposed_version, proposed_data.clone()));
                    }
                }
            }
        }

        // Select highest version
        compatible_versions.sort_by_key(|(version, _)| *version);
        Ok(compatible_versions.pop())
    }

    /// Check if protocol modes are compatible
    pub fn is_mode_compatible(
        &self,
        peer_modes: &[ProtocolMode],
        our_modes: &[ProtocolMode],
    ) -> bool {
        for peer_mode in peer_modes {
            for our_mode in our_modes {
                match (peer_mode, our_mode) {
                    (ProtocolMode::Duplex, _) => return true,
                    (_, ProtocolMode::Duplex) => return true,
                    (ProtocolMode::InitiatorOnly, ProtocolMode::ResponderOnly) => return true,
                    (ProtocolMode::ResponderOnly, ProtocolMode::InitiatorOnly) => return true,
                    _ => continue,
                }
            }
        }
        false
    }
}

impl Default for HandshakeProtocol {
    fn default() -> Self {
        Self::new()
    }
}

/// Version negotiation utilities
pub struct VersionNegotiation;

impl VersionNegotiation {
    /// Check if version is supported
    pub fn is_version_supported(version: &ProtocolVersion) -> bool {
        version >= &ProtocolVersion::MINIMUM && version <= &ProtocolVersion::CURRENT
    }

    /// Get supported version range
    pub fn supported_range() -> (ProtocolVersion, ProtocolVersion) {
        (ProtocolVersion::MINIMUM, ProtocolVersion::CURRENT)
    }

    /// Create version proposal for network
    pub fn create_proposal(network_magic: u32) -> HashMap<ProtocolVersion, VersionData> {
        let mut versions = HashMap::new();

        let version_data = VersionData {
            network_magic,
            initiator_only: false,
            modes: vec![ProtocolMode::Duplex],
        };

        versions.insert(ProtocolVersion::CURRENT, version_data);
        versions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert!(v1_0.is_compatible_with(&v1_1));
        assert!(!v1_0.is_compatible_with(&v2_0));
    }

    #[test]
    fn test_version_data_creation() {
        let mainnet = VersionData::mainnet();
        let testnet = VersionData::testnet();

        assert_eq!(mainnet.network_magic, 764824073);
        assert_eq!(testnet.network_magic, 1097911063);
        assert!(!mainnet.initiator_only);
        assert!(!testnet.initiator_only);
    }

    #[test]
    fn test_mode_compatibility() {
        let handshake = HandshakeProtocol::new();

        // Duplex modes are always compatible
        assert!(
            handshake.is_mode_compatible(&[ProtocolMode::Duplex], &[ProtocolMode::InitiatorOnly])
        );

        // Complementary modes are compatible
        assert!(handshake.is_mode_compatible(
            &[ProtocolMode::InitiatorOnly],
            &[ProtocolMode::ResponderOnly]
        ));

        // Same unidirectional modes are not compatible
        assert!(!handshake.is_mode_compatible(
            &[ProtocolMode::InitiatorOnly],
            &[ProtocolMode::InitiatorOnly]
        ));
    }

    #[test]
    fn test_handshake_message_encoding() {
        let versions = VersionNegotiation::create_proposal(764824073);
        let message = HandshakeMessage::ProposeVersions { versions };

        // Test that we can encode and decode the message
        let encoded = minicbor::to_vec(&message).unwrap();
        let decoded: HandshakeMessage = minicbor::decode(&encoded).unwrap();

        match decoded {
            HandshakeMessage::ProposeVersions { versions } => {
                assert!(versions.contains_key(&ProtocolVersion::CURRENT));
            }
            _ => panic!("Unexpected message type"),
        }
    }

    #[test]
    fn test_refuse_reason_display() {
        let reason = RefuseReason::NetworkMismatch {
            expected: 764824073,
            received: 1097911063,
        };

        let display = format!("{}", reason);
        assert!(display.contains("Network mismatch"));
        assert!(display.contains("764824073"));
        assert!(display.contains("1097911063"));
    }
}
