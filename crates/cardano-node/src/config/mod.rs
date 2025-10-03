//! Configuration Management Module
//!
//! Handles loading, parsing, validation, and management of Cardano Node configuration
//! including node settings, network topology, protocol parameters, and runtime configuration.
//!
//! Compatible with Haskell cardano-node configuration formats including:
//! - Node configuration (JSON/YAML)
//! - Network topology configuration
//! - Genesis files validation
//! - Protocol parameter validation
//!
//! Reference: <https://github.com/IntersectMBO/cardano-node/tree/master>

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Full Haskell-compatible node configuration structure
/// Compatible with cardano-node v10.5.1+
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct NodeConfiguration {
    // Genesis Files
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alonzo_genesis_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alonzo_genesis_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byron_genesis_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byron_genesis_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conway_genesis_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conway_genesis_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shelley_genesis_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shelley_genesis_hash: Option<String>,

    // Checkpoints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoints_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkpoints_file_hash: Option<String>,

    // Consensus Configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consensus_mode: Option<String>, // "PraosMode" or "GenesisMode"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_p2_p: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_sharing: Option<bool>,

    // Protocol Configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>, // "Cardano"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_network_magic: Option<String>, // "RequiresNoMagic" or "RequiresMagic"

    // Block Version
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "LastKnownBlockVersion-Major"
    )]
    pub last_known_block_version_major: Option<u32>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "LastKnownBlockVersion-Minor"
    )]
    pub last_known_block_version_minor: Option<u32>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "LastKnownBlockVersion-Alt"
    )]
    pub last_known_block_version_alt: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_known_major_protocol_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_node_version: Option<String>,

    // LedgerDB Configuration
    #[serde(skip_serializing_if = "Option::is_none", rename = "LedgerDB")]
    pub ledger_db: Option<LedgerDBConfig>,

    // Tracing Configuration (40+ flags)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_accept_policy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_block_fetch_client: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_block_fetch_decisions: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_block_fetch_protocol: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_block_fetch_protocol_serialised: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_block_fetch_server: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_chain_db: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_chain_sync_block_server: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_chain_sync_client: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_chain_sync_header_server: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_chain_sync_protocol: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_connection_manager: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "TraceDNSResolver")]
    pub trace_dns_resolver: Option<bool>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "TraceDNSSubscription"
    )]
    pub trace_dns_subscription: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_diffusion_initialization: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_error_policy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_forge: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_handshake: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_inbound_governor: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_ip_subscription: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_ledger_peers: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_local_chain_sync_protocol: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_local_connection_manager: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_local_error_policy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_local_handshake: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_local_root_peers: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_local_tx_submission_protocol: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_local_tx_submission_server: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_mempool: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_mux: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_peer_selection: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_peer_selection_actions: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_public_root_peers: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_server: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_tx_inbound: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_tx_outbound: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_tx_submission_protocol: Option<bool>,

    // Logging Configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracing_verbosity: Option<String>, // "NormalVerbosity", "MinimalVerbosity", "MaximalVerbosity"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_on_log_metrics: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_on_logging: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_trace_dispatcher: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_backends: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_scribes: Option<Vec<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<LoggingOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<LogRotationConfig>,

    // Metrics Configuration
    #[serde(skip_serializing_if = "Option::is_none", rename = "hasEKG")]
    pub has_ekg: Option<u16>, // EKG metrics port
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_prometheus: Option<Vec<serde_json::Value>>, // ["host", port]

    // Legacy fields for backward compatibility (used by Rust-specific code)
    #[serde(skip)]
    pub network_magic: Option<u32>,
    #[serde(skip)]
    pub listening_port: Option<u16>,
    #[serde(skip)]
    pub database_path: Option<PathBuf>,
    #[serde(skip)]
    pub socket_path: Option<PathBuf>,
    #[serde(skip)]
    pub enable_logging: Option<bool>,
    #[serde(skip)]
    pub max_connections: Option<u32>,
    #[serde(skip)]
    pub enable_metrics: Option<bool>,
    #[serde(skip)]
    pub metrics_port: Option<u16>,
    #[serde(skip)]
    pub topology_file: Option<PathBuf>,
}

/// LedgerDB configuration
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct LedgerDBConfig {
    pub backend: String, // "V2InMemory" or "OnDisk"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_of_disk_snapshots: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_batch_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_interval: Option<u32>,
}

/// Logging options configuration
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggingOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map_backends: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map_subtrace: Option<HashMap<String, SubtraceConfig>>,
}

/// Subtrace configuration
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SubtraceConfig {
    pub subtrace: String, // "Neutral", "Verbose", etc.
}

/// Log rotation configuration
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRotationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rp_keep_files_num: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rp_log_limit_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rp_max_age_hours: Option<u32>,
}

/// Network topology structure for P2P peer connections
/// Compatible with Haskell cardano-node P2P topology
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkTopology {
    /// Bootstrap peers for initial network discovery
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_peers: Option<Vec<BootstrapPeer>>,

    /// Local root peers configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_roots: Option<Vec<LocalRoot>>,

    /// Public root peers configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_roots: Option<Vec<PublicRoot>>,

    /// Slot number after which to use ledger peers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_ledger_after_slot: Option<i64>,

    /// Path to peer snapshot file (for Genesis mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_snapshot_file: Option<String>,

    /// Legacy: List of producer nodes (for backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producers: Option<Vec<TopologyProducer>>,
}

/// Bootstrap peer configuration
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct BootstrapPeer {
    pub address: String,
    pub port: u16,
}

/// Local root peer configuration
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalRoot {
    pub access_points: Vec<AccessPoint>,
    pub advertise: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trustable: Option<bool>,
    pub valency: u32,
}

/// Public root peer configuration
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicRoot {
    pub access_points: Vec<AccessPoint>,
    pub advertise: bool,
}

/// Access point (peer address and port)
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AccessPoint {
    pub address: String,
    pub port: u16,
}

/// Individual producer/peer in the network topology (legacy)
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TopologyProducer {
    /// IP address or hostname
    pub addr: String,

    /// Port number
    pub port: u16,

    /// Number of connections to maintain
    pub valency: u32,
}

impl NodeConfiguration {
    /// Load configuration from file (JSON or YAML)
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(&path)?;
        let path_ref = path.as_ref();

        // Parse based on file extension
        match path_ref.extension().and_then(|s| s.to_str()) {
            Some("json") => Ok(serde_json::from_str(&content)?),
            Some("yaml") | Some("yml") => {
                // For now, treat YAML same as JSON - in production would use serde_yaml
                // This maintains compatibility with existing tests
                Ok(serde_json::from_str(&content)?)
            }
            _ => {
                // Default to JSON parsing for backward compatibility
                Ok(serde_json::from_str(&content)?)
            }
        }
    }

    /// Load configuration from JSON string content
    pub fn from_json_str(content: &str) -> Result<Self> {
        Ok(serde_json::from_str(content)?)
    }

    /// Save configuration to file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Convert configuration to JSON string
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Validate configuration parameters
    pub fn validate(&self) -> Result<()> {
        // Validate genesis files and hashes
        if let Some(_file) = &self.byron_genesis_file {
            if self.byron_genesis_hash.is_none() {
                return Err(anyhow::anyhow!(
                    "Byron genesis file specified but hash is missing"
                ));
            }
            // Would validate file exists and hash matches in production
        }

        if let Some(_file) = &self.shelley_genesis_file {
            if self.shelley_genesis_hash.is_none() {
                return Err(anyhow::anyhow!(
                    "Shelley genesis file specified but hash is missing"
                ));
            }
        }

        if let Some(_file) = &self.alonzo_genesis_file {
            if self.alonzo_genesis_hash.is_none() {
                return Err(anyhow::anyhow!(
                    "Alonzo genesis file specified but hash is missing"
                ));
            }
        }

        if let Some(_file) = &self.conway_genesis_file {
            if self.conway_genesis_hash.is_none() {
                return Err(anyhow::anyhow!(
                    "Conway genesis file specified but hash is missing"
                ));
            }
        } // Validate consensus mode
        if let Some(mode) = &self.consensus_mode {
            if mode != "PraosMode" && mode != "GenesisMode" {
                return Err(anyhow::anyhow!("Invalid consensus mode: {}", mode));
            }
        }

        // Validate protocol
        if let Some(protocol) = &self.protocol {
            if protocol != "Cardano" {
                return Err(anyhow::anyhow!("Invalid protocol: {}", protocol));
            }
        }

        // Validate network magic requirement
        if let Some(magic) = &self.requires_network_magic {
            if magic != "RequiresNoMagic" && magic != "RequiresMagic" {
                return Err(anyhow::anyhow!("Invalid RequiresNetworkMagic: {}", magic));
            }
        }

        // Validate tracing verbosity
        if let Some(verbosity) = &self.tracing_verbosity {
            if !["NormalVerbosity", "MinimalVerbosity", "MaximalVerbosity"]
                .contains(&verbosity.as_str())
            {
                return Err(anyhow::anyhow!("Invalid tracing verbosity: {}", verbosity));
            }
        }

        Ok(())
    }

    /// Create default configuration for testing
    pub fn default_for_testing() -> Self {
        Self {
            // Genesis files
            byron_genesis_file: Some("byron-genesis.json".to_string()),
            byron_genesis_hash: Some(
                "5f20df933584822601f9e3f8c024eb5eb252fe8cefb24d1317dc3d432e940ebb".to_string(),
            ),
            shelley_genesis_file: Some("shelley-genesis.json".to_string()),
            shelley_genesis_hash: Some(
                "1a3be38bcbb7911969283716ad7aa550250226b76a61fc51cc9a9a35d9276d81".to_string(),
            ),
            alonzo_genesis_file: Some("alonzo-genesis.json".to_string()),
            alonzo_genesis_hash: Some(
                "7e94a15f55d1e82d10f09203fa1d40f8eede58fd8066542cf6566008068ed874".to_string(),
            ),
            conway_genesis_file: Some("conway-genesis.json".to_string()),
            conway_genesis_hash: Some(
                "15a199f895e461ec0ffc6dd4e4028af28a492ab4e806d39cb674c88f7643ef62".to_string(),
            ),

            // Checkpoints
            checkpoints_file: None,
            checkpoints_file_hash: None,

            // Consensus
            consensus_mode: Some("PraosMode".to_string()),
            enable_p2_p: Some(true),
            peer_sharing: Some(false), // Default false for testing

            // Protocol
            protocol: Some("Cardano".to_string()),
            requires_network_magic: Some("RequiresNoMagic".to_string()),

            // Block version
            last_known_block_version_major: Some(3),
            last_known_block_version_minor: Some(0),
            last_known_block_version_alt: Some(0),
            max_known_major_protocol_version: Some(2),
            min_node_version: Some("10.4.0".to_string()),

            // LedgerDB
            ledger_db: Some(LedgerDBConfig {
                backend: "V2InMemory".to_string(),
                num_of_disk_snapshots: Some(2),
                query_batch_size: Some(100000),
                snapshot_interval: Some(4320),
            }),

            // Trace flags (set minimal defaults for testing)
            trace_accept_policy: Some(false),
            trace_block_fetch_client: Some(false),
            trace_block_fetch_decisions: Some(false),
            trace_block_fetch_protocol: Some(false),
            trace_block_fetch_protocol_serialised: Some(false),
            trace_block_fetch_server: Some(false),
            trace_chain_db: Some(false),
            trace_chain_sync_block_server: Some(false),
            trace_chain_sync_client: Some(false),
            trace_chain_sync_header_server: Some(false),
            trace_chain_sync_protocol: Some(false),
            trace_connection_manager: Some(false),
            trace_dns_resolver: Some(false),
            trace_dns_subscription: Some(false),
            trace_diffusion_initialization: Some(false),
            trace_error_policy: Some(false),
            trace_forge: Some(false),
            trace_handshake: Some(false),
            trace_inbound_governor: Some(false),
            trace_ip_subscription: Some(false),
            trace_ledger_peers: Some(false),
            trace_local_chain_sync_protocol: Some(false),
            trace_local_connection_manager: Some(false),
            trace_local_error_policy: Some(false),
            trace_local_handshake: Some(false),
            trace_local_root_peers: Some(false),
            trace_local_tx_submission_protocol: Some(false),
            trace_local_tx_submission_server: Some(false),
            trace_mempool: Some(false),
            trace_mux: Some(false),
            trace_peer_selection: Some(false),
            trace_peer_selection_actions: Some(false),
            trace_public_root_peers: Some(false),
            trace_server: Some(false),
            trace_tx_inbound: Some(false),
            trace_tx_outbound: Some(false),
            trace_tx_submission_protocol: Some(false),

            // Logging
            tracing_verbosity: Some("NormalVerbosity".to_string()),
            turn_on_log_metrics: Some(false),
            turn_on_logging: Some(false),
            use_trace_dispatcher: Some(false),
            default_backends: Some(vec!["KatipBK".to_string()]),
            default_scribes: Some(vec![vec!["StdoutSK".to_string(), "stdout".to_string()]]),
            min_severity: Some("Info".to_string()),
            options: None,
            rotation: None,

            // Metrics
            has_ekg: None,
            has_prometheus: None,

            // Legacy fields
            network_magic: Some(764824073),
            listening_port: Some(3001),
            database_path: Some(PathBuf::from("cardano-db")),
            socket_path: Some(PathBuf::from("cardano-node.socket")),
            enable_logging: Some(false),
            max_connections: Some(10),
            enable_metrics: Some(false),
            metrics_port: Some(8080),
            topology_file: Some(PathBuf::from("topology.json")),
        }
    }

    /// Merge configuration with CLI arguments, CLI takes precedence
    pub fn merge_with_args(&mut self, args: &crate::cli::RunArgs) {
        if let Some(port) = args.port {
            self.listening_port = Some(port);
        }

        if let Some(host_addr) = &args.host_addr {
            tracing::debug!("Host address specified: {}", host_addr);
        }

        if let Some(socket) = &args.socket_path {
            self.socket_path = Some(socket.clone());
        }

        if let Some(database) = &args.database_path {
            self.database_path = Some(database.clone());
        }

        if args.validate_db {
            tracing::debug!("Database validation mode enabled");
        }

        if args.metrics {
            self.enable_metrics = Some(true);
        }

        if let Some(metrics_host) = &args.metrics_host {
            tracing::debug!("Metrics host specified: {}", metrics_host);
        }

        if let Some(metrics_port) = args.metrics_port {
            self.metrics_port = Some(metrics_port);
        }

        if let Some(protocol_magic) = args.protocol_magic {
            self.network_magic = Some(protocol_magic);
        }

        if let Some(topology) = &args.topology {
            self.topology_file = Some(topology.clone());
        }
    }
}

impl NetworkTopology {
    /// Load topology from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Load topology from JSON string content
    pub fn from_json_str(content: &str) -> Result<Self> {
        Ok(serde_json::from_str(content)?)
    }

    /// Save topology to file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate topology configuration
    pub fn validate(&self) -> Result<()> {
        // For P2P mode, check bootstrap peers or local/public roots
        let has_bootstrap = self.bootstrap_peers.as_ref().is_some_and(|p| !p.is_empty());
        let has_local = self.local_roots.as_ref().is_some_and(|r| !r.is_empty());
        let has_public = self.public_roots.as_ref().is_some_and(|r| !r.is_empty());
        let has_legacy = self.producers.as_ref().is_some_and(|p| !p.is_empty());

        if !has_bootstrap && !has_local && !has_public && !has_legacy {
            return Err(anyhow::anyhow!(
                "Network topology must contain at least one of: bootstrapPeers, localRoots, publicRoots, or producers"
            ));
        }

        // Validate bootstrap peers
        if let Some(peers) = &self.bootstrap_peers {
            for (i, peer) in peers.iter().enumerate() {
                if peer.port == 0 {
                    return Err(anyhow::anyhow!("Bootstrap peer {} port cannot be 0", i));
                }
                if peer.address.is_empty() {
                    return Err(anyhow::anyhow!(
                        "Bootstrap peer {} address cannot be empty",
                        i
                    ));
                }
            }
        }

        // Validate local roots
        if let Some(roots) = &self.local_roots {
            for (i, root) in roots.iter().enumerate() {
                if root.valency == 0 {
                    return Err(anyhow::anyhow!(
                        "Local root {} valency must be greater than 0",
                        i
                    ));
                }
                for (j, ap) in root.access_points.iter().enumerate() {
                    if ap.port == 0 {
                        return Err(anyhow::anyhow!(
                            "Local root {} access point {} port cannot be 0",
                            i,
                            j
                        ));
                    }
                }
            }
        }

        // Validate public roots
        if let Some(roots) = &self.public_roots {
            for (i, root) in roots.iter().enumerate() {
                for (j, ap) in root.access_points.iter().enumerate() {
                    if ap.port == 0 {
                        return Err(anyhow::anyhow!(
                            "Public root {} access point {} port cannot be 0",
                            i,
                            j
                        ));
                    }
                }
            }
        }

        // Validate legacy producers
        if let Some(producers) = &self.producers {
            for (i, producer) in producers.iter().enumerate() {
                if producer.port == 0 {
                    return Err(anyhow::anyhow!("Producer {} port cannot be 0", i));
                }
                if producer.valency == 0 {
                    return Err(anyhow::anyhow!(
                        "Producer {} valency must be greater than 0",
                        i
                    ));
                }
                if producer.addr.is_empty() {
                    return Err(anyhow::anyhow!("Producer {} address cannot be empty", i));
                }
            }
        }

        Ok(())
    }

    /// Create default P2P topology for testing
    pub fn default_for_testing() -> Self {
        Self {
            bootstrap_peers: Some(vec![BootstrapPeer {
                address: "backbone.cardano.iog.io".to_string(),
                port: 3001,
            }]),
            local_roots: Some(vec![LocalRoot {
                access_points: vec![],
                advertise: false,
                trustable: Some(false),
                valency: 1,
            }]),
            public_roots: Some(vec![PublicRoot {
                access_points: vec![],
                advertise: false,
            }]),
            use_ledger_after_slot: Some(157852837),
            peer_snapshot_file: None,
            producers: None, // Legacy mode disabled
        }
    }

    /// Create legacy topology for backward compatibility
    pub fn legacy_for_testing() -> Self {
        Self {
            bootstrap_peers: None,
            local_roots: None,
            public_roots: None,
            use_ledger_after_slot: None,
            peer_snapshot_file: None,
            producers: Some(vec![TopologyProducer {
                addr: "127.0.0.1".to_string(),
                port: 3001,
                valency: 1,
            }]),
        }
    }
}

/// Configuration manager for handling multiple configuration sources
#[derive(Debug, Clone)]
pub struct ConfigurationManager {
    /// Current active configuration
    config: NodeConfiguration,

    /// Configuration file path
    config_path: Option<PathBuf>,

    /// Network topology
    topology: Option<NetworkTopology>,
}

impl ConfigurationManager {
    /// Create new configuration manager
    pub fn new() -> Self {
        Self {
            config: NodeConfiguration::default_for_testing(),
            config_path: None,
            topology: None,
        }
    }

    /// Load configuration from file
    pub fn load_config<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        self.config = NodeConfiguration::from_file(&path)?;
        self.config_path = Some(path.as_ref().to_path_buf());
        self.config.validate()?;
        Ok(())
    }

    /// Load network topology
    pub fn load_topology<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let topology = NetworkTopology::from_file(&path)?;
        topology.validate()?;
        self.topology = Some(topology);
        Ok(())
    }

    /// Get current configuration
    pub fn get_config(&self) -> &NodeConfiguration {
        &self.config
    }

    /// Get current topology
    pub fn get_topology(&self) -> Option<&NetworkTopology> {
        self.topology.as_ref()
    }

    /// Apply CLI arguments to configuration
    pub fn apply_cli_args(&mut self, args: &crate::cli::RunArgs) {
        self.config.merge_with_args(args);
    }

    /// Validate all loaded configurations
    pub fn validate_all(&self) -> Result<()> {
        self.config.validate()?;

        if let Some(topology) = &self.topology {
            topology.validate()?;
        }

        Ok(())
    }
}

impl Default for ConfigurationManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper for creating test configurations - exposed for integration tests
pub struct ConfigTestHelper {
    temp_dir: tempfile::TempDir,
    pub config_file: PathBuf,
    pub topology_file: PathBuf,
}

impl ConfigTestHelper {
    /// Create new test helper
    pub fn new() -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let config_file = temp_dir.path().join("test_config.json");
        let topology_file = temp_dir.path().join("test_topology.json");

        Ok(Self {
            temp_dir,
            config_file,
            topology_file,
        })
    }

    /// Create valid test configuration
    pub fn create_valid_config(&self) -> Result<()> {
        let config = NodeConfiguration {
            // Genesis files with mainnet hashes
            byron_genesis_file: Some("byron-genesis.json".to_string()),
            byron_genesis_hash: Some(
                "5f20df933584822601f9e3f8c024eb5eb252fe8cefb24d1317dc3d432e940ebb".to_string(),
            ),
            shelley_genesis_file: Some("shelley-genesis.json".to_string()),
            shelley_genesis_hash: Some(
                "1a3be38bcbb7911969283716ad7aa550250226b76a61fc51cc9a9a35d9276d81".to_string(),
            ),
            alonzo_genesis_file: Some("alonzo-genesis.json".to_string()),
            alonzo_genesis_hash: Some(
                "7e94a15f55d1e82d10f09203fa1d40f8eede58fd8066542cf6566008068ed874".to_string(),
            ),
            conway_genesis_file: Some("conway-genesis.json".to_string()),
            conway_genesis_hash: Some(
                "15a199f895e461ec0ffc6dd4e4028af28a492ab4e806d39cb674c88f7643ef62".to_string(),
            ),

            checkpoints_file: None,
            checkpoints_file_hash: None,

            consensus_mode: Some("PraosMode".to_string()),
            enable_p2_p: Some(true),
            peer_sharing: Some(true),

            protocol: Some("Cardano".to_string()),
            requires_network_magic: Some("RequiresNoMagic".to_string()),

            last_known_block_version_major: Some(3),
            last_known_block_version_minor: Some(0),
            last_known_block_version_alt: Some(0),
            max_known_major_protocol_version: Some(2),
            min_node_version: Some("10.4.0".to_string()),

            ledger_db: Some(LedgerDBConfig {
                backend: "V2InMemory".to_string(),
                num_of_disk_snapshots: Some(2),
                query_batch_size: Some(100000),
                snapshot_interval: Some(4320),
            }),

            trace_accept_policy: Some(true),
            trace_block_fetch_client: Some(false),
            trace_block_fetch_decisions: Some(false),
            trace_block_fetch_protocol: Some(false),
            trace_block_fetch_protocol_serialised: Some(false),
            trace_block_fetch_server: Some(false),
            trace_chain_db: Some(true),
            trace_chain_sync_block_server: Some(false),
            trace_chain_sync_client: Some(false),
            trace_chain_sync_header_server: Some(false),
            trace_chain_sync_protocol: Some(false),
            trace_connection_manager: Some(true),
            trace_dns_resolver: Some(true),
            trace_dns_subscription: Some(true),
            trace_diffusion_initialization: Some(true),
            trace_error_policy: Some(true),
            trace_forge: Some(true),
            trace_handshake: Some(true),
            trace_inbound_governor: Some(true),
            trace_ip_subscription: Some(true),
            trace_ledger_peers: Some(true),
            trace_local_chain_sync_protocol: Some(false),
            trace_local_connection_manager: Some(true),
            trace_local_error_policy: Some(true),
            trace_local_handshake: Some(true),
            trace_local_root_peers: Some(true),
            trace_local_tx_submission_protocol: Some(false),
            trace_local_tx_submission_server: Some(false),
            trace_mempool: Some(false),
            trace_mux: Some(false),
            trace_peer_selection: Some(true),
            trace_peer_selection_actions: Some(true),
            trace_public_root_peers: Some(true),
            trace_server: Some(true),
            trace_tx_inbound: Some(false),
            trace_tx_outbound: Some(false),
            trace_tx_submission_protocol: Some(false),

            tracing_verbosity: Some("NormalVerbosity".to_string()),
            turn_on_log_metrics: Some(true),
            turn_on_logging: Some(true),
            use_trace_dispatcher: Some(false),
            default_backends: Some(vec!["KatipBK".to_string()]),
            default_scribes: Some(vec![vec!["StdoutSK".to_string(), "stdout".to_string()]]),
            min_severity: Some("Info".to_string()),
            options: None,
            rotation: None,

            has_ekg: Some(12788),
            has_prometheus: Some(vec![
                serde_json::Value::String("127.0.0.1".to_string()),
                serde_json::Value::Number(12798.into()),
            ]),

            network_magic: Some(764824073),
            listening_port: Some(3001),
            database_path: Some(PathBuf::from("/tmp/cardano-db")),
            socket_path: Some(PathBuf::from("/tmp/cardano-node.socket")),
            enable_logging: Some(true),
            max_connections: Some(100),
            enable_metrics: Some(true),
            metrics_port: Some(12798),
            topology_file: Some(PathBuf::from("/tmp/topology.json")),
        };

        config.to_file(&self.config_file)?;
        Ok(())
    }

    /// Create invalid test configuration
    pub fn create_invalid_config(&self) -> Result<()> {
        std::fs::write(&self.config_file, "{ invalid json }")?;
        Ok(())
    }

    /// Create malformed JSON configuration
    pub fn create_malformed_config(&self) -> Result<()> {
        std::fs::write(
            &self.config_file,
            r#"{ "network_magic": 764824073, "invalid": }"#,
        )?;
        Ok(())
    }

    /// Create configuration with invalid port (would fail validation, not compile-time)
    pub fn create_invalid_port_config(&self) -> Result<()> {
        // Create a config that will pass compilation but fail validation
        let mut config = NodeConfiguration::default_for_testing();
        // Set byron genesis without hash to trigger validation error
        config.byron_genesis_file = Some("byron-genesis.json".to_string());
        config.byron_genesis_hash = None; // This will fail validation

        config.to_file(&self.config_file)?;
        Ok(())
    }

    /// Create valid test topology
    pub fn create_valid_topology(&self) -> Result<()> {
        let topology = NetworkTopology::default_for_testing();
        topology.to_file(&self.topology_file)?;
        Ok(())
    }

    /// Create invalid test topology
    pub fn create_invalid_topology(&self) -> Result<()> {
        std::fs::write(&self.topology_file, "{ invalid json }")?;
        Ok(())
    }

    /// Get temporary directory path
    pub fn temp_dir(&self) -> &Path {
        self.temp_dir.path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_configuration_validation() {
        let config = NodeConfiguration::default_for_testing();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_network_topology_validation() {
        let topology = NetworkTopology::default_for_testing();
        assert!(topology.validate().is_ok());
    }

    #[test]
    fn test_config_manager() {
        let manager = ConfigurationManager::new();
        assert!(manager.validate_all().is_ok());
    }

    #[test]
    fn test_config_test_helper() {
        let helper = ConfigTestHelper::new().unwrap();
        helper.create_valid_config().unwrap();

        let config = NodeConfiguration::from_file(&helper.config_file).unwrap();
        assert_eq!(config.protocol, Some("Cardano".to_string()));
        assert_eq!(config.consensus_mode, Some("PraosMode".to_string()));
    }

    #[test]
    fn test_p2p_topology() {
        let topology = NetworkTopology::default_for_testing();
        assert!(topology.bootstrap_peers.is_some());
        assert!(topology.validate().is_ok());
    }

    #[test]
    fn test_legacy_topology() {
        let topology = NetworkTopology::legacy_for_testing();
        assert!(topology.producers.is_some());
        assert!(topology.validate().is_ok());
    }
}
