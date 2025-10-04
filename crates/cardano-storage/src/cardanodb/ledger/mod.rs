//! LedgerDB - In-memory ledger state with disk snapshots
//!
//! This module implements the LedgerDB component which maintains the current
//! ledger state (UTxO set, stake distribution, etc.) with periodic snapshots
//! for fast rollback.

pub mod snapshot;
pub mod state;

use crate::cardanodb::{
    config::LedgerDBConfig,
    types::{BlockNo, EpochNo, SlotNo},
};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

pub use snapshot::SnapshotManager;
pub use state::LedgerState;

/// LedgerDB: In-memory ledger with disk snapshots
pub struct LedgerDB {
    /// Current ledger state (in memory)
    current_state: Arc<RwLock<LedgerState>>,

    /// Snapshot manager
    #[allow(dead_code)]
    snapshots: SnapshotManager,

    /// Configuration
    #[allow(dead_code)]
    config: LedgerDBConfig,
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
}
