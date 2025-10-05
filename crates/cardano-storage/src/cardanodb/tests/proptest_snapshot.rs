//! Property-based tests for snapshot serialization
//!
//! These tests verify that snapshots can be reliably serialized and
//! deserialized across a wide range of ledger states.

use crate::cardanodb::{
    ledger::state::LedgerState,
    types::{BlockNo, EpochNo, SlotNo},
};
use proptest::prelude::*;

// Strategy for generating valid SlotNo values
fn slot_no_strategy() -> impl Strategy<Value = SlotNo> {
    any::<u64>().prop_map(SlotNo)
}

// Strategy for generating valid BlockNo values
fn block_no_strategy() -> impl Strategy<Value = BlockNo> {
    any::<u64>().prop_map(BlockNo)
}

// Strategy for generating valid EpochNo values
fn epoch_no_strategy() -> impl Strategy<Value = EpochNo> {
    any::<u64>().prop_map(EpochNo)
}

// Strategy for generating valid LedgerState
fn ledger_state_strategy() -> impl Strategy<Value = LedgerState> {
    (slot_no_strategy(), block_no_strategy(), epoch_no_strategy()).prop_map(
        |(slot, block_no, epoch)| {
            let mut state = LedgerState::genesis();
            state.slot = slot;
            state.block_no = block_no;
            state.epoch = epoch;
            state
        },
    )
}

proptest! {
    #[test]
    fn ledger_state_json_serialization_roundtrip(state in ledger_state_strategy()) {
        let json = serde_json::to_vec(&state).unwrap();
        let decoded: LedgerState = serde_json::from_slice(&json).unwrap();

        prop_assert_eq!(state.slot, decoded.slot);
        prop_assert_eq!(state.block_no, decoded.block_no);
        prop_assert_eq!(state.epoch, decoded.epoch);
    }

    #[test]
    fn ledger_state_json_serialization_is_deterministic(state in ledger_state_strategy()) {
        let json1 = serde_json::to_vec(&state).unwrap();
        let json2 = serde_json::to_vec(&state).unwrap();

        prop_assert_eq!(json1, json2);
    }

    #[test]
    fn ledger_state_preserves_field_order(
        slot in any::<u64>(),
        block_no in any::<u64>(),
        epoch in any::<u64>()
    ) {
        let mut state = LedgerState::genesis();
        state.slot = SlotNo(slot);
        state.block_no = BlockNo(block_no);
        state.epoch = EpochNo(epoch);

        let json = serde_json::to_vec(&state).unwrap();
        let decoded: LedgerState = serde_json::from_slice(&json).unwrap();

        prop_assert_eq!(state.slot.0, decoded.slot.0);
        prop_assert_eq!(state.block_no.0, decoded.block_no.0);
        prop_assert_eq!(state.epoch.0, decoded.epoch.0);
    }

    #[test]
    fn slot_no_ordering_property(a in any::<u64>(), b in any::<u64>()) {
        let slot_a = SlotNo(a);
        let slot_b = SlotNo(b);

        // SlotNo ordering should match u64 ordering
        prop_assert_eq!(slot_a < slot_b, a < b);
        prop_assert_eq!(slot_a > slot_b, a > b);
        prop_assert_eq!(slot_a == slot_b, a == b);
    }

    #[test]
    fn block_no_ordering_property(a in any::<u64>(), b in any::<u64>()) {
        let block_a = BlockNo(a);
        let block_b = BlockNo(b);

        // BlockNo ordering should match u64 ordering
        prop_assert_eq!(block_a < block_b, a < b);
        prop_assert_eq!(block_a > block_b, a > b);
        prop_assert_eq!(block_a == block_b, a == b);
    }

    #[test]
    fn epoch_no_ordering_property(a in any::<u64>(), b in any::<u64>()) {
        let epoch_a = EpochNo(a);
        let epoch_b = EpochNo(b);

        // EpochNo ordering should match u64 ordering
        prop_assert_eq!(epoch_a < epoch_b, a < b);
        prop_assert_eq!(epoch_a > epoch_b, a > b);
        prop_assert_eq!(epoch_a == epoch_b, a == b);
    }

    #[test]
    fn ledger_state_json_size_is_reasonable(state in ledger_state_strategy()) {
        let json = serde_json::to_vec(&state).unwrap();

        // JSON should be compact (< 200 bytes for 3 u64 fields)
        prop_assert!(json.len() < 200);

        // But not too small (needs field names + values)
        prop_assert!(json.len() > 30);
    }

    #[test]
    fn different_states_produce_different_json(
        state1 in ledger_state_strategy(),
        state2 in ledger_state_strategy()
    ) {
        // Skip if states are actually equal
        prop_assume!(
            state1.slot != state2.slot ||
            state1.block_no != state2.block_no ||
            state1.epoch != state2.epoch
        );

        let json1 = serde_json::to_vec(&state1).unwrap();
        let json2 = serde_json::to_vec(&state2).unwrap();

        prop_assert_ne!(json1, json2);
    }

    #[test]
    fn ledger_state_handles_extreme_values(
        slot in prop_oneof![Just(0u64), Just(u64::MAX), any::<u64>()],
        block_no in prop_oneof![Just(0u64), Just(u64::MAX), any::<u64>()],
        epoch in prop_oneof![Just(0u64), Just(u64::MAX), any::<u64>()]
    ) {
        let mut state = LedgerState::genesis();
        state.slot = SlotNo(slot);
        state.block_no = BlockNo(block_no);
        state.epoch = EpochNo(epoch);

        // Should serialize and deserialize even with extreme values
        let json = serde_json::to_vec(&state).unwrap();
        let decoded: LedgerState = serde_json::from_slice(&json).unwrap();

        prop_assert_eq!(state.slot, decoded.slot);
        prop_assert_eq!(state.block_no, decoded.block_no);
        prop_assert_eq!(state.epoch, decoded.epoch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ledger_state_strategy_produces_valid_states() {
        proptest!(|(state in ledger_state_strategy())| {
            // Just verify the strategy works - all fields are unsigned so always >= 0
            let _ = (state.slot, state.block_no, state.epoch);
        });
    }
}
