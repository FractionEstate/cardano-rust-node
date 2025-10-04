//! Handshake Protocol Types
//!
//! Defines version numbers, version data, and network parameters for the
//! node-to-node handshake protocol.

use std::collections::HashMap;
use std::fmt;

/// Node-to-node protocol version
///
/// Represents the supported protocol versions for Cardano node-to-node communication.
/// Each version may introduce new features, change message formats, or modify protocol behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeToNodeVersion {
    /// Version 14 - Conway era base version
    V14 = 14,
    /// Version 15 - Latest with SRV support (required for modern testnets)
    V15 = 15,
}

impl NodeToNodeVersion {
    /// Get all supported versions in ascending order
    pub fn all_supported() -> Vec<Self> {
        vec![Self::V14, Self::V15]
    }

    /// Get the highest supported version
    pub fn latest() -> Self {
        Self::V15
    }

    /// Get the minimum required version
    pub fn minimum() -> Self {
        Self::V14
    }

    /// Convert from integer tag
    pub fn from_tag(tag: i64) -> Option<Self> {
        match tag {
            14 => Some(Self::V14),
            15 => Some(Self::V15),
            _ => None,
        }
    }

    /// Convert to integer tag for CBOR encoding
    pub fn to_tag(self) -> i64 {
        self as i64
    }
}

impl fmt::Display for NodeToNodeVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeToNodeV_{}", self.to_tag())
    }
}

/// Network magic number identifying the Cardano network
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetworkMagic(pub u32);

impl NetworkMagic {
    /// Cardano mainnet magic number
    pub const MAINNET: Self = Self(764824073);

    /// Cardano preview testnet magic number (SanchoNet/Conway era)
    pub const PREVIEW_TESTNET: Self = Self(2);

    /// Cardano preprod testnet magic number
    pub const PREPROD_TESTNET: Self = Self(1);

    /// Create a new network magic
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Get the magic number value
    pub const fn value(self) -> u32 {
        self.0
    }

    /// Check if this is mainnet
    pub fn is_mainnet(self) -> bool {
        self == Self::MAINNET
    }

    /// Check if this is a testnet
    pub fn is_testnet(self) -> bool {
        self == Self::PREVIEW_TESTNET || self == Self::PREPROD_TESTNET
    }

    /// Get network name for display
    pub fn network_name(self) -> &'static str {
        match self {
            Self::MAINNET => "mainnet",
            Self::PREVIEW_TESTNET => "preview-testnet",
            Self::PREPROD_TESTNET => "preprod-testnet",
            _ => "unknown",
        }
    }
}

impl fmt::Display for NetworkMagic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.network_name(), self.0)
    }
}

/// Diffusion mode for network operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffusionMode {
    /// Node can both initiate and accept connections
    InitiatorAndResponder,
    /// Node can only initiate connections (e.g., behind NAT)
    InitiatorOnly,
}

impl DiffusionMode {
    /// Convert to CBOR representation (bool)
    pub fn to_cbor_bool(self) -> bool {
        match self {
            Self::InitiatorAndResponder => false,
            Self::InitiatorOnly => true,
        }
    }

    /// Convert from CBOR representation (bool)
    pub fn from_cbor_bool(value: bool) -> Self {
        if value {
            Self::InitiatorOnly
        } else {
            Self::InitiatorAndResponder
        }
    }
}

impl fmt::Display for DiffusionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InitiatorAndResponder => write!(f, "InitiatorAndResponder"),
            Self::InitiatorOnly => write!(f, "InitiatorOnly"),
        }
    }
}

/// Peer sharing capability
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerSharing {
    /// Peer sharing is disabled
    Disabled,
    /// Peer sharing is enabled (node can share peer addresses)
    Enabled,
}

impl PeerSharing {
    /// Convert to CBOR representation (int)
    pub fn to_cbor_int(self) -> i64 {
        match self {
            Self::Disabled => 0,
            Self::Enabled => 1,
        }
    }

    /// Convert from CBOR representation (int)
    pub fn from_cbor_int(value: i64) -> Option<Self> {
        match value {
            0 => Some(Self::Disabled),
            1 => Some(Self::Enabled),
            _ => None,
        }
    }
}

impl fmt::Display for PeerSharing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => write!(f, "PeerSharingDisabled"),
            Self::Enabled => write!(f, "PeerSharingEnabled"),
        }
    }
}

/// Version-specific data exchanged during handshake
///
/// Contains network parameters and node capabilities that must be negotiated
/// and agreed upon before any mini-protocols can communicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeToNodeVersionData {
    /// Network magic number (mainnet/testnet identifier)
    pub network_magic: NetworkMagic,
    /// Diffusion mode (can accept connections or not)
    pub diffusion_mode: DiffusionMode,
    /// Peer sharing capability
    pub peer_sharing: PeerSharing,
    /// Query mode (used for network topology queries)
    pub query: bool,
}

impl NodeToNodeVersionData {
    /// Create version data for mainnet with default settings
    pub fn mainnet() -> Self {
        Self {
            network_magic: NetworkMagic::MAINNET,
            diffusion_mode: DiffusionMode::InitiatorAndResponder,
            peer_sharing: PeerSharing::Disabled,
            query: false,
        }
    }

    /// Create version data for preview testnet with default settings
    pub fn preview_testnet() -> Self {
        Self {
            network_magic: NetworkMagic::PREVIEW_TESTNET,
            diffusion_mode: DiffusionMode::InitiatorAndResponder,
            peer_sharing: PeerSharing::Enabled, // Preview testnet has peer sharing enabled
            query: false,
        }
    }

    /// Create version data for preprod testnet with default settings
    pub fn preprod_testnet() -> Self {
        Self {
            network_magic: NetworkMagic::PREPROD_TESTNET,
            diffusion_mode: DiffusionMode::InitiatorAndResponder,
            peer_sharing: PeerSharing::Disabled,
            query: false,
        }
    }

    /// Create custom version data
    pub fn new(
        network_magic: NetworkMagic,
        diffusion_mode: DiffusionMode,
        peer_sharing: PeerSharing,
        query: bool,
    ) -> Self {
        Self {
            network_magic,
            diffusion_mode,
            peer_sharing,
            query,
        }
    }

    /// Validate that this version data is compatible with remote version data
    ///
    /// Returns Ok(negotiated_data) if compatible, Err otherwise.
    pub fn negotiate_with(&self, remote: &Self) -> Result<Self, String> {
        // Network magic must match exactly
        if self.network_magic != remote.network_magic {
            return Err(format!(
                "Network magic mismatch: local={}, remote={}",
                self.network_magic, remote.network_magic
            ));
        }

        // Negotiate diffusion mode (both must support the agreed mode)
        let negotiated_diffusion = match (self.diffusion_mode, remote.diffusion_mode) {
            (DiffusionMode::InitiatorAndResponder, DiffusionMode::InitiatorAndResponder) => {
                DiffusionMode::InitiatorAndResponder
            }
            _ => DiffusionMode::InitiatorOnly,
        };

        // Peer sharing is enabled if both support it
        let negotiated_peer_sharing = match (self.peer_sharing, remote.peer_sharing) {
            (PeerSharing::Enabled, PeerSharing::Enabled) => PeerSharing::Enabled,
            _ => PeerSharing::Disabled,
        };

        // Query mode is enabled if either side requests it
        let negotiated_query = self.query || remote.query;

        Ok(Self {
            network_magic: self.network_magic,
            diffusion_mode: negotiated_diffusion,
            peer_sharing: negotiated_peer_sharing,
            query: negotiated_query,
        })
    }
}

impl fmt::Display for NodeToNodeVersionData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "VersionData(network={}, diffusion={}, peer_sharing={}, query={})",
            self.network_magic, self.diffusion_mode, self.peer_sharing, self.query
        )
    }
}

/// A map of supported versions with their associated version data
pub type VersionTable = HashMap<NodeToNodeVersion, NodeToNodeVersionData>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_ordering() {
        assert!(NodeToNodeVersion::V14 < NodeToNodeVersion::V15);
        assert_eq!(NodeToNodeVersion::latest(), NodeToNodeVersion::V15);
    }

    #[test]
    fn test_version_conversion() {
        assert_eq!(
            NodeToNodeVersion::from_tag(14),
            Some(NodeToNodeVersion::V14)
        );
        assert_eq!(
            NodeToNodeVersion::from_tag(15),
            Some(NodeToNodeVersion::V15)
        );
        assert_eq!(NodeToNodeVersion::from_tag(99), None);
    }

    #[test]
    fn test_network_magic() {
        assert!(NetworkMagic::MAINNET.is_mainnet());
        assert!(!NetworkMagic::MAINNET.is_testnet());
        assert!(NetworkMagic::PREVIEW_TESTNET.is_testnet());
        assert_eq!(NetworkMagic::MAINNET.network_name(), "mainnet");
    }

    #[test]
    fn test_version_data_negotiation() {
        let local = NodeToNodeVersionData::preview_testnet();
        let remote = NodeToNodeVersionData::preview_testnet();

        let result = local.negotiate_with(&remote);
        assert!(result.is_ok());

        let negotiated = result.unwrap();
        assert_eq!(negotiated.network_magic, NetworkMagic::PREVIEW_TESTNET);
    }

    #[test]
    fn test_version_data_network_mismatch() {
        let local = NodeToNodeVersionData::mainnet();
        let remote = NodeToNodeVersionData::preview_testnet();

        let result = local.negotiate_with(&remote);
        assert!(result.is_err());
    }

    #[test]
    fn test_diffusion_mode_negotiation() {
        let mut local = NodeToNodeVersionData::preview_testnet();
        local.diffusion_mode = DiffusionMode::InitiatorOnly;

        let remote = NodeToNodeVersionData::preview_testnet();

        let negotiated = local.negotiate_with(&remote).unwrap();
        assert_eq!(negotiated.diffusion_mode, DiffusionMode::InitiatorOnly);
    }

    #[test]
    fn test_peer_sharing_negotiation() {
        let mut local = NodeToNodeVersionData::preview_testnet();
        local.peer_sharing = PeerSharing::Enabled;

        let mut remote = NodeToNodeVersionData::preview_testnet();
        remote.peer_sharing = PeerSharing::Enabled;

        let negotiated = local.negotiate_with(&remote).unwrap();
        assert_eq!(negotiated.peer_sharing, PeerSharing::Enabled);

        // One disabled means negotiated is disabled
        remote.peer_sharing = PeerSharing::Disabled;
        let negotiated = local.negotiate_with(&remote).unwrap();
        assert_eq!(negotiated.peer_sharing, PeerSharing::Disabled);
    }
}
