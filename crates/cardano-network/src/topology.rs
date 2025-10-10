//! Network Topology Configuration
//!
//! This module provides parsing and management of network topology configuration
//! files used by Cardano nodes to discover and connect to peers.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::Path;
use tokio::fs;

/// Network topology configuration
///
/// Defines the set of peers a node should connect to, including bootstrap peers,
/// local trusted peers, and public relay nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopologyConfig {
    /// Bootstrap peers for initial network entry
    #[serde(default)]
    pub bootstrap_peers: Vec<AccessPoint>,

    /// Local trusted peers with guaranteed connections
    #[serde(default)]
    pub local_roots: Vec<LocalRoot>,

    /// Public relay nodes for general connectivity
    #[serde(default)]
    pub public_roots: Vec<PublicRoot>,

    /// Slot number after which to use ledger peer discovery
    #[serde(default)]
    pub use_ledger_after_slot: u64,
}

/// A peer access point (domain or IP with port)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AccessPoint {
    /// Hostname or IP address
    pub address: String,
    /// TCP port
    pub port: u16,
}

/// Local trusted peer root configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalRoot {
    /// List of access points for this local root
    pub access_points: Vec<AccessPoint>,
    /// Whether to advertise these peers to others
    pub advertise: bool,
    /// Whether these peers are trustable
    pub trustable: bool,
    /// Number of connections to maintain (valency)
    pub valency: u32,
}

/// Public relay root configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicRoot {
    /// List of access points for this public root
    pub access_points: Vec<AccessPoint>,
    /// Whether to advertise these peers to others
    pub advertise: bool,
}

impl TopologyConfig {
    /// Load topology configuration from a JSON file
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, TopologyError> {
        let contents = fs::read_to_string(path)
            .await
            .map_err(TopologyError::IoError)?;

        Self::from_json(&contents)
    }

    /// Parse topology configuration from JSON string
    pub fn from_json(json: &str) -> Result<Self, TopologyError> {
        serde_json::from_str(json).map_err(TopologyError::ParseError)
    }

    /// Get all access points from all roots
    pub fn all_access_points(&self) -> Vec<&AccessPoint> {
        let mut points = Vec::new();

        // Add bootstrap peers
        points.extend(self.bootstrap_peers.iter());

        // Add local roots
        for root in &self.local_roots {
            points.extend(root.access_points.iter());
        }

        // Add public roots
        for root in &self.public_roots {
            points.extend(root.access_points.iter());
        }

        points
    }

    /// Get local root access points (trusted peers)
    pub fn local_access_points(&self) -> Vec<&AccessPoint> {
        self.local_roots
            .iter()
            .flat_map(|root| root.access_points.iter())
            .collect()
    }

    /// Get public root access points (relay nodes)
    pub fn public_access_points(&self) -> Vec<&AccessPoint> {
        self.public_roots
            .iter()
            .flat_map(|root| root.access_points.iter())
            .collect()
    }

    /// Get total valency (number of connections to maintain)
    pub fn total_valency(&self) -> u32 {
        self.local_roots.iter().map(|root| root.valency).sum()
    }
}

impl AccessPoint {
    /// Create a new access point
    pub fn new(address: String, port: u16) -> Self {
        Self { address, port }
    }

    /// Check if this is a domain name (not an IP address)
    pub fn is_domain(&self) -> bool {
        // Simple heuristic: if it contains letters, it's likely a domain
        self.address.chars().any(|c| c.is_alphabetic())
    }

    /// Get the address as a string for display
    pub fn to_string(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}

/// Topology configuration errors
#[derive(Debug, thiserror::Error)]
pub enum TopologyError {
    #[error("Failed to read topology file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to parse topology JSON: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Invalid topology: {0}")]
    Invalid(String),
}

/// Resolved peer with socket address
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedPeer {
    /// Original access point
    pub access_point: AccessPoint,
    /// Resolved socket address
    pub socket_addr: SocketAddr,
    /// Whether this peer is trustable
    pub trustable: bool,
}

impl ResolvedPeer {
    pub fn new(access_point: AccessPoint, socket_addr: SocketAddr, trustable: bool) -> Self {
        Self {
            access_point,
            socket_addr,
            trustable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_preview_topology() {
        let json = r#"{
            "bootstrapPeers": [],
            "localRoots": [
                {
                    "accessPoints": [
                        {
                            "address": "preview-node.world.dev.cardano.org",
                            "port": 30002
                        },
                        {
                            "address": "preview-node.play.dev.cardano.org",
                            "port": 3001
                        }
                    ],
                    "advertise": false,
                    "trustable": true,
                    "valency": 2
                }
            ],
            "publicRoots": [
                {
                    "accessPoints": [
                        {
                            "address": "relays-new.cardano-preview.iohk.io",
                            "port": 3001
                        }
                    ],
                    "advertise": false
                }
            ],
            "useLedgerAfterSlot": 0
        }"#;

        let config = TopologyConfig::from_json(json).unwrap();

        assert_eq!(config.bootstrap_peers.len(), 0);
        assert_eq!(config.local_roots.len(), 1);
        assert_eq!(config.public_roots.len(), 1);
        assert_eq!(config.total_valency(), 2);

        let local_root = &config.local_roots[0];
        assert_eq!(local_root.access_points.len(), 2);
        assert!(local_root.trustable);
        assert_eq!(local_root.valency, 2);

        let all_points = config.all_access_points();
        assert_eq!(all_points.len(), 3);
    }

    #[test]
    fn test_access_point_is_domain() {
        let domain = AccessPoint::new("preview-node.world.dev.cardano.org".to_string(), 30002);
        assert!(domain.is_domain());

        let ip = AccessPoint::new("192.168.1.1".to_string(), 3001);
        assert!(!ip.is_domain());
    }

    #[test]
    fn test_access_point_to_string() {
        let point = AccessPoint::new("example.com".to_string(), 8080);
        assert_eq!(point.to_string(), "example.com:8080");
    }
}
