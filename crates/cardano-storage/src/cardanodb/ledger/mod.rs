//! LedgerDB - In-memory ledger state with disk snapshots
//!
//! This module implements the LedgerDB component which maintains the current
//! ledger state (UTxO set, stake distribution, etc.) with periodic snapshots
//! for fast rollback.

pub mod snapshot;
pub mod state;

use crate::cardanodb::{
    config::LedgerDBConfig,
    types::{Blake2b256Hash, BlockNo, EpochNo, SlotNo},
};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

pub use snapshot::SnapshotManager;
pub use state::{LedgerState, TxInput, TxOutput};

/// Statistics about snapshot management
#[derive(Debug, Clone)]
pub struct SnapshotStats {
    /// Block number of the last snapshot
    pub last_snapshot_block: BlockNo,
    /// Current block number
    pub current_block: BlockNo,
    /// Number of blocks since last snapshot
    pub blocks_since_snapshot: u64,
    /// Total number of snapshots on disk
    pub snapshot_count: usize,
    /// Configured snapshot interval
    pub snapshot_interval: u32,
}

/// LedgerDB: In-memory ledger with disk snapshots
pub struct LedgerDB {
    /// Current ledger state (in memory)
    current_state: Arc<RwLock<LedgerState>>,

    /// Snapshot manager
    snapshots: SnapshotManager,

    /// Configuration
    config: LedgerDBConfig,

    /// Last snapshot block number
    last_snapshot_block: Arc<RwLock<BlockNo>>,
}

impl LedgerDB {
    /// Create a new LedgerDB
    pub fn new(config: LedgerDBConfig) -> Result<Self> {
        // Create snapshot directory
        std::fs::create_dir_all(&config.path)?;

        // Load latest snapshot if available
        let snapshots = SnapshotManager::new(config.path.clone())?;
        let current_state: Arc<RwLock<LedgerState>> =
            if let Some(latest) = snapshots.load_latest()? {
                Arc::new(RwLock::new(latest))
            } else {
                Arc::new(RwLock::new(LedgerState::genesis()))
            };

        Ok(Self {
            current_state,
            snapshots,
            config,
            last_snapshot_block: Arc::new(RwLock::new(BlockNo(0))),
        })
    }

    /// Get the current block number
    pub async fn get_block_no(&self) -> BlockNo {
        self.current_state.read().await.block_no
    }

    /// Get the current slot
    pub async fn get_slot(&self) -> SlotNo {
        self.current_state.read().await.slot
    }

    /// Get the current epoch
    pub async fn get_epoch(&self) -> EpochNo {
        self.current_state.read().await.epoch
    }

    /// Get a clone of the current ledger state
    ///
    /// This returns a complete snapshot of the current state.
    /// Used by block production and other services that need
    /// read-only access to the ledger state.
    pub async fn get_current_state(&self) -> LedgerState {
        self.current_state.read().await.clone()
    }

    // ===== UTxO Operations =====

    /// Get a specific UTxO by input reference
    pub async fn get_utxo(&self, input: &TxInput) -> Option<TxOutput> {
        self.current_state.read().await.get_utxo(input).cloned()
    }

    /// Check if a UTxO exists
    pub async fn has_utxo(&self, input: &TxInput) -> bool {
        self.current_state.read().await.has_utxo(input)
    }

    /// Get total number of UTxOs
    pub async fn utxo_count(&self) -> usize {
        self.current_state.read().await.utxo_count()
    }

    /// Get total value locked in UTxO set (in lovelace)
    pub async fn total_utxo_value(&self) -> u64 {
        self.current_state.read().await.total_utxo_value()
    }

    /// Apply a transaction to the ledger state
    ///
    /// This validates that all inputs exist, consumes them, and creates new outputs
    pub async fn apply_transaction(
        &self,
        tx_id: Blake2b256Hash,
        inputs: &[TxInput],
        outputs: Vec<TxOutput>,
    ) -> Result<(), String> {
        let mut state = self.current_state.write().await;
        state.apply_transaction(tx_id, inputs, outputs)
    }

    /// Rollback a transaction (for chain reorganization)
    ///
    /// This adds back the consumed inputs and removes the created outputs
    pub async fn rollback_transaction(
        &self,
        tx_id: Blake2b256Hash,
        inputs: Vec<(TxInput, TxOutput)>,
        output_count: u32,
    ) {
        let mut state = self.current_state.write().await;
        state.rollback_transaction(tx_id, inputs, output_count);
    }

    /// Update the current slot, block, and epoch
    pub async fn update_tip(&self, slot: SlotNo, block_no: BlockNo, epoch: EpochNo) {
        let mut state = self.current_state.write().await;
        state.slot = slot;
        state.block_no = block_no;
        state.epoch = epoch;
    }

    // ===== Snapshot Operations =====

    /// Create a snapshot of the current ledger state
    ///
    /// This saves the entire ledger state (UTxO set, slot, block, epoch) to disk
    pub async fn create_snapshot(&self) -> Result<()> {
        let state = self.current_state.read().await.clone();
        let block_no = state.block_no;

        let snapshot = snapshot::Snapshot {
            block_no,
            slot: state.slot,
            epoch: state.epoch,
            state,
        };

        self.snapshots.save(snapshot).await?;

        // Update last snapshot block
        let mut last = self.last_snapshot_block.write().await;
        *last = block_no;

        Ok(())
    }

    /// Check if a snapshot should be created based on configuration
    ///
    /// Returns true if enough blocks have passed since the last snapshot
    pub async fn should_create_snapshot(&self) -> bool {
        let current_block = self.current_state.read().await.block_no;
        let last_snapshot = *self.last_snapshot_block.read().await;

        current_block.0 >= last_snapshot.0 + self.config.snapshot_interval as u64
    }

    /// Create snapshot if the interval has passed
    ///
    /// This is a convenience method that checks if a snapshot should be created
    /// and creates it if necessary
    pub async fn maybe_create_snapshot(&self) -> Result<bool> {
        if self.should_create_snapshot().await {
            self.create_snapshot().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Restore ledger state from the latest snapshot
    ///
    /// This replaces the current state with the most recent snapshot
    pub async fn restore_from_latest_snapshot(&self) -> Result<bool> {
        if let Some(state) = self.snapshots.load_latest()? {
            let mut current = self.current_state.write().await;
            *current = state;

            let mut last = self.last_snapshot_block.write().await;
            *last = current.block_no;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Find and restore from the closest snapshot to a target block
    ///
    /// This is useful for fast-forwarding to a specific point in the chain
    pub async fn restore_from_snapshot_at(&self, target_block: BlockNo) -> Result<bool> {
        if let Some(snapshot) = self.snapshots.find_closest(target_block).await? {
            let mut current = self.current_state.write().await;
            *current = snapshot.state;

            let mut last = self.last_snapshot_block.write().await;
            *last = current.block_no;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Cleanup old snapshots, keeping only the N most recent
    ///
    /// This helps manage disk space by removing outdated snapshots
    pub async fn cleanup_old_snapshots(&self) -> Result<()> {
        self.snapshots.cleanup(self.config.snapshot_retention).await
    }

    /// Validate the current ledger state
    ///
    /// Performs sanity checks on the UTxO set and state
    pub async fn validate_state(&self) -> Result<(), String> {
        let state = self.current_state.read().await;

        // Check that total value doesn't overflow
        let total = state.total_utxo_value();
        if total > 45_000_000_000_000_000 {
            // Max ADA supply is 45 billion
            return Err(format!(
                "Total UTxO value {} exceeds maximum ADA supply",
                total
            ));
        }

        // Check that block/slot/epoch are reasonable
        if state.block_no.0 > 100_000_000 {
            return Err(format!(
                "Block number {} is unreasonably high",
                state.block_no.0
            ));
        }

        if state.slot.0 > 1_000_000_000 {
            return Err(format!("Slot number {} is unreasonably high", state.slot.0));
        }

        Ok(())
    }

    /// Get snapshot statistics
    pub async fn snapshot_stats(&self) -> Result<SnapshotStats> {
        let current_block = self.current_state.read().await.block_no;
        let last_snapshot = *self.last_snapshot_block.read().await;
        let blocks_since_snapshot = current_block.0.saturating_sub(last_snapshot.0);

        // Count snapshot files
        let snapshot_count = self.snapshots.count_snapshots()?;

        Ok(SnapshotStats {
            last_snapshot_block: last_snapshot,
            current_block,
            blocks_since_snapshot,
            snapshot_count,
            snapshot_interval: self.config.snapshot_interval,
        })
    }

    // TODO: Add block application, rollback, and snapshot methods in next phase
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn ledger_db_can_be_created() {
        let dir = tempdir().unwrap();
        let config = LedgerDBConfig {
            path: dir.path().to_path_buf(),
            snapshot_interval: 100,
            snapshot_retention: 10,
        };

        let db = LedgerDB::new(config).unwrap();
        // Should have genesis state
        assert_eq!(db.current_state.blocking_read().block_no, BlockNo(0));
    }

    #[tokio::test]
    async fn ledger_db_utxo_operations() {
        let dir = tempdir().unwrap();
        let config = LedgerDBConfig {
            path: dir.path().to_path_buf(),
            snapshot_interval: 100,
            snapshot_retention: 10,
        };

        let db = LedgerDB::new(config).unwrap();

        // Initial state should be empty
        assert_eq!(db.utxo_count().await, 0);
        assert_eq!(db.total_utxo_value().await, 0);

        // Create a genesis UTxO manually (in reality this would come from a block)
        {
            let mut state = db.current_state.write().await;
            let tx_id = Blake2b256Hash::new([1u8; 32]);
            state.insert_utxo(
                TxInput::new(tx_id, 0),
                TxOutput::new(vec![0x01], 100_000_000),
            );
        }

        // Check UTxO was added
        assert_eq!(db.utxo_count().await, 1);
        assert_eq!(db.total_utxo_value().await, 100_000_000);

        let genesis_input = TxInput::new(Blake2b256Hash::new([1u8; 32]), 0);
        assert!(db.has_utxo(&genesis_input).await);

        let utxo = db.get_utxo(&genesis_input).await.unwrap();
        assert_eq!(utxo.value, 100_000_000);
    }

    #[tokio::test]
    async fn ledger_db_apply_transaction() {
        let dir = tempdir().unwrap();
        let config = LedgerDBConfig {
            path: dir.path().to_path_buf(),
            snapshot_interval: 100,
            snapshot_retention: 10,
        };

        let db = LedgerDB::new(config).unwrap();

        // Setup: Add initial UTxO
        let initial_tx_id = Blake2b256Hash::new([1u8; 32]);
        let input1 = TxInput::new(initial_tx_id, 0);
        {
            let mut state = db.current_state.write().await;
            state.insert_utxo(input1.clone(), TxOutput::new(vec![0x01], 10_000_000));
        }

        // Apply transaction
        let new_tx_id = Blake2b256Hash::new([2u8; 32]);
        let result = db
            .apply_transaction(
                new_tx_id,
                &[input1.clone()],
                vec![
                    TxOutput::new(vec![0x02], 6_000_000),
                    TxOutput::new(vec![0x03], 3_500_000),
                ],
            )
            .await;

        assert!(result.is_ok());

        // Old input should be spent
        assert!(!db.has_utxo(&input1).await);

        // New outputs should exist
        assert!(db.has_utxo(&TxInput::new(new_tx_id, 0)).await);
        assert!(db.has_utxo(&TxInput::new(new_tx_id, 1)).await);

        // Total value should have decreased (fees)
        assert_eq!(db.utxo_count().await, 2);
        assert_eq!(db.total_utxo_value().await, 9_500_000); // 500k lovelace fee
    }

    #[tokio::test]
    async fn ledger_db_rollback_transaction() {
        let dir = tempdir().unwrap();
        let config = LedgerDBConfig {
            path: dir.path().to_path_buf(),
            snapshot_interval: 100,
            snapshot_retention: 10,
        };

        let db = LedgerDB::new(config).unwrap();

        // Setup: Add initial UTxO
        let initial_tx_id = Blake2b256Hash::new([1u8; 32]);
        let input1 = TxInput::new(initial_tx_id, 0);
        let output1 = TxOutput::new(vec![0x01], 10_000_000);
        {
            let mut state = db.current_state.write().await;
            state.insert_utxo(input1.clone(), output1.clone());
        }

        // Apply transaction
        let new_tx_id = Blake2b256Hash::new([2u8; 32]);
        db.apply_transaction(
            new_tx_id,
            &[input1.clone()],
            vec![
                TxOutput::new(vec![0x02], 6_000_000),
                TxOutput::new(vec![0x03], 3_000_000),
            ],
        )
        .await
        .unwrap();

        assert_eq!(db.utxo_count().await, 2);

        // Rollback the transaction
        db.rollback_transaction(new_tx_id, vec![(input1.clone(), output1)], 2)
            .await;

        // Should be back to initial state
        assert_eq!(db.utxo_count().await, 1);
        assert!(db.has_utxo(&input1).await);
        assert!(!db.has_utxo(&TxInput::new(new_tx_id, 0)).await);
        assert!(!db.has_utxo(&TxInput::new(new_tx_id, 1)).await);
        assert_eq!(db.total_utxo_value().await, 10_000_000);
    }

    #[tokio::test]
    async fn ledger_db_update_tip() {
        let dir = tempdir().unwrap();
        let config = LedgerDBConfig {
            path: dir.path().to_path_buf(),
            snapshot_interval: 100,
            snapshot_retention: 10,
        };

        let db = LedgerDB::new(config).unwrap();

        // Initial state
        assert_eq!(db.get_block_no().await, BlockNo(0));
        assert_eq!(db.get_slot().await, SlotNo(0));
        assert_eq!(db.get_epoch().await, EpochNo(0));

        // Update tip
        db.update_tip(SlotNo(12345), BlockNo(678), EpochNo(23))
            .await;

        // Check updated values
        assert_eq!(db.get_block_no().await, BlockNo(678));
        assert_eq!(db.get_slot().await, SlotNo(12345));
        assert_eq!(db.get_epoch().await, EpochNo(23));
    }

    #[tokio::test]
    async fn ledger_db_multiple_transactions() {
        let dir = tempdir().unwrap();
        let config = LedgerDBConfig {
            path: dir.path().to_path_buf(),
            snapshot_interval: 100,
            snapshot_retention: 10,
        };

        let db = LedgerDB::new(config).unwrap();

        // TX1: Create initial UTxOs
        let tx1_id = Blake2b256Hash::new([1u8; 32]);
        {
            let mut state = db.current_state.write().await;
            state.insert_utxo(
                TxInput::new(tx1_id, 0),
                TxOutput::new(vec![0x01], 100_000_000),
            );
            state.insert_utxo(
                TxInput::new(tx1_id, 1),
                TxOutput::new(vec![0x02], 50_000_000),
            );
        }

        assert_eq!(db.utxo_count().await, 2);
        assert_eq!(db.total_utxo_value().await, 150_000_000);

        // TX2: Spend first output
        let tx2_id = Blake2b256Hash::new([2u8; 32]);
        db.apply_transaction(
            tx2_id,
            &[TxInput::new(tx1_id, 0)],
            vec![
                TxOutput::new(vec![0x03], 70_000_000),
                TxOutput::new(vec![0x04], 29_000_000),
            ],
        )
        .await
        .unwrap();

        assert_eq!(db.utxo_count().await, 3);

        // TX3: Spend multiple outputs
        let tx3_id = Blake2b256Hash::new([3u8; 32]);
        db.apply_transaction(
            tx3_id,
            &[TxInput::new(tx1_id, 1), TxInput::new(tx2_id, 1)],
            vec![TxOutput::new(vec![0x05], 78_000_000)],
        )
        .await
        .unwrap();

        assert_eq!(db.utxo_count().await, 2);
        assert_eq!(db.total_utxo_value().await, 148_000_000);
    }

    #[tokio::test]
    async fn ledger_db_create_snapshot() {
        let dir = tempdir().unwrap();
        let config = LedgerDBConfig {
            path: dir.path().to_path_buf(),
            snapshot_interval: 100,
            snapshot_retention: 10,
        };

        let db = LedgerDB::new(config).unwrap();

        // Update state
        db.update_tip(SlotNo(1000), BlockNo(500), EpochNo(10)).await;

        // Create snapshot
        db.create_snapshot().await.unwrap();

        // Verify snapshot was created
        let stats = db.snapshot_stats().await.unwrap();
        assert_eq!(stats.snapshot_count, 1);
        assert_eq!(stats.last_snapshot_block, BlockNo(500));
    }

    #[tokio::test]
    async fn ledger_db_restore_from_snapshot() {
        let dir = tempdir().unwrap();
        let config = LedgerDBConfig {
            path: dir.path().to_path_buf(),
            snapshot_interval: 100,
            snapshot_retention: 10,
        };

        let db = LedgerDB::new(config).unwrap();

        // Set up initial state and snapshot
        db.update_tip(SlotNo(1000), BlockNo(500), EpochNo(10)).await;
        {
            let mut state = db.current_state.write().await;
            let tx_id = Blake2b256Hash::new([1u8; 32]);
            state.insert_utxo(
                TxInput::new(tx_id, 0),
                TxOutput::new(vec![0x01], 100_000_000),
            );
        }
        db.create_snapshot().await.unwrap();

        // Modify state
        db.update_tip(SlotNo(2000), BlockNo(1000), EpochNo(20))
            .await;

        // Restore from snapshot
        let restored = db.restore_from_latest_snapshot().await.unwrap();
        assert!(restored);

        // Verify state was restored
        assert_eq!(db.get_block_no().await, BlockNo(500));
        assert_eq!(db.get_slot().await, SlotNo(1000));
        assert_eq!(db.get_epoch().await, EpochNo(10));
        assert_eq!(db.utxo_count().await, 1);
    }

    #[tokio::test]
    async fn ledger_db_should_create_snapshot() {
        let dir = tempdir().unwrap();
        let config = LedgerDBConfig {
            path: dir.path().to_path_buf(),
            snapshot_interval: 100,
            snapshot_retention: 10,
        };

        let db = LedgerDB::new(config).unwrap();

        // Initially should not need snapshot (block 0)
        assert!(!db.should_create_snapshot().await);

        // Advance to block 100
        db.update_tip(SlotNo(1000), BlockNo(100), EpochNo(10)).await;

        // Now should need snapshot
        assert!(db.should_create_snapshot().await);

        // Create snapshot
        db.create_snapshot().await.unwrap();

        // Should not need another yet
        assert!(!db.should_create_snapshot().await);

        // Advance to block 200
        db.update_tip(SlotNo(2000), BlockNo(200), EpochNo(20)).await;

        // Should need snapshot again
        assert!(db.should_create_snapshot().await);
    }

    #[tokio::test]
    async fn ledger_db_maybe_create_snapshot() {
        let dir = tempdir().unwrap();
        let config = LedgerDBConfig {
            path: dir.path().to_path_buf(),
            snapshot_interval: 100,
            snapshot_retention: 10,
        };

        let db = LedgerDB::new(config).unwrap();

        // Should not create at block 0
        let created = db.maybe_create_snapshot().await.unwrap();
        assert!(!created);

        // Advance to block 100
        db.update_tip(SlotNo(1000), BlockNo(100), EpochNo(10)).await;

        // Should create now
        let created = db.maybe_create_snapshot().await.unwrap();
        assert!(created);

        // Should not create again immediately
        let created = db.maybe_create_snapshot().await.unwrap();
        assert!(!created);
    }

    #[tokio::test]
    async fn ledger_db_validate_state() {
        let dir = tempdir().unwrap();
        let config = LedgerDBConfig {
            path: dir.path().to_path_buf(),
            snapshot_interval: 100,
            snapshot_retention: 10,
        };

        let db = LedgerDB::new(config).unwrap();

        // Genesis state should be valid
        assert!(db.validate_state().await.is_ok());

        // Add reasonable state
        db.update_tip(SlotNo(1000), BlockNo(500), EpochNo(10)).await;
        {
            let mut state = db.current_state.write().await;
            let tx_id = Blake2b256Hash::new([1u8; 32]);
            state.insert_utxo(
                TxInput::new(tx_id, 0),
                TxOutput::new(vec![0x01], 1_000_000_000),
            );
        }

        assert!(db.validate_state().await.is_ok());
    }

    #[tokio::test]
    async fn ledger_db_cleanup_snapshots() {
        let dir = tempdir().unwrap();
        let config = LedgerDBConfig {
            path: dir.path().to_path_buf(),
            snapshot_interval: 10,
            snapshot_retention: 3,
        };

        let db = LedgerDB::new(config).unwrap();

        // Create 5 snapshots
        for i in 1..=5 {
            db.update_tip(SlotNo(i * 100), BlockNo(i * 10), EpochNo(i))
                .await;
            db.create_snapshot().await.unwrap();
        }

        let stats = db.snapshot_stats().await.unwrap();
        assert_eq!(stats.snapshot_count, 5);

        // Cleanup old snapshots
        db.cleanup_old_snapshots().await.unwrap();

        let stats = db.snapshot_stats().await.unwrap();
        assert_eq!(stats.snapshot_count, 3);
    }

    #[tokio::test]
    async fn ledger_db_restore_from_snapshot_at() {
        let dir = tempdir().unwrap();
        let config = LedgerDBConfig {
            path: dir.path().to_path_buf(),
            snapshot_interval: 100,
            snapshot_retention: 10,
        };

        let db = LedgerDB::new(config).unwrap();

        // Create snapshots at blocks 100, 200, 300
        for i in 1..=3 {
            db.update_tip(SlotNo(i * 1000), BlockNo(i * 100), EpochNo(i * 10))
                .await;
            db.create_snapshot().await.unwrap();
        }

        // Restore to closest snapshot before block 250 (should be 200)
        let restored = db.restore_from_snapshot_at(BlockNo(250)).await.unwrap();
        assert!(restored);
        assert_eq!(db.get_block_no().await, BlockNo(200));

        // Restore to closest snapshot before block 350 (should be 300)
        let restored = db.restore_from_snapshot_at(BlockNo(350)).await.unwrap();
        assert!(restored);
        assert_eq!(db.get_block_no().await, BlockNo(300));
    }
}
