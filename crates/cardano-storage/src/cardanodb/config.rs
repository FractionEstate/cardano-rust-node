//! Configuration for CardanoDB
//!
//! This module defines the configuration options for the three-database
//! CardanoDB system (ImmutableDB + VolatileDB + LedgerDB).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration for CardanoDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardanoDBConfig {
    /// Base directory for all databases
    pub base_path: PathBuf,

    /// Configuration for ImmutableDB
    pub immutable: ImmutableDBConfig,

    /// Configuration for VolatileDB
    pub volatile: VolatileDBConfig,

    /// Configuration for LedgerDB
    pub ledger: LedgerDBConfig,
}

impl CardanoDBConfig {
    /// Create a new configuration with default values
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            immutable: ImmutableDBConfig {
                path: base_path.join("immutable"),
                chunk_size: 21600, // One epoch (21600 slots)
                enable_compression: true,
                max_cached_chunks: 100, // Keep up to 100 chunks in cache
            },
            volatile: VolatileDBConfig {
                k: 2160, // Security parameter (2160 blocks)
            },
            ledger: LedgerDBConfig {
                path: base_path.join("ledger"),
                snapshot_interval: 100, // Snapshot every 100 blocks
                snapshot_retention: 10, // Keep last 10 snapshots
            },
            base_path,
        }
    }

    /// Create configuration for mainnet
    pub fn mainnet(base_path: PathBuf) -> Self {
        Self::new(base_path)
    }

    /// Create configuration for testnet
    pub fn testnet(base_path: PathBuf) -> Self {
        let mut config = Self::new(base_path);
        // Testnet may use shorter epochs
        config.immutable.chunk_size = 10800; // Half epoch
        config
    }

    /// Create configuration for preview network
    pub fn preview(base_path: PathBuf) -> Self {
        let mut config = Self::new(base_path);
        config.immutable.chunk_size = 7200; // Shorter chunks for testing
        config.volatile.k = 720; // Smaller k for faster testing
        config
    }
}

/// Configuration for ImmutableDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImmutableDBConfig {
    /// Directory for chunk files
    pub path: PathBuf,

    /// Number of slots per chunk (typically one epoch)
    /// Mainnet: 21600 slots (5 days)
    /// Testnet: May vary
    pub chunk_size: u32,

    /// Enable zstd compression for finalized chunks
    pub enable_compression: bool,

    /// Maximum number of chunks to keep in cache
    /// Default: 100 chunks (~2.1M blocks for mainnet)
    /// Set to 0 for unlimited cache
    #[serde(default = "default_max_cached_chunks")]
    pub max_cached_chunks: usize,
}

fn default_max_cached_chunks() -> usize {
    100
}

/// Configuration for VolatileDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatileDBConfig {
    /// Security parameter k - number of recent blocks to keep
    /// Mainnet: 2160 blocks (~1 day)
    /// This is the rollback window
    pub k: u64,
}

/// Configuration for LedgerDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerDBConfig {
    /// Directory for ledger snapshots
    pub path: PathBuf,

    /// Take a snapshot every N blocks
    /// Smaller = more frequent snapshots = faster rollback but more disk I/O
    /// Larger = less frequent snapshots = slower rollback but less disk I/O
    pub snapshot_interval: u32,

    /// Number of snapshots to retain
    /// Should be at least k / snapshot_interval to cover the rollback window
    pub snapshot_retention: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn default_config_has_reasonable_values() {
        let config = CardanoDBConfig::new(PathBuf::from("/tmp/cardano"));

        // Check immutable config
        assert_eq!(config.immutable.chunk_size, 21600);
        assert!(config.immutable.enable_compression);
        assert_eq!(config.immutable.path, Path::new("/tmp/cardano/immutable"));

        // Check volatile config
        assert_eq!(config.volatile.k, 2160);

        // Check ledger config
        assert_eq!(config.ledger.snapshot_interval, 100);
        assert_eq!(config.ledger.snapshot_retention, 10);
        assert_eq!(config.ledger.path, Path::new("/tmp/cardano/ledger"));
    }

    #[test]
    fn preview_config_has_smaller_values() {
        let config = CardanoDBConfig::preview(PathBuf::from("/tmp/cardano"));

        assert_eq!(config.immutable.chunk_size, 7200);
        assert_eq!(config.volatile.k, 720);
    }

    #[test]
    fn config_serialization_roundtrip() {
        let config = CardanoDBConfig::new(PathBuf::from("/tmp/test"));
        let json = serde_json::to_string(&config).unwrap();
        let decoded: CardanoDBConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.immutable.chunk_size, decoded.immutable.chunk_size);
        assert_eq!(config.volatile.k, decoded.volatile.k);
    }
}
