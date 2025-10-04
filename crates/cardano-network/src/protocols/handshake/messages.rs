//! Handshake Protocol Messages
//!
//! Defines the message types exchanged during the handshake protocol.

use super::types::{NodeToNodeVersion, NodeToNodeVersionData, VersionTable};
use std::fmt;

/// Handshake protocol messages
#[derive(Debug, Clone, PartialEq)]
pub enum HandshakeMessage {
    /// Client proposes supported versions with their version data
    ///
    /// Sent by the client to initiate the handshake. Contains a map of all
    /// protocol versions the client supports, along with the version-specific
    /// data for each.
    MsgProposeVersions {
        /// Map of version → version data for all supported versions
        versions: VersionTable,
    },

    /// Server replies with its supported versions (simultaneous open)
    ///
    /// In case of simultaneous TCP open (both sides initiate connection),
    /// the server also sends its version table. Both sides will then
    /// independently select the same version using a symmetric algorithm.
    MsgReplyVersions {
        /// Map of version → version data for server's supported versions
        versions: VersionTable,
    },

    /// Server accepts a specific version
    ///
    /// The server has selected a version from the client's proposed versions
    /// and provides its version-specific data for that version.
    MsgAcceptVersion {
        /// The selected protocol version
        version: NodeToNodeVersion,
        /// Server's version data for the selected version
        version_data: NodeToNodeVersionData,
    },

    /// Server refuses the handshake
    ///
    /// The server could not find a compatible version or the version data
    /// is incompatible. The connection will be closed after this message.
    MsgRefuse {
        /// Reason for refusal
        reason: RefuseReason,
    },

    /// Response to version query (for network topology queries)
    ///
    /// When query mode is enabled, the server responds with its full version
    /// table without establishing a connection.
    MsgQueryReply {
        /// Map of version → version data for all supported versions
        versions: VersionTable,
    },
}

impl HandshakeMessage {
    /// Create a MsgProposeVersions message with given version table
    pub fn propose_versions(versions: VersionTable) -> Self {
        Self::MsgProposeVersions { versions }
    }

    /// Create a MsgAcceptVersion message
    pub fn accept_version(version: NodeToNodeVersion, version_data: NodeToNodeVersionData) -> Self {
        Self::MsgAcceptVersion {
            version,
            version_data,
        }
    }

    /// Create a MsgRefuse message
    pub fn refuse(reason: RefuseReason) -> Self {
        Self::MsgRefuse { reason }
    }

    /// Get message name for logging
    pub fn name(&self) -> &'static str {
        match self {
            Self::MsgProposeVersions { .. } => "MsgProposeVersions",
            Self::MsgReplyVersions { .. } => "MsgReplyVersions",
            Self::MsgAcceptVersion { .. } => "MsgAcceptVersion",
            Self::MsgRefuse { .. } => "MsgRefuse",
            Self::MsgQueryReply { .. } => "MsgQueryReply",
        }
    }
}

impl fmt::Display for HandshakeMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MsgProposeVersions { versions } => {
                write!(f, "MsgProposeVersions({} versions)", versions.len())
            }
            Self::MsgReplyVersions { versions } => {
                write!(f, "MsgReplyVersions({} versions)", versions.len())
            }
            Self::MsgAcceptVersion {
                version,
                version_data,
            } => {
                write!(f, "MsgAcceptVersion({}, {})", version, version_data)
            }
            Self::MsgRefuse { reason } => {
                write!(f, "MsgRefuse({})", reason)
            }
            Self::MsgQueryReply { versions } => {
                write!(f, "MsgQueryReply({} versions)", versions.len())
            }
        }
    }
}

/// Reasons for handshake refusal
#[derive(Debug, Clone, PartialEq)]
pub enum RefuseReason {
    /// No compatible protocol versions found
    ///
    /// The client and server don't share any common protocol versions.
    /// Includes the versions proposed by the client and any version tags
    /// the server couldn't decode.
    VersionMismatch {
        /// Versions the client proposed
        proposed: Vec<NodeToNodeVersion>,
        /// Version tags the server couldn't decode
        unknown_tags: Vec<i64>,
    },

    /// Failed to decode version data for a specific version
    ///
    /// The server recognizes the version number but couldn't decode
    /// the associated version data.
    HandshakeDecodeError {
        /// The version that failed to decode
        version: NodeToNodeVersion,
        /// Error message describing the decode failure
        error: String,
    },

    /// Server explicitly refuses the proposed version/version data
    ///
    /// The version and version data are valid, but the server refuses
    /// to accept them for some application-specific reason (e.g.,
    /// network magic mismatch, incompatible modes).
    Refused {
        /// The version that was refused
        version: NodeToNodeVersion,
        /// Reason for refusal
        reason: String,
    },
}

impl RefuseReason {
    /// Create a version mismatch reason
    pub fn version_mismatch(
        proposed: Vec<NodeToNodeVersion>,
        unknown_tags: Vec<i64>,
    ) -> Self {
        Self::VersionMismatch {
            proposed,
            unknown_tags,
        }
    }

    /// Create a handshake decode error reason
    pub fn handshake_decode_error(version: NodeToNodeVersion, error: impl Into<String>) -> Self {
        Self::HandshakeDecodeError {
            version,
            error: error.into(),
        }
    }

    /// Create a refused reason
    pub fn refused(version: NodeToNodeVersion, reason: impl Into<String>) -> Self {
        Self::Refused {
            version,
            reason: reason.into(),
        }
    }
}

impl fmt::Display for RefuseReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VersionMismatch {
                proposed,
                unknown_tags,
            } => {
                write!(
                    f,
                    "VersionMismatch(proposed: {:?}, unknown: {:?})",
                    proposed, unknown_tags
                )
            }
            Self::HandshakeDecodeError { version, error } => {
                write!(f, "HandshakeDecodeError({}: {})", version, error)
            }
            Self::Refused { version, reason } => {
                write!(f, "Refused({}: {})", version, reason)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_names() {
        let versions = VersionTable::new();
        assert_eq!(
            HandshakeMessage::propose_versions(versions.clone()).name(),
            "MsgProposeVersions"
        );
        assert_eq!(
            HandshakeMessage::accept_version(
                NodeToNodeVersion::V15,
                NodeToNodeVersionData::preview_testnet()
            )
            .name(),
            "MsgAcceptVersion"
        );
        assert_eq!(
            HandshakeMessage::refuse(RefuseReason::version_mismatch(vec![], vec![])).name(),
            "MsgRefuse"
        );
    }

    #[test]
    fn test_refuse_reason_creation() {
        let reason = RefuseReason::version_mismatch(
            vec![NodeToNodeVersion::V14],
            vec![1, 2, 3],
        );
        match reason {
            RefuseReason::VersionMismatch {
                proposed,
                unknown_tags,
            } => {
                assert_eq!(proposed, vec![NodeToNodeVersion::V14]);
                assert_eq!(unknown_tags, vec![1, 2, 3]);
            }
            _ => panic!("Wrong variant"),
        }
    }
}
