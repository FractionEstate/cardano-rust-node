//! CardanoDB - Pure Rust storage engine for Cardano
//!
//! This module implements a specialized, high-performance storage solution
//! for the Cardano blockchain that eliminates C/C++ dependencies while
//! maintaining 100% compatibility with the official Haskell cardano-node.
//!
//! # Architecture
//!
//! CardanoDB uses a three-database design matching the official Haskell node:
//!
//! - **ImmutableDB**: Chunk-based storage for ancient, immutable blocks
//! - **VolatileDB**: Ring buffer for recent blocks (last k blocks)
//! - **LedgerDB**: In-memory ledger state with periodic disk snapshots
//!
//! # Example
//!
//! ```no_run
//! use cardano_storage::cardanodb::{CardanoDB, CardanoDBConfig};
//! use std::path::PathBuf;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Create configuration for mainnet
//! let config = CardanoDBConfig::mainnet(PathBuf::from("/data/cardano"));
//!
//! // Open CardanoDB
//! let db = CardanoDB::open(config).await?;
//!
//! // Store and retrieve blocks...
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod immutable;
pub mod ledger;
pub mod types;
pub mod volatile;

#[cfg(test)]
mod tests;

use anyhow::Result;
pub use config::{CardanoDBConfig, ImmutableDBConfig, LedgerDBConfig, VolatileDBConfig};
pub use immutable::ImmutableDB;
pub use ledger::LedgerDB;
pub use types::{Blake2b256Hash, BlockLocation, BlockNo, ChunkNo, EpochNo, ImmutableTip, SlotNo};
pub use volatile::VolatileDB;

/// CardanoDB - Pure Rust storage engine for Cardano
///
/// This is the main entry point for the CardanoDB storage system, coordinating
/// three separate databases: ImmutableDB, VolatileDB, and LedgerDB.
pub struct CardanoDB {
    /// ImmutableDB: Chunk-based storage for ancient blocks
    immutable: ImmutableDB,

    /// VolatileDB: Ring buffer for recent blocks
    volatile: VolatileDB,

    /// LedgerDB: In-memory ledger with disk snapshots
    ledger: LedgerDB,

    /// Configuration
    config: CardanoDBConfig,
}

impl CardanoDB {
    /// Open or create a new CardanoDB instance
    pub async fn open(config: CardanoDBConfig) -> Result<Self> {
        // Create base directory
        std::fs::create_dir_all(&config.base_path)?;

        // Open each database
        let immutable = ImmutableDB::open(config.immutable.clone())?;
        let volatile = VolatileDB::new(config.volatile.clone());
        let ledger = LedgerDB::new(config.ledger.clone())?;

        Ok(Self {
            immutable,
            volatile,
            ledger,
            config,
        })
    }

    /// Get the current tip of the immutable chain
    pub async fn get_immutable_tip(&self) -> Option<ImmutableTip> {
        self.immutable.get_tip().await
    }

    /// Get the current block number from the ledger
    pub async fn get_ledger_block_no(&self) -> BlockNo {
        self.ledger.get_block_no().await
    }

    /// Get the number of blocks currently in VolatileDB
    pub async fn get_volatile_block_count(&self) -> usize {
        self.volatile.block_count().await
    }

    /// Get a reference to the configuration
    pub fn config(&self) -> &CardanoDBConfig {
        &self.config
    }

    // TODO: Add block storage/retrieval methods in Phase 2
    // pub async fn store_block(&self, block: Block) -> Result<()> { ... }
    // pub async fn get_block(&self, hash: &Blake2b256Hash) -> Result<Option<Vec<u8>>> { ... }
    // pub async fn rollback_to(&self, block_no: BlockNo) -> Result<()> { ... }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn cardanodb_can_be_opened() {
        let dir = tempdir().unwrap();
        let config = CardanoDBConfig::new(dir.path().to_path_buf());

        let db = CardanoDB::open(config).await.unwrap();

        // Check initial state
        assert!(db.get_immutable_tip().await.is_none());
        assert_eq!(db.get_ledger_block_no().await, BlockNo(0));
        assert_eq!(db.get_volatile_block_count().await, 0);
    }

    #[tokio::test]
    async fn cardanodb_creates_directory_structure() {
        let dir = tempdir().unwrap();
        let base = dir.path().to_path_buf();
        let config = CardanoDBConfig::new(base.clone());

        let _db = CardanoDB::open(config).await.unwrap();

        // Verify directories were created
        assert!(base.exists());
        assert!(base.join("immutable").exists());
        assert!(base.join("ledger").exists());
    }

    #[tokio::test]
    async fn cardanodb_mainnet_config() {
        let dir = tempdir().unwrap();
        let config = CardanoDBConfig::mainnet(dir.path().to_path_buf());

        assert_eq!(config.immutable.chunk_size, 21600); // One epoch
        assert_eq!(config.volatile.k, 2160); // Security parameter
        assert!(config.immutable.enable_compression);

        let _db = CardanoDB::open(config).await.unwrap();
    }

    #[tokio::test]
    async fn cardanodb_preview_config() {
        let dir = tempdir().unwrap();
        let config = CardanoDBConfig::preview(dir.path().to_path_buf());

        assert_eq!(config.immutable.chunk_size, 7200); // Shorter for testing
        assert_eq!(config.volatile.k, 720); // Smaller k

        let _db = CardanoDB::open(config).await.unwrap();
    }
}
