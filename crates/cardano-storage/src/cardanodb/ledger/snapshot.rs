//! Snapshot management for LedgerDB
//!
//! This module handles creating, loading, and managing ledger state snapshots
//! on disk for fast rollback capability.

use super::state::{LedgerState, TxInput, TxOutput};
use crate::cardanodb::types::{BlockNo, EpochNo, SlotNo};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Snapshot manager for disk persistence
pub struct SnapshotManager {
    snapshot_dir: PathBuf,
}

/// A serializable snapshot of the ledger state
///
/// Note: The UTxO set is converted to a Vec for JSON serialization compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerializableSnapshot {
    pub block_no: BlockNo,
    pub slot: SlotNo,
    pub epoch: EpochNo,
    /// UTxO entries as a vector of (input, output) pairs
    pub utxo_entries: Vec<(TxInput, TxOutput)>,
}

/// A snapshot of the ledger state at a specific point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub block_no: BlockNo,
    pub slot: SlotNo,
    pub epoch: EpochNo,
    pub state: LedgerState,
}

impl Snapshot {
    /// Convert to serializable format
    fn to_serializable(&self) -> SerializableSnapshot {
        SerializableSnapshot {
            block_no: self.block_no,
            slot: self.slot,
            epoch: self.epoch,
            utxo_entries: self
                .state
                .utxo
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        }
    }

    /// Create from serializable format
    fn from_serializable(ser: SerializableSnapshot) -> Self {
        let mut utxo = HashMap::new();
        for (input, output) in ser.utxo_entries {
            utxo.insert(input, output);
        }

        let state = LedgerState {
            slot: ser.slot,
            block_no: ser.block_no,
            epoch: ser.epoch,
            utxo,
        };

        Snapshot {
            block_no: ser.block_no,
            slot: ser.slot,
            epoch: ser.epoch,
            state,
        }
    }
}

impl SnapshotManager {
    /// Create a new snapshot manager
    pub fn new(snapshot_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&snapshot_dir).context("Failed to create snapshot directory")?;
        Ok(Self { snapshot_dir })
    }

    /// Save a snapshot to disk
    pub async fn save(&self, snapshot: Snapshot) -> Result<()> {
        let filename = format!("ledger_snapshot_{}.bin", snapshot.block_no.0);
        let path = self.snapshot_dir.join(&filename);

        // Convert to serializable format
        let serializable = snapshot.to_serializable();

        // Serialize snapshot
        let data = serde_json::to_vec(&serializable)
            .map_err(|e| anyhow::anyhow!("JSON encoding error: {}", e))?;

        // Write atomically (tmp + rename)
        let tmp_path = path.with_extension("tmp");
        tokio::fs::write(&tmp_path, data)
            .await
            .context("Failed to write snapshot")?;
        tokio::fs::rename(&tmp_path, &path)
            .await
            .context("Failed to rename snapshot")?;

        Ok(())
    }

    /// Load the latest snapshot
    pub fn load_latest(&self) -> Result<Option<LedgerState>> {
        // Find latest snapshot file
        let mut entries: Vec<_> = std::fs::read_dir(&self.snapshot_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("bin"))
            .collect();

        if entries.is_empty() {
            return Ok(None);
        }

        entries.sort_by_key(|e| e.path());

        if let Some(latest) = entries.last() {
            let data = std::fs::read(latest.path()).context("Failed to read snapshot file")?;
            let serializable: SerializableSnapshot =
                serde_json::from_slice(&data).context("Failed to deserialize snapshot")?;
            let snapshot = Snapshot::from_serializable(serializable);
            Ok(Some(snapshot.state))
        } else {
            Ok(None)
        }
    }

    /// Find the closest snapshot to a given block number
    pub async fn find_closest(&self, target_block_no: BlockNo) -> Result<Option<Snapshot>> {
        let entries: Vec<_> = std::fs::read_dir(&self.snapshot_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("bin"))
            .collect();

        let mut best: Option<Snapshot> = None;

        for entry in entries {
            let data = tokio::fs::read(entry.path()).await?;
            let serializable: SerializableSnapshot = serde_json::from_slice(&data)
                .map_err(|e| anyhow::anyhow!("JSON decoding error: {}", e))?;
            let snapshot = Snapshot::from_serializable(serializable);

            if snapshot.block_no <= target_block_no {
                if let Some(ref current) = best {
                    if snapshot.block_no > current.block_no {
                        best = Some(snapshot);
                    }
                } else {
                    best = Some(snapshot);
                }
            }
        }

        Ok(best)
    }

    /// Cleanup old snapshots, keeping only the N most recent
    pub async fn cleanup(&self, retention: u32) -> Result<()> {
        let mut entries: Vec<_> = std::fs::read_dir(&self.snapshot_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("bin"))
            .collect();

        if entries.len() <= retention as usize {
            return Ok(());
        }

        entries.sort_by_key(|e| e.path());

        // Remove old snapshots, keeping only the most recent N
        let to_remove = entries.len() - retention as usize;
        for entry in entries.iter().take(to_remove) {
            tokio::fs::remove_file(entry.path())
                .await
                .context("Failed to remove old snapshot")?;
        }

        Ok(())
    }

    /// Count the total number of snapshots on disk
    pub fn count_snapshots(&self) -> Result<usize> {
        let count = std::fs::read_dir(&self.snapshot_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("bin"))
            .count();

        Ok(count)
    }

    /// List all snapshot block numbers
    pub fn list_snapshots(&self) -> Result<Vec<BlockNo>> {
        let mut block_numbers: Vec<BlockNo> = std::fs::read_dir(&self.snapshot_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("bin"))
            .filter_map(|e| {
                // Parse block number from filename: ledger_snapshot_12345.bin
                e.path()
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.strip_prefix("ledger_snapshot_"))
                    .and_then(|s| s.parse::<u64>().ok())
                    .map(BlockNo)
            })
            .collect();

        block_numbers.sort();
        Ok(block_numbers)
    }

    /// Get the size of a specific snapshot file in bytes
    pub fn snapshot_size(&self, block_no: BlockNo) -> Result<u64> {
        let filename = format!("ledger_snapshot_{}.bin", block_no.0);
        let path = self.snapshot_dir.join(&filename);

        let metadata = std::fs::metadata(&path).context("Failed to read snapshot file metadata")?;

        Ok(metadata.len())
    }

    /// Validate a snapshot file integrity
    pub async fn validate_snapshot(&self, block_no: BlockNo) -> Result<bool> {
        let filename = format!("ledger_snapshot_{}.bin", block_no.0);
        let path = self.snapshot_dir.join(&filename);

        // Try to load and deserialize the snapshot
        let data = tokio::fs::read(&path).await?;
        let serializable: Result<SerializableSnapshot, _> = serde_json::from_slice(&data);

        match serializable {
            Ok(ser) => {
                // Verify the block number matches
                if ser.block_no != block_no {
                    return Ok(false);
                }
                // Basic sanity checks
                if ser.utxo_entries.len() > 1_000_000_000 {
                    return Ok(false); // Unreasonable UTxO count
                }
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn snapshot_save_and_load() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::new(dir.path().to_path_buf()).unwrap();

        let mut state = LedgerState::genesis();
        state.slot = SlotNo(1000);
        state.block_no = BlockNo(500);
        state.epoch = EpochNo(10);

        let snapshot = Snapshot {
            block_no: state.block_no,
            slot: state.slot,
            epoch: state.epoch,
            state: state.clone(),
        };

        manager.save(snapshot).await.unwrap();

        let loaded = manager.load_latest().unwrap().unwrap();
        assert_eq!(loaded.block_no, state.block_no);
        assert_eq!(loaded.slot, state.slot);
    }

    #[tokio::test]
    async fn snapshot_cleanup_removes_old() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::new(dir.path().to_path_buf()).unwrap();

        // Create 5 snapshots
        for i in 0..5 {
            let mut state = LedgerState::genesis();
            state.slot = SlotNo(i * 100);
            state.block_no = BlockNo(i * 50);
            state.epoch = EpochNo(i);

            let snapshot = Snapshot {
                block_no: state.block_no,
                slot: state.slot,
                epoch: state.epoch,
                state,
            };

            manager.save(snapshot).await.unwrap();
        }

        // Cleanup, keeping only 3
        manager.cleanup(3).await.unwrap();

        // Count remaining files
        let count = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("bin"))
            .count();

        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn snapshot_count() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::new(dir.path().to_path_buf()).unwrap();

        assert_eq!(manager.count_snapshots().unwrap(), 0);

        // Create 3 snapshots
        for i in 0..3 {
            let mut state = LedgerState::genesis();
            state.block_no = BlockNo(i * 100);

            let snapshot = Snapshot {
                block_no: state.block_no,
                slot: state.slot,
                epoch: state.epoch,
                state,
            };

            manager.save(snapshot).await.unwrap();
        }

        assert_eq!(manager.count_snapshots().unwrap(), 3);
    }

    #[tokio::test]
    async fn snapshot_list() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::new(dir.path().to_path_buf()).unwrap();

        let block_numbers = vec![100, 200, 300];

        for &block_no in &block_numbers {
            let mut state = LedgerState::genesis();
            state.block_no = BlockNo(block_no);

            let snapshot = Snapshot {
                block_no: state.block_no,
                slot: state.slot,
                epoch: state.epoch,
                state,
            };

            manager.save(snapshot).await.unwrap();
        }

        let listed = manager.list_snapshots().unwrap();
        assert_eq!(listed.len(), 3);
        assert_eq!(listed[0], BlockNo(100));
        assert_eq!(listed[1], BlockNo(200));
        assert_eq!(listed[2], BlockNo(300));
    }

    #[tokio::test]
    async fn snapshot_size() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::new(dir.path().to_path_buf()).unwrap();

        let mut state = LedgerState::genesis();
        state.block_no = BlockNo(100);

        let snapshot = Snapshot {
            block_no: state.block_no,
            slot: state.slot,
            epoch: state.epoch,
            state,
        };

        manager.save(snapshot).await.unwrap();

        let size = manager.snapshot_size(BlockNo(100)).unwrap();
        assert!(size > 0);
        assert!(size < 10_000); // Should be small for genesis state
    }

    #[tokio::test]
    async fn snapshot_validate_good() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::new(dir.path().to_path_buf()).unwrap();

        let mut state = LedgerState::genesis();
        state.block_no = BlockNo(100);

        let snapshot = Snapshot {
            block_no: state.block_no,
            slot: state.slot,
            epoch: state.epoch,
            state,
        };

        manager.save(snapshot).await.unwrap();

        let valid = manager.validate_snapshot(BlockNo(100)).await.unwrap();
        assert!(valid);
    }

    #[tokio::test]
    async fn snapshot_validate_missing() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::new(dir.path().to_path_buf()).unwrap();

        let result = manager.validate_snapshot(BlockNo(999)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn snapshot_find_closest() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::new(dir.path().to_path_buf()).unwrap();

        // Create snapshots at blocks 100, 200, 300
        for i in 1..=3 {
            let mut state = LedgerState::genesis();
            state.block_no = BlockNo(i * 100);

            let snapshot = Snapshot {
                block_no: state.block_no,
                slot: state.slot,
                epoch: state.epoch,
                state,
            };

            manager.save(snapshot).await.unwrap();
        }

        // Find closest to 250 should return 200
        let closest = manager.find_closest(BlockNo(250)).await.unwrap();
        assert!(closest.is_some());
        assert_eq!(closest.unwrap().block_no, BlockNo(200));

        // Find closest to 50 should return None (no snapshot before it)
        let closest = manager.find_closest(BlockNo(50)).await.unwrap();
        assert!(closest.is_none());

        // Find closest to 400 should return 300
        let closest = manager.find_closest(BlockNo(400)).await.unwrap();
        assert!(closest.is_some());
        assert_eq!(closest.unwrap().block_no, BlockNo(300));
    }
}
