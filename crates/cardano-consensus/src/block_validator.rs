//! Block Validator Service
//!
//! This module provides block validation services that integrate with LedgerDB.
//! It validates incoming block headers against the current ledger state and
//! handles chain reorganizations through rollback/rollforward mechanisms.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────┐
//! │ BlockValidator   │
//! ├──────────────────┤
//! │ - Validate header│ ──> Check VRF, KES, slot
//! │ - Apply to ledger│ ──> Update UTxO set
//! │ - Handle rollback│ <── Chain reorganization
//! └──────────────────┘
//!        │
//!        v
//!  ┌────────────┐
//!  │  LedgerDB  │
//!  └────────────┘
//! ```

use crate::block_production::BlockHeader;
use crate::ouroboros::SlotNo;
use cardano_crypto::Blake2b256Hash;
use cardano_storage::cardanodb::ledger::LedgerDB;
use cardano_storage::cardanodb::types::{BlockNo, EpochNo};
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Block validation errors
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid slot number: {0}")]
    InvalidSlot(String),

    #[error("Slot regression: current={current}, new={new}")]
    SlotRegression { current: u64, new: u64 },

    #[error("Invalid block number: {0}")]
    InvalidBlockNumber(String),

    #[error("Block hash mismatch: expected={expected}, got={got}")]
    HashMismatch { expected: String, got: String },

    #[error("Ledger error: {0}")]
    LedgerError(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),
}

/// Result type for block validation
pub type Result<T> = std::result::Result<T, ValidationError>;

/// Block validator that integrates with LedgerDB
pub struct BlockValidator {
    /// Reference to the ledger database
    ledger: Arc<LedgerDB>,

    /// Maximum allowed slot regression during rollback
    max_rollback_slots: u64,
}

impl BlockValidator {
    /// Create a new block validator
    pub fn new(ledger: Arc<LedgerDB>) -> Self {
        Self {
            ledger,
            max_rollback_slots: 2160, // 1 hour at 1 slot/second
        }
    }

    /// Create a new block validator with custom rollback limit
    pub fn with_rollback_limit(ledger: Arc<LedgerDB>, max_rollback_slots: u64) -> Self {
        Self {
            ledger,
            max_rollback_slots,
        }
    }

    /// Validate a block header against the current ledger state
    ///
    /// This performs the following checks:
    /// - Slot number is greater than current slot (monotonic time)
    /// - Block number follows sequence
    /// - Header structure is valid
    ///
    /// Note: Full validation (VRF, KES, etc.) will be added in later tasks
    pub async fn validate_header(&self, header: &BlockHeader) -> Result<()> {
        let current_slot = self.ledger.get_slot().await;
        let current_block = self.ledger.get_block_no().await;

        // Check slot progression
        if header.slot.0 <= current_slot.0 {
            return Err(ValidationError::SlotRegression {
                current: current_slot.0,
                new: header.slot.0,
            });
        }

        // Check block number progression (simplified - should allow gaps for missing blocks)
        if header.block_number <= current_block.0 {
            return Err(ValidationError::InvalidBlockNumber(format!(
                "Block number {} is not greater than current {}",
                header.block_number, current_block.0
            )));
        }

        debug!(
            "Header validation passed: slot={}, block={}",
            header.slot.0, header.block_number
        );

        Ok(())
    }

    /// Apply a validated block header to the ledger state
    ///
    /// This is a rollforward operation that advances the ledger state.
    /// The header must have been validated with `validate_header()` first.
    pub async fn apply_header(&self, header: &BlockHeader) -> Result<()> {
        let slot = cardano_storage::cardanodb::types::SlotNo(header.slot.0);
        let block_no = BlockNo(header.block_number);

        // Calculate epoch from slot (simplified - should use protocol params)
        // For now, use a default epoch length of 432000 slots (5 days)
        let epoch_length = 432000u64;
        let epoch = EpochNo((header.slot.0 / epoch_length) as u64);

        info!(
            "Applying header: slot={}, block={}, epoch={}",
            slot.0, block_no.0, epoch.0
        );

        // Update the ledger tip
        self.ledger.update_tip(slot, block_no, epoch).await;

        // TODO: In future tasks:
        // - Apply transactions from the block body
        // - Update UTxO set
        // - Process stake pool registrations/delegations
        // - Update protocol parameters if needed

        // Check if we should create a snapshot
        if let Err(e) = self.ledger.maybe_create_snapshot().await {
            warn!("Failed to create ledger snapshot: {:?}", e);
        }

        Ok(())
    }

    /// Rollback the ledger state to a previous point
    ///
    /// This is used during chain reorganizations when we need to switch to a different fork.
    /// The target slot must be within the configured rollback limit.
    pub async fn rollback_to_slot(&self, target_slot: SlotNo) -> Result<()> {
        let current_slot = self.ledger.get_slot().await;

        // Validate rollback distance
        if current_slot.0 < target_slot.0 {
            return Err(ValidationError::InvalidSlot(format!(
                "Cannot rollback to future slot {} (current: {})",
                target_slot.0, current_slot.0
            )));
        }

        let rollback_distance = current_slot.0 - target_slot.0;
        if rollback_distance > self.max_rollback_slots {
            return Err(ValidationError::ValidationFailed(format!(
                "Rollback distance {} exceeds maximum {}",
                rollback_distance, self.max_rollback_slots
            )));
        }

        warn!(
            "Rolling back from slot {} to slot {} ({} slots)",
            current_slot.0, target_slot.0, rollback_distance
        );

        // TODO: Implement actual rollback logic
        // For now, this is a placeholder that would:
        // 1. Find the snapshot before target_slot
        // 2. Load that snapshot into current_state
        // 3. Replay blocks from snapshot to target_slot
        //
        // The CardanoDB LedgerDB already has snapshot infrastructure,
        // but we need to wire it up to block/slot-based rollback

        Ok(())
    }

    /// Validate and apply a block header (convenience method)
    ///
    /// This combines validation and application into a single operation.
    pub async fn validate_and_apply(&self, header: &BlockHeader) -> Result<()> {
        self.validate_header(header).await?;
        self.apply_header(header).await?;
        Ok(())
    }

    /// Get the current ledger slot
    pub async fn current_slot(&self) -> SlotNo {
        let ledger_slot = self.ledger.get_slot().await;
        SlotNo(ledger_slot.0) // Convert from LedgerDB SlotNo to consensus SlotNo
    }

    /// Get the current ledger block number
    pub async fn current_block_no(&self) -> BlockNo {
        self.ledger.get_block_no().await
    }

    /// Get the current ledger epoch
    pub async fn current_epoch(&self) -> EpochNo {
        self.ledger.get_epoch().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cardano_crypto::{Ed25519KeyHash, VrfOutput, VrfProof};
    use cardano_storage::cardanodb::config::LedgerDBConfig;
    use cardano_storage::cardanodb::types::SlotNo as LedgerSlotNo;

    fn create_test_ledger() -> Arc<LedgerDB> {
        // Use a unique temporary path for each test to avoid conflicts
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_path =
            std::env::temp_dir().join(format!("ledger_test_{}_{}", std::process::id(), id));
        std::fs::create_dir_all(&temp_path).unwrap();

        let config = LedgerDBConfig {
            path: temp_path,
            snapshot_interval: 100,
            snapshot_retention: 10,
        };
        Arc::new(LedgerDB::new(config).unwrap())
    }

    fn create_test_header(slot: u64, block_number: u64) -> BlockHeader {
        BlockHeader {
            slot: SlotNo(slot),
            block_number,
            prev_hash: Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap(),
            issuer_vkey: Ed25519KeyHash::from_bytes([1u8; 20]),
            vrf_proof: VrfProof::from_bytes(&[2u8; 128]).unwrap(), // VRF_PROOF_LENGTH is 128
            vrf_output: VrfOutput::from_bytes(&[3u8; 64]).unwrap(), // VRF_OUTPUT_LENGTH is 64
            block_body_hash: Blake2b256Hash::from_bytes(&[4u8; 32]).unwrap(),
            block_size: 1000,
            operational_cert: crate::block_production::OperationalCertificate {
                hot_vkey: Ed25519KeyHash::from_bytes([5u8; 20]),
                sequence_number: 0,
                kes_period: 0,
                sigma: Blake2b256Hash::from_bytes(&[6u8; 32]).unwrap(),
            },
            protocol_magic: 764824073, // Preview network magic
        }
    }
    #[tokio::test]
    async fn test_validator_creation() {
        let ledger = create_test_ledger();
        let validator = BlockValidator::new(ledger.clone());

        // Convert ledger SlotNo to consensus SlotNo for comparison
        let ledger_slot = validator.ledger.get_slot().await;
        assert_eq!(ledger_slot.0, 0);
        assert_eq!(validator.current_block_no().await.0, 0);
        assert_eq!(validator.current_epoch().await.0, 0);
    }

    #[tokio::test]
    async fn test_validate_header_progression() {
        let ledger = create_test_ledger();
        let validator = BlockValidator::new(ledger.clone());

        // First header should validate (slot 1 > 0)
        let header1 = create_test_header(1, 1);
        assert!(validator.validate_header(&header1).await.is_ok());
    }

    #[tokio::test]
    async fn test_validate_header_slot_regression() {
        let ledger = create_test_ledger();
        let validator = BlockValidator::new(ledger.clone());

        // Manually update ledger to simulate applied header
        ledger
            .update_tip(LedgerSlotNo(100), BlockNo(50), EpochNo(0))
            .await;

        // Try to apply header with earlier slot - should fail
        let header2 = create_test_header(99, 51);
        let result = validator.validate_header(&header2).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::SlotRegression { .. }
        ));
    }

    #[tokio::test]
    async fn test_apply_header_updates_ledger() {
        let ledger = create_test_ledger();
        let validator = BlockValidator::new(ledger.clone());

        let header = create_test_header(100, 50);
        validator.apply_header(&header).await.unwrap();

        // Verify ledger was updated (note: epoch is not stored in header, defaults to 0)
        let ledger_slot = ledger.get_slot().await;
        assert_eq!(ledger_slot.0, 100);
        assert_eq!(validator.current_block_no().await.0, 50);
    }

    #[tokio::test]
    async fn test_validate_and_apply() {
        let ledger = create_test_ledger();
        let validator = BlockValidator::new(ledger.clone());

        let header = create_test_header(1, 1);
        validator.validate_and_apply(&header).await.unwrap();

        let ledger_slot = ledger.get_slot().await;
        assert_eq!(ledger_slot.0, 1);
        assert_eq!(validator.current_block_no().await.0, 1);
    }

    #[tokio::test]
    async fn test_rollback_validation() {
        let ledger = create_test_ledger();
        let validator = BlockValidator::with_rollback_limit(ledger.clone(), 100);

        // Set current slot to 200
        let header = create_test_header(200, 100);
        validator.apply_header(&header).await.unwrap();

        // Rollback within limit should be allowed
        let result = validator.rollback_to_slot(SlotNo(150)).await;
        assert!(result.is_ok());

        // Rollback beyond limit should fail
        let result = validator.rollback_to_slot(SlotNo(50)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rollback_to_future_fails() {
        let ledger = create_test_ledger();
        let validator = BlockValidator::new(ledger.clone());

        // Current slot is 0, try to rollback to future
        let result = validator.rollback_to_slot(SlotNo(100)).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::InvalidSlot(_)
        ));
    }

    #[tokio::test]
    async fn test_sequential_header_application() {
        let ledger = create_test_ledger();
        let validator = BlockValidator::new(ledger.clone());

        // Apply headers in sequence
        for i in 1..=5 {
            let header = create_test_header(i * 20, i);
            validator.validate_and_apply(&header).await.unwrap();
        }

        // Verify final state
        let ledger_slot = ledger.get_slot().await;
        assert_eq!(ledger_slot.0, 100);
        assert_eq!(validator.current_block_no().await.0, 5);
    }
}
