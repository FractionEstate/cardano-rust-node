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
pub mod migration;
pub mod types;
pub mod volatile;

#[cfg(test)]
mod tests;

use anyhow::Result;
pub use config::{CardanoDBConfig, ImmutableDBConfig, LedgerDBConfig, VolatileDBConfig};
pub use immutable::ImmutableDB;
pub use ledger::{LedgerDB, LedgerState};
pub use migration::{BlockMigrationService, MigrationConfig, MigrationStats};
use std::sync::Arc;
pub use types::{
    Blake2b256Hash, BlockLocation, BlockNo, ChainTip, ChunkNo, EpochNo, ImmutableTip, SlotNo,
};
pub use volatile::VolatileDB;

/// CardanoDB - Pure Rust storage engine for Cardano
///
/// This is the main entry point for the CardanoDB storage system, coordinating
/// three separate databases: ImmutableDB, VolatileDB, and LedgerDB.
pub struct CardanoDB {
    /// ImmutableDB: Chunk-based storage for ancient blocks
    immutable: Arc<ImmutableDB>,

    /// VolatileDB: Ring buffer for recent blocks
    volatile: Arc<VolatileDB>,

    /// LedgerDB: In-memory ledger with disk snapshots
    ledger: LedgerDB,

    /// Block migration service
    migration_service: Option<Arc<BlockMigrationService>>,

    /// Configuration
    config: CardanoDBConfig,
}

impl CardanoDB {
    /// Open or create a new CardanoDB instance
    pub async fn open(config: CardanoDBConfig) -> Result<Self> {
        Self::open_with_migration(config, None).await
    }

    /// Open CardanoDB with custom migration configuration
    pub async fn open_with_migration(
        config: CardanoDBConfig,
        migration_config: Option<MigrationConfig>,
    ) -> Result<Self> {
        // Create base directory
        std::fs::create_dir_all(&config.base_path)?;

        // Open each database
        let immutable = Arc::new(ImmutableDB::open(config.immutable.clone())?);
        let volatile = Arc::new(VolatileDB::new(config.volatile.clone()));
        let ledger = LedgerDB::new(config.ledger.clone())?;

        // Create migration service if enabled
        let migration_service = migration_config.map(|mc| {
            Arc::new(BlockMigrationService::new(
                volatile.clone(),
                immutable.clone(),
                mc,
            ))
        });

        Ok(Self {
            immutable,
            volatile,
            ledger,
            migration_service,
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

    /// Get the current chain tip (latest block)
    pub async fn get_chain_tip(&self) -> Result<Option<ChainTip>> {
        // First check VolatileDB for the most recent blocks
        if let Some(tip) = self.volatile.get_tip().await {
            return Ok(Some(tip));
        }

        // Fallback to ImmutableDB if VolatileDB is empty
        if let Some(immutable_tip) = self.get_immutable_tip().await {
            return Ok(Some(ChainTip {
                block_hash: immutable_tip.hash,
                block_no: immutable_tip.block_no,
                slot_no: immutable_tip.slot_no,
            }));
        }

        Ok(None)
    }

    /// Get the number of blocks currently in VolatileDB
    pub async fn get_volatile_block_count(&self) -> usize {
        self.volatile.block_count().await
    }

    /// Start the block migration service if configured
    pub fn start_migration(&self) -> Option<tokio::task::JoinHandle<()>> {
        self.migration_service
            .as_ref()
            .map(|service| service.clone().start())
    }

    /// Trigger a manual migration cycle (useful for testing)
    pub async fn trigger_migration(&self) -> Result<()> {
        if let Some(service) = &self.migration_service {
            service.trigger_migration().await?;
        }
        Ok(())
    }

    /// Get migration statistics
    pub async fn get_migration_stats(&self) -> Option<MigrationStats> {
        if let Some(service) = &self.migration_service {
            Some(service.stats().await)
        } else {
            None
        }
    }

    /// Stop the migration service gracefully
    pub fn stop_migration(&self) -> Result<()> {
        if let Some(service) = &self.migration_service {
            service.stop()?;
        }
        Ok(())
    }

    /// Get the current ledger state
    ///
    /// Returns a complete snapshot of the current ledger state including
    /// UTxO set, slot, block number, and epoch. Used by block production
    /// and validation services.
    pub async fn get_ledger_state(&self) -> ledger::LedgerState {
        self.ledger.get_current_state().await
    }

    /// Get a reference to the configuration
    pub fn config(&self) -> &CardanoDBConfig {
        &self.config
    }

    /// Store a block in the database
    ///
    /// This method determines where to store the block:
    /// - Recent blocks (within k of tip) go to VolatileDB
    /// - Older blocks go to ImmutableDB
    ///
    /// # Arguments
    /// * `hash` - Blake2b-256 hash of the block
    /// * `slot_no` - Slot number of the block
    /// * `block_no` - Block number
    /// * `data` - Raw block data (CBOR-encoded)
    pub async fn put_block(
        &self,
        hash: Blake2b256Hash,
        slot_no: SlotNo,
        block_no: BlockNo,
        data: Vec<u8>,
    ) -> Result<()> {
        // For now, add to VolatileDB
        // TODO: Implement logic to move old blocks from volatile to immutable
        self.volatile.add_block(hash, slot_no, block_no, data).await
    }

    /// Retrieve a block by its hash
    ///
    /// Searches both VolatileDB and ImmutableDB for the block.
    ///
    /// # Arguments
    /// * `hash` - Blake2b-256 hash of the block to retrieve
    ///
    /// # Returns
    /// * `Ok(Some(data))` - Block found, returns raw CBOR-encoded data
    /// * `Ok(None)` - Block not found
    /// * `Err(...)` - Database error
    pub async fn get_block(&self, hash: &Blake2b256Hash) -> Result<Option<Vec<u8>>> {
        // Check VolatileDB first (most recent blocks)
        if let Some(data) = self.volatile.get_block(hash).await {
            return Ok(Some(data));
        }

        // Check ImmutableDB for older blocks
        self.immutable.get_block_by_hash(hash).await
    }

    /// Retrieve a block by its slot number
    ///
    /// # Arguments
    /// * `slot_no` - Slot number of the block to retrieve
    ///
    /// # Returns
    /// * `Ok(Some(data))` - Block found, returns raw CBOR-encoded data
    /// * `Ok(None)` - Block not found
    /// * `Err(...)` - Database error
    pub async fn get_block_by_slot(&self, slot_no: SlotNo) -> Result<Option<Vec<u8>>> {
        // Check VolatileDB first
        if let Some(data) = self.volatile.get_block_by_slot(&slot_no).await {
            return Ok(Some(data));
        }

        // Check ImmutableDB
        self.immutable.get_block_by_slot(slot_no).await
    }

    // TODO: Add block storage/retrieval methods in Phase 2
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

    #[tokio::test]
    async fn cardanodb_put_and_get_block() {
        let dir = tempdir().unwrap();
        let config = CardanoDBConfig::new(dir.path().to_path_buf());
        let db = CardanoDB::open(config).await.unwrap();

        // Create test block
        let hash = Blake2b256Hash::new([42u8; 32]);
        let slot = SlotNo(1000);
        let block_no = BlockNo(500);
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];

        // Store block
        db.put_block(hash, slot, block_no, data.clone())
            .await
            .unwrap();

        // Retrieve by hash
        let retrieved = db.get_block(&hash).await.unwrap();
        assert_eq!(retrieved, Some(data.clone()));

        // Retrieve by slot
        let retrieved_by_slot = db.get_block_by_slot(slot).await.unwrap();
        assert_eq!(retrieved_by_slot, Some(data));
    }

    #[tokio::test]
    async fn cardanodb_get_nonexistent_block() {
        let dir = tempdir().unwrap();
        let config = CardanoDBConfig::new(dir.path().to_path_buf());
        let db = CardanoDB::open(config).await.unwrap();

        let hash = Blake2b256Hash::new([99u8; 32]);
        let result = db.get_block(&hash).await.unwrap();
        assert_eq!(result, None);

        let slot = SlotNo(9999);
        let result_by_slot = db.get_block_by_slot(slot).await.unwrap();
        assert_eq!(result_by_slot, None);
    }

    #[tokio::test]
    async fn cardanodb_multiple_blocks() {
        let dir = tempdir().unwrap();
        let config = CardanoDBConfig::new(dir.path().to_path_buf());
        let db = CardanoDB::open(config).await.unwrap();

        // Add multiple blocks
        for i in 0..10 {
            let hash = Blake2b256Hash::new([i as u8; 32]);
            let slot = SlotNo(i as u64 * 100);
            let block_no = BlockNo(i as u64 * 10);
            let data = vec![i as u8; 100];

            db.put_block(hash, slot, block_no, data).await.unwrap();
        }

        // Verify all blocks can be retrieved
        for i in 0..10 {
            let hash = Blake2b256Hash::new([i as u8; 32]);
            let result = db.get_block(&hash).await.unwrap();
            assert!(result.is_some());
            assert_eq!(result.unwrap().len(), 100);
        }
    }
}
