//! Slot and time calculations
//!
//! Handles conversion between wall-clock time and blockchain slots.

use crate::{ConsensusError, Result};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Slot time calculator
pub struct SlotCalculator {
    slot_length: Duration,
    genesis_time: SystemTime,
}

impl SlotCalculator {
    /// Create new slot calculator
    pub fn new(slot_length_secs: u64, genesis_time: SystemTime) -> Self {
        Self {
            slot_length: Duration::from_secs(slot_length_secs),
            genesis_time,
        }
    }

    /// Get current slot number
    pub fn current_slot(&self) -> Result<u64> {
        let now = SystemTime::now();
        let elapsed = now.duration_since(self.genesis_time)
            .map_err(|e| ConsensusError::SlotError(format!("Time calculation error: {}", e)))?;

        Ok(elapsed.as_secs() / self.slot_length.as_secs())
    }

    /// Get the start time of a slot
    pub fn slot_start_time(&self, slot: u64) -> SystemTime {
        self.genesis_time + Duration::from_secs(slot * self.slot_length.as_secs())
    }

    /// Get the end time of a slot
    pub fn slot_end_time(&self, slot: u64) -> SystemTime {
        self.slot_start_time(slot + 1)
    }
}
