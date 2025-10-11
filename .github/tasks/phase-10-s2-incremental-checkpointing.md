# S2 Roadmap - Task 10: Incremental Checkpointing

**Task 10:** Implement Incremental Checkpointing for LedgerDB

- **Status**: Not Started
- **Files**:
  - Create: `crates/cardano-storage/src/checkpoint_manager.rs`
  - Create: `crates/cardano-storage/src/checkpoint_diff.rs`
  - Modify: `crates/cardano-storage/src/ledgerdb.rs`
  - Create: `tests/storage/checkpoint_tests.rs`
- **Description**: Implement incremental checkpointing to reduce full snapshot overhead, enable fast rollback, and optimize disk usage through checkpoint diffing and pruning.

## Task Checklist

### 1. Checkpoint Manager Foundation

- [ ] Create CheckpointManager struct
- [ ] Implement checkpoint interval configuration
- [ ] Add checkpoint metadata tracking
- [ ] Create checkpoint storage abstraction
- [ ] Implement checkpoint ID generation
- [ ] Add checkpoint verification
- [ ] Implement checkpoint cleanup policies

### 2. Incremental Snapshot Diffing

- [ ] Design checkpoint diff format
- [ ] Implement state diffing algorithm
- [ ] Create diff serialization
- [ ] Add diff compression (LZ4/Zstd)
- [ ] Implement diff application logic
- [ ] Add diff validation
- [ ] Optimize diff size for common patterns

### 3. Fast Rollback Support

- [ ] Integrate checkpoints with rollback
- [ ] Implement checkpoint-based recovery
- [ ] Add rollback distance estimation
- [ ] Optimize checkpoint selection for rollback
- [ ] Add rollback validation with checkpoints
- [ ] Implement progressive rollback (checkpoint → diff → current)

### 4. Checkpoint Pruning

- [ ] Define pruning policies (age, count, disk)
- [ ] Implement automatic pruning triggers
- [ ] Add manual pruning interface
- [ ] Create pruning safety checks
- [ ] Implement checkpoint retention rules
- [ ] Add pruning metrics

### 5. Testing and Validation

- [ ] Test checkpoint creation at intervals
- [ ] Test diff generation correctness
- [ ] Test checkpoint restoration
- [ ] Test fast rollback scenarios
- [ ] Test pruning policies
- [ ] Benchmark checkpoint overhead
- [ ] Test disk space savings
- [ ] Test recovery from checkpoint corruption

## Implementation Details

### CheckpointManager Structure

```rust
// crates/cardano-storage/src/checkpoint_manager.rs

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::{LedgerState, SlotNo, Result, StorageError};

#[derive(Debug, Clone)]
pub struct CheckpointConfig {
    /// Checkpoint every N slots (e.g., 21600 = 6 hours)
    pub interval_slots: u64,

    /// Maximum checkpoints to retain
    pub max_checkpoints: usize,

    /// Maximum age of checkpoints (slots)
    pub max_age_slots: u64,

    /// Enable compression
    pub compress: bool,

    /// Compression level (1-9 for Zstd)
    pub compression_level: i32,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            interval_slots: 21600, // ~6 hours
            max_checkpoints: 10,
            max_age_slots: 432000, // ~5 days
            compress: true,
            compression_level: 3,
        }
    }
}

pub struct CheckpointManager {
    config: CheckpointConfig,
    base_path: PathBuf,
    checkpoints: Arc<RwLock<Vec<CheckpointMetadata>>>,
}

#[derive(Debug, Clone)]
pub struct CheckpointMetadata {
    pub id: CheckpointId,
    pub slot: SlotNo,
    pub block_hash: Blake2b256Hash,
    pub created_at: SystemTime,
    pub size_bytes: u64,
    pub is_full: bool, // true = full snapshot, false = diff
    pub parent_id: Option<CheckpointId>, // for diffs
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CheckpointId(pub u64);

impl CheckpointManager {
    pub async fn new(base_path: PathBuf, config: CheckpointConfig) -> Result<Self> {
        tokio::fs::create_dir_all(&base_path).await
            .map_err(|e| StorageError::Io(e))?;

        let checkpoints = Self::load_metadata(&base_path).await?;

        Ok(Self {
            config,
            base_path,
            checkpoints: Arc::new(RwLock::new(checkpoints)),
        })
    }

    pub async fn should_checkpoint(&self, current_slot: SlotNo) -> bool {
        let checkpoints = self.checkpoints.read().await;

        if checkpoints.is_empty() {
            return true;
        }

        let last_checkpoint_slot = checkpoints.last().unwrap().slot;
        current_slot.0 >= last_checkpoint_slot.0 + self.config.interval_slots
    }

    pub async fn create_checkpoint(
        &self,
        ledger: &LedgerState,
        slot: SlotNo,
        block_hash: Blake2b256Hash,
    ) -> Result<CheckpointId> {
        let id = CheckpointId::new(slot);
        let is_full = self.should_create_full_snapshot().await;

        let size = if is_full {
            self.write_full_snapshot(&id, ledger).await?
        } else {
            let parent_id = self.latest_checkpoint_id().await?;
            self.write_diff_snapshot(&id, parent_id, ledger).await?
        };

        let metadata = CheckpointMetadata {
            id,
            slot,
            block_hash,
            created_at: SystemTime::now(),
            size_bytes: size,
            is_full,
            parent_id: if is_full { None } else { Some(parent_id) },
        };

        self.add_checkpoint(metadata).await?;
        self.prune_old_checkpoints().await?;

        Ok(id)
    }

    pub async fn restore_checkpoint(
        &self,
        id: CheckpointId,
    ) -> Result<LedgerState> {
        let metadata = self.find_checkpoint(id).await?;

        if metadata.is_full {
            self.load_full_snapshot(id).await
        } else {
            // Recursively apply diffs from nearest full snapshot
            let chain = self.build_diff_chain(id).await?;
            let mut state = self.load_full_snapshot(chain[0]).await?;

            for diff_id in &chain[1..] {
                let diff = self.load_diff(*diff_id).await?;
                state = diff.apply_to(state)?;
            }

            Ok(state)
        }
    }

    pub async fn rollback_to_checkpoint(
        &self,
        target_slot: SlotNo,
    ) -> Result<(CheckpointId, LedgerState)> {
        let checkpoint = self.find_checkpoint_before_slot(target_slot).await?;
        let state = self.restore_checkpoint(checkpoint.id).await?;
        Ok((checkpoint.id, state))
    }

    async fn should_create_full_snapshot(&self) -> bool {
        let checkpoints = self.checkpoints.read().await;

        // Create full snapshot every 10 checkpoints
        checkpoints.iter().filter(|c| c.is_full).count() == 0 ||
        checkpoints.len() % 10 == 0
    }

    async fn prune_old_checkpoints(&self) -> Result<()> {
        let mut checkpoints = self.checkpoints.write().await;
        let current_slot = checkpoints.last().map(|c| c.slot).unwrap_or(SlotNo(0));

        // Remove checkpoints exceeding max count
        while checkpoints.len() > self.config.max_checkpoints {
            let removed = checkpoints.remove(0);
            self.delete_checkpoint_file(removed.id).await?;
        }

        // Remove checkpoints exceeding max age
        checkpoints.retain(|c| {
            current_slot.0 - c.slot.0 <= self.config.max_age_slots
        });

        // Always keep at least one full snapshot
        let has_full = checkpoints.iter().any(|c| c.is_full);
        if !has_full && !checkpoints.is_empty() {
            // Convert oldest diff to full snapshot
            // (This would require implementation)
        }

        Ok(())
    }

    fn checkpoint_path(&self, id: CheckpointId) -> PathBuf {
        self.base_path.join(format!("checkpoint_{}.dat", id.0))
    }
}

impl CheckpointId {
    pub fn new(slot: SlotNo) -> Self {
        Self(slot.0)
    }
}
```

### Checkpoint Diff Format

```rust
// crates/cardano-storage/src/checkpoint_diff.rs

use std::collections::HashMap;
use crate::{LedgerState, UtxoId, UtxoEntry, StakeDistribution, Result};

#[derive(Debug, Clone)]
pub struct CheckpointDiff {
    /// UTXO additions
    pub utxo_added: HashMap<UtxoId, UtxoEntry>,

    /// UTXO removals
    pub utxo_removed: Vec<UtxoId>,

    /// Stake distribution changes
    pub stake_updates: Vec<StakeUpdate>,

    /// Protocol parameter updates
    pub param_updates: Option<ProtocolParams>,

    /// Slot and epoch metadata
    pub target_slot: SlotNo,
    pub target_epoch: EpochNo,
}

#[derive(Debug, Clone)]
pub struct StakeUpdate {
    pub pool_id: PoolId,
    pub stake_delta: i64, // positive = added, negative = removed
}

impl CheckpointDiff {
    pub fn compute(from: &LedgerState, to: &LedgerState) -> Self {
        let mut diff = CheckpointDiff {
            utxo_added: HashMap::new(),
            utxo_removed: Vec::new(),
            stake_updates: Vec::new(),
            param_updates: None,
            target_slot: to.current_slot,
            target_epoch: to.current_epoch,
        };

        // Compute UTXO diff
        for (id, entry) in &to.utxo {
            if !from.utxo.contains_key(id) {
                diff.utxo_added.insert(id.clone(), entry.clone());
            }
        }

        for id in from.utxo.keys() {
            if !to.utxo.contains_key(id) {
                diff.utxo_removed.push(id.clone());
            }
        }

        // Compute stake diff
        // (Simplified - would need proper stake distribution diffing)

        diff
    }

    pub fn apply_to(self, mut state: LedgerState) -> Result<LedgerState> {
        // Apply UTXO changes
        for (id, entry) in self.utxo_added {
            state.utxo.insert(id, entry);
        }

        for id in self.utxo_removed {
            state.utxo.remove(&id);
        }

        // Apply stake updates
        // (Implementation depends on stake distribution structure)

        // Update metadata
        state.current_slot = self.target_slot;
        state.current_epoch = self.target_epoch;

        Ok(state)
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        // Use bincode or custom format
        bincode::serialize(self)
            .map_err(|e| StorageError::Serialization(e.to_string()))
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data)
            .map_err(|e| StorageError::Deserialization(e.to_string()))
    }
}
```

### LedgerDB Integration

```rust
// Modify crates/cardano-storage/src/ledgerdb.rs

use crate::checkpoint_manager::{CheckpointManager, CheckpointConfig};

impl LedgerDB {
    pub async fn new_with_checkpoints(
        db_path: PathBuf,
        checkpoint_config: CheckpointConfig,
    ) -> Result<Self> {
        let checkpoint_path = db_path.join("checkpoints");
        let checkpoint_manager = CheckpointManager::new(checkpoint_path, checkpoint_config).await?;

        // ... existing initialization ...

        Ok(Self {
            // ... existing fields ...
            checkpoint_manager: Some(Arc::new(checkpoint_manager)),
        })
    }

    pub async fn update_ledger_with_checkpointing(
        &self,
        slot: SlotNo,
        block_hash: Blake2b256Hash,
        header: &BlockHeader,
    ) -> Result<()> {
        // Update ledger state
        self.update_ledger(slot, header).await?;

        // Check if checkpoint needed
        if let Some(cm) = &self.checkpoint_manager {
            if cm.should_checkpoint(slot).await {
                let state = self.get_ledger_state().await?;
                cm.create_checkpoint(&state, slot, block_hash).await?;
            }
        }

        Ok(())
    }

    pub async fn rollback_with_checkpoints(
        &self,
        target_slot: SlotNo,
    ) -> Result<()> {
        if let Some(cm) = &self.checkpoint_manager {
            // Try to rollback from checkpoint first (much faster)
            if let Ok((checkpoint_id, state)) = cm.rollback_to_checkpoint(target_slot).await {
                self.restore_state(state).await?;
                return Ok(());
            }
        }

        // Fallback to regular rollback
        self.rollback_to_slot(target_slot).await
    }
}
```

## Success Criteria

- [ ] CheckpointManager creates checkpoints at configured intervals
- [ ] Diff snapshots are significantly smaller than full snapshots (>70% reduction)
- [ ] Checkpoint restoration is successful and produces correct state
- [ ] Fast rollback from checkpoint is 10x faster than regular rollback
- [ ] Pruning maintains configured checkpoint limits
- [ ] Compression reduces checkpoint size by >50%
- [ ] All tests pass (8+ tests)
- [ ] Benchmarks show acceptable overhead (<5% sync slowdown)
- [ ] Disk usage stays within bounds
- [ ] Documentation complete

## Dependencies

- Phase 05: LedgerDB implementation
- External: bincode, zstd (or lz4) crates

## Testing

```bash
# Run checkpoint tests
cargo test --package cardano-storage checkpoint

# Benchmark checkpoint performance
cargo bench --package cardano-storage checkpoint_bench

# Test with real sync
cargo run --release --bin cardano-node -- --network preview --checkpoint-interval 10000
```

## Performance Targets

- Full snapshot creation: <2 seconds
- Diff snapshot creation: <500ms
- Checkpoint restoration: <1 second
- Diff size vs full: <30% of full snapshot
- Checkpoint overhead during sync: <5%
- Disk space for 10 checkpoints: <500MB

## Estimated Effort

- CheckpointManager foundation: 4-5 hours
- Diff implementation: 5-6 hours
- Rollback integration: 3-4 hours
- Pruning policies: 2-3 hours
- Testing and benchmarks: 4-5 hours
- **Total: 18-23 hours**

## Future Enhancements

- [ ] Parallel checkpoint compression
- [ ] Cloud backup integration
- [ ] Checkpoint sharing between nodes
- [ ] Delta encoding for diffs
- [ ] Checkpoint verification with Merkle proofs
- [ ] Configurable checkpoint strategies (epoch boundaries, etc.)
