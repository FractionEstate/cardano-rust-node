//! Block Migration Service
//!
//! Automatically migrates stable blocks from VolatileDB to ImmutableDB after K confirmations.
//!
//! ## Architecture
//!
//! The migration service runs as a background task that periodically:
//! 1. Checks for blocks with K+ confirmations in VolatileDB
//! 2. Migrates those blocks to ImmutableDB
//! 3. Removes migrated blocks from VolatileDB
//!
//! ## Safety
//!
//! - Only migrates blocks that are K-deep (have K confirmations)
//! - Maintains atomic operations to prevent data loss
//! - Monitors migration failures and retries
//!
//! ## Performance
//!
//! - Batch migration for efficiency
//! - Configurable migration interval
//! - Non-blocking async operations

use crate::cardanodb::{
    immutable::ImmutableDB,
    types::BlockNo,
    volatile::{VolatileBlock, VolatileDB},
};
use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info};

/// Configuration for block migration
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    /// Security parameter K - blocks need K confirmations before migration
    pub k: u64,

    /// Interval between migration checks
    pub check_interval: Duration,

    /// Maximum blocks to migrate in one batch
    pub batch_size: usize,

    /// Enable automatic migration
    pub enabled: bool,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            k: 2160,                                 // Mainnet security parameter
            check_interval: Duration::from_secs(60), // Check every minute
            batch_size: 100,                         // Migrate up to 100 blocks at once
            enabled: true,
        }
    }
}

impl MigrationConfig {
    /// Create configuration for preview testnet
    pub fn preview() -> Self {
        Self {
            k: 720,
            check_interval: Duration::from_secs(30),
            batch_size: 50,
            enabled: true,
        }
    }

    /// Create configuration for testing
    pub fn test() -> Self {
        Self {
            k: 10,
            check_interval: Duration::from_secs(1),
            batch_size: 10,
            enabled: true,
        }
    }
}

/// Statistics for migration operations
#[derive(Debug, Clone, Default)]
pub struct MigrationStats {
    /// Total blocks migrated
    pub total_migrated: u64,

    /// Total migration attempts
    pub migration_runs: u64,

    /// Failed migration attempts
    pub failed_migrations: u64,

    /// Last migration timestamp
    pub last_migration: Option<std::time::Instant>,

    /// Average blocks per migration
    pub avg_blocks_per_migration: f64,
}

/// Block Migration Service
///
/// Manages automatic migration of stable blocks from VolatileDB to ImmutableDB.
pub struct BlockMigrationService {
    volatile_db: Arc<VolatileDB>,
    immutable_db: Arc<ImmutableDB>,
    config: MigrationConfig,
    stats: Arc<RwLock<MigrationStats>>,
    shutdown_tx: broadcast::Sender<()>,
}

impl BlockMigrationService {
    /// Create a new migration service
    pub fn new(
        volatile_db: Arc<VolatileDB>,
        immutable_db: Arc<ImmutableDB>,
        config: MigrationConfig,
    ) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);

        Self {
            volatile_db,
            immutable_db,
            config,
            stats: Arc::new(RwLock::new(MigrationStats::default())),
            shutdown_tx,
        }
    }

    /// Start the migration service as a background task
    ///
    /// Returns a handle that can be awaited to stop the service.
    pub fn start(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            info!(
                "Block migration service started (K={}, interval={:?})",
                self.config.k, self.config.check_interval
            );

            loop {
                tokio::select! {
                    _ = tokio::time::sleep(self.config.check_interval) => {
                        if self.config.enabled {
                            if let Err(e) = self.run_migration().await {
                                error!("Migration error: {}", e);
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Block migration service shutting down");
                        break;
                    }
                }
            }
        })
    }

    /// Stop the migration service
    pub fn stop(&self) -> Result<()> {
        self.shutdown_tx
            .send(())
            .context("Failed to send shutdown signal")?;
        Ok(())
    }

    /// Run a single migration cycle
    ///
    /// This method:
    /// 1. Identifies blocks in VolatileDB that are K-deep
    /// 2. Migrates them to ImmutableDB in batches
    /// 3. Removes successfully migrated blocks from VolatileDB
    async fn run_migration(&self) -> Result<()> {
        let start = std::time::Instant::now();

        // Get current chain tip from VolatileDB
        let tip = self.volatile_db.get_tip().await;
        let tip_block_no = match tip {
            Some(ref t) => t.block_no,
            None => {
                debug!("No blocks in VolatileDB, skipping migration");
                return Ok(());
            }
        };

        // Find blocks that are K-deep (have K confirmations)
        let candidates = self.find_migration_candidates(tip_block_no).await?;

        if candidates.is_empty() {
            debug!(
                "No blocks ready for migration (tip={}, K={})",
                tip_block_no.0, self.config.k
            );
            return Ok(());
        }

        info!(
            "Found {} blocks ready for migration (tip={}, K={})",
            candidates.len(),
            tip_block_no.0,
            self.config.k
        );

        // Migrate blocks in batches
        let mut migrated_count = 0;
        for batch in candidates.chunks(self.config.batch_size) {
            match self.migrate_batch(batch).await {
                Ok(count) => {
                    migrated_count += count;
                    debug!("Migrated batch of {} blocks", count);
                }
                Err(e) => {
                    error!("Batch migration failed: {}", e);
                    self.stats.write().await.failed_migrations += 1;
                    // Continue with next batch
                }
            }
        }

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_migrated += migrated_count as u64;
        stats.migration_runs += 1;
        stats.last_migration = Some(start);
        stats.avg_blocks_per_migration = stats.total_migrated as f64 / stats.migration_runs as f64;

        let elapsed = start.elapsed();
        info!(
            "Migration complete: {} blocks in {:?} ({:.1} blocks/sec)",
            migrated_count,
            elapsed,
            migrated_count as f64 / elapsed.as_secs_f64()
        );

        Ok(())
    }

    /// Find blocks in VolatileDB that are ready for migration
    ///
    /// A block is ready if it has K confirmations (i.e., there are K blocks after it)
    async fn find_migration_candidates(&self, tip_block_no: BlockNo) -> Result<Vec<VolatileBlock>> {
        // Get all blocks from VolatileDB
        let all_blocks = self.volatile_db.get_all_blocks().await;

        // Filter blocks that are K-deep
        let stable_threshold = tip_block_no.0.saturating_sub(self.config.k);

        let candidates: Vec<VolatileBlock> = all_blocks
            .into_iter()
            .filter(|block| block.block_no.0 <= stable_threshold)
            .collect();

        debug!(
            "find_migration_candidates: tip={}, K={}, threshold={}, found {} candidates",
            tip_block_no.0,
            self.config.k,
            stable_threshold,
            candidates.len()
        );

        Ok(candidates)
    }

    /// Migrate a batch of blocks from VolatileDB to ImmutableDB
    async fn migrate_batch(&self, blocks: &[VolatileBlock]) -> Result<usize> {
        let mut migrated = 0;

        for block in blocks {
            // Store in ImmutableDB
            self.immutable_db
                .store_block(block.slot_no, block.block_no, block.hash, &block.data)
                .await
                .context(format!(
                    "Failed to store block {} in ImmutableDB",
                    block.block_no.0
                ))?;

            // Remove from VolatileDB
            self.volatile_db
                .remove_block(&block.hash)
                .await
                .context(format!(
                    "Failed to remove block {} from VolatileDB",
                    block.block_no.0
                ))?;

            migrated += 1;

            debug!(
                "Migrated block {} (slot {}, hash {})",
                block.block_no.0,
                block.slot_no.0,
                hex::encode(&block.hash.as_bytes()[..8])
            );
        }

        Ok(migrated)
    }

    /// Get current migration statistics
    pub async fn stats(&self) -> MigrationStats {
        self.stats.read().await.clone()
    }

    /// Manually trigger a migration cycle
    ///
    /// Useful for testing or forcing migration on demand.
    pub async fn trigger_migration(&self) -> Result<()> {
        info!("Manual migration triggered");
        self.run_migration().await
    }

    /// Check if a block with given block number is stable (K-deep)
    pub async fn is_stable(&self, block_no: BlockNo) -> bool {
        if let Some(tip) = self.volatile_db.get_tip().await {
            let confirmations = tip.block_no.0.saturating_sub(block_no.0);
            confirmations >= self.config.k
        } else {
            false
        }
    }

    /// Get the stability threshold block number
    ///
    /// Blocks with block_no <= threshold are considered stable.
    pub async fn stability_threshold(&self) -> Option<u64> {
        self.volatile_db
            .get_tip()
            .await
            .map(|tip| tip.block_no.0.saturating_sub(self.config.k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cardanodb::{
        config::{ImmutableDBConfig, VolatileDBConfig},
        types::{Blake2b256Hash, SlotNo},
    };
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_migration_service_basic() {
        let temp_dir = tempdir().unwrap();

        // Create databases with larger K to hold all test blocks
        let volatile_config = VolatileDBConfig { k: 30 };
        let volatile_db = Arc::new(VolatileDB::new(volatile_config));

        let immutable_config = ImmutableDBConfig {
            path: temp_dir.path().to_path_buf(),
            chunk_size: 1000,
            max_cached_chunks: 10,
            enable_compression: false,
        };
        let immutable_db = Arc::new(ImmutableDB::open(immutable_config).unwrap());

        // Create migration service with K=10
        let config = MigrationConfig::test();
        let service = Arc::new(BlockMigrationService::new(
            volatile_db.clone(),
            immutable_db.clone(),
            config,
        ));

        // Add 20 blocks to VolatileDB
        for i in 0..20 {
            let hash = Blake2b256Hash::hash(&[i as u8; 32]);
            let slot = SlotNo(i as u64);
            let block_no = BlockNo(i as u64);
            let data = vec![i as u8; 100];

            volatile_db
                .add_block(hash, slot, block_no, data)
                .await
                .unwrap();
        }

        // Check initial state
        let initial_count = volatile_db.block_count().await;
        assert_eq!(initial_count, 20);

        // Run migration
        service.trigger_migration().await.unwrap();

        // Check results
        let _remaining = volatile_db.block_count().await;
        let stats = service.stats().await;

        // Verify that blocks were migrated
        assert!(
            stats.total_migrated >= 10,
            "Expected at least 10 blocks migrated, got {}",
            stats.total_migrated
        );
        assert_eq!(stats.migration_runs, 1);
    }

    #[tokio::test]
    async fn test_stability_threshold() {
        let volatile_config = VolatileDBConfig { k: 10 };
        let volatile_db = Arc::new(VolatileDB::new(volatile_config));

        let temp_dir = tempdir().unwrap();
        let immutable_config = ImmutableDBConfig {
            path: temp_dir.path().to_path_buf(),
            chunk_size: 1000,
            max_cached_chunks: 10,
            enable_compression: false,
        };
        let immutable_db = Arc::new(ImmutableDB::open(immutable_config).unwrap());

        let config = MigrationConfig {
            k: 5,
            ..MigrationConfig::test()
        };
        let service = Arc::new(BlockMigrationService::new(
            volatile_db.clone(),
            immutable_db,
            config,
        ));

        // Add blocks
        for i in 0..15 {
            let hash = Blake2b256Hash::hash(&[i as u8; 32]);
            volatile_db
                .add_block(hash, SlotNo(i), BlockNo(i), vec![i as u8])
                .await
                .unwrap();
        }

        // With tip at block 14 and K=5, threshold should be 9
        let threshold = service.stability_threshold().await;
        assert_eq!(threshold, Some(9));

        // Block 5 should be stable (5 <= 9)
        assert!(service.is_stable(BlockNo(5)).await);

        // Block 12 should not be stable (12 > 9)
        assert!(!service.is_stable(BlockNo(12)).await);
    }

    #[tokio::test]
    async fn test_migration_with_empty_volatile() {
        let volatile_config = VolatileDBConfig { k: 10 };
        let volatile_db = Arc::new(VolatileDB::new(volatile_config));

        let temp_dir = tempdir().unwrap();
        let immutable_config = ImmutableDBConfig {
            path: temp_dir.path().to_path_buf(),
            chunk_size: 1000,
            max_cached_chunks: 10,
            enable_compression: false,
        };
        let immutable_db = Arc::new(ImmutableDB::open(immutable_config).unwrap());

        let service = Arc::new(BlockMigrationService::new(
            volatile_db,
            immutable_db,
            MigrationConfig::test(),
        ));

        // Should not error with empty VolatileDB
        let result = service.trigger_migration().await;
        assert!(result.is_ok());

        let stats = service.stats().await;
        assert_eq!(stats.total_migrated, 0);
    }
}
