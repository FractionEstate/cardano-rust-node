//! Snapshot management for LedgerDB
//!
//! This module handles creating, loading, and managing ledger state snapshots
//! on disk for fast rollback capability.

use super::state::LedgerState;
use crate::cardanodb::types::{BlockNo, EpochNo, SlotNo};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Snapshot manager for disk persistence
pub struct SnapshotManager {
    snapshot_dir: PathBuf,
}

/// A snapshot of the ledger state at a specific point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub block_no: BlockNo,
    pub slot: SlotNo,
    pub epoch: EpochNo,
    pub state: LedgerState,
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

        // Serialize snapshot
        let data = serde_json::to_vec(&snapshot)
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
            let snapshot: Snapshot =
                serde_json::from_slice(&data).context("Failed to deserialize snapshot")?;
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
            let snapshot: Snapshot = serde_json::from_slice(&data)
                .map_err(|e| anyhow::anyhow!("JSON decoding error: {}", e))?;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn snapshot_save_and_load() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::new(dir.path().to_path_buf()).unwrap();

        let state = LedgerState {
            slot: SlotNo(1000),
            block_no: BlockNo(500),
            epoch: EpochNo(10),
        };

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
            let state = LedgerState {
                slot: SlotNo(i * 100),
                block_no: BlockNo(i * 50),
                epoch: EpochNo(i),
            };

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
}
