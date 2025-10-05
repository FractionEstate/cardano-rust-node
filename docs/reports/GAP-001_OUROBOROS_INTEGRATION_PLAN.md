# OuroborosState Integration with EpochTransitionHandler

**Status**: 🔄 READY FOR INTEGRATION
**Priority**: P1 (High)
**Effort**: 1-2 hours
**Date**: October 2025

## Overview

This document outlines the integration plan for wiring the `EpochTransitionHandler` into the `OuroborosState` to replace the current stub implementations of epoch boundary logic.

## Current State

### Existing Stub Implementations (in ouroboros.rs)

The following methods are currently stubbed and need to be replaced:

```rust
// Lines 446-468: evolve_epoch_nonce
fn evolve_epoch_nonce(&mut self, new_epoch: EpochNo) -> Result<()> {
    // Uses simple XOR instead of proper VRF mixing
    let epoch_bytes = new_epoch.0.to_le_bytes();
    let mut nonce_bytes = [0u8; 32];
    for (i, byte) in self.epoch_nonce.as_bytes().iter().enumerate() {
        nonce_bytes[i] = byte ^ epoch_bytes[i % epoch_bytes.len()];
    }
    self.epoch_nonce = Blake2b256Hash::hash(&nonce_bytes);
    Ok(())
}

// Lines 470-480: snapshot_stake_distribution
fn snapshot_stake_distribution(&mut self, current_epoch: EpochNo) -> Result<()> {
    // Just logs, doesn't actually persist anything
    let snapshot_epoch = EpochNo(current_epoch.0 + 2);
    eprintln!("[DEBUG] Taking stake snapshot...");
    Ok(())
}

// Lines 532-548: calculate_epoch_rewards
fn calculate_epoch_rewards(&mut self, completed_epoch: EpochNo) -> Result<()> {
    // Just logs, no actual reward calculation
    eprintln!("[DEBUG] Reward calculation... (stub)");
    Ok(())
}
```

### Architecture Gap

The `OuroborosState` currently manages:
- ✅ Slot progression
- ✅ Epoch detection
- ✅ Stake pool registration
- ✅ Leader schedule caching
- ❌ **Epoch transitions** (stubs only)

The `EpochTransitionHandler` provides:
- ✅ VRF-based nonce evolution
- ✅ Mark-Set-Go stake snapshots
- ✅ Reward calculation and distribution
- ✅ Integration with LedgerDB

## Integration Plan

### Phase 1: Add Handler to OuroborosState

#### Step 1.1: Modify Imports

```rust
// Add to ouroboros.rs imports
use crate::epoch_transition::EpochTransitionHandler;
use cardano_storage::LedgerDatabase;
use std::sync::Arc;
```

#### Step 1.2: Add Handler Field

```rust
pub struct OuroborosState<L: LedgerDatabase> {
    pub current_slot: SlotNo,
    pub current_epoch: EpochNo,
    pub stake_distribution: StakeDistribution,
    pub registered_pools: HashMap<PoolId, StakePool>,
    pub protocol_params: ProtocolParameters,
    pub epoch_nonce: Blake2b256Hash,
    pub leader_schedule: HashMap<SlotNo, PoolId>,

    // NEW: Epoch transition handler
    pub epoch_handler: EpochTransitionHandler<L>,
}
```

#### Step 1.3: Update Constructor

```rust
impl<L: LedgerDatabase> OuroborosState<L> {
    pub fn new(
        genesis_hash: Blake2b256Hash,
        protocol_params: ProtocolParameters,
        ledgerdb: Arc<L>,
    ) -> Self {
        let epoch_handler = EpochTransitionHandler::new(
            ledgerdb,
            protocol_params.clone(),
            genesis_hash.clone(),
        );

        Self {
            current_slot: SlotNo(0),
            current_epoch: EpochNo(0),
            stake_distribution: StakeDistribution::new(),
            registered_pools: HashMap::new(),
            protocol_params,
            epoch_nonce: genesis_hash,
            leader_schedule: HashMap::new(),
            epoch_handler,
        }
    }
}
```

### Phase 2: Replace Stub Methods

#### Step 2.1: VRF Output Collection

Add a new method to collect VRF outputs during block processing:

```rust
/// Collect VRF output from block for nonce evolution
pub fn collect_block_vrf(&mut self, slot: SlotNo, vrf_output: VrfOutput) -> Result<()> {
    self.epoch_handler.collect_vrf_output(slot, vrf_output)
}
```

**Usage**: Call this method when processing each block's VRF proof.

#### Step 2.2: Replace handle_epoch_transition

```rust
/// Handle epoch boundary transitions
async fn handle_epoch_transition(
    &mut self,
    old_epoch: EpochNo,
    new_epoch: EpochNo
) -> Result<()> {
    eprintln!(
        "[INFO] Processing epoch transition: {} -> {}",
        old_epoch.0, new_epoch.0
    );

    // Use EpochTransitionHandler for all epoch logic
    self.epoch_handler
        .process_epoch_transition(old_epoch, new_epoch, self.current_slot)
        .await?;

    // Update our cached nonce from handler
    self.epoch_nonce = self.epoch_handler.current_nonce();

    // Reset leader schedule for new epoch
    self.leader_schedule.clear();

    eprintln!("[INFO] Epoch transition complete: epoch {}", new_epoch.0);
    Ok(())
}
```

#### Step 2.3: Remove Stub Methods

Delete these methods entirely:
- `evolve_epoch_nonce()` - Replaced by handler
- `snapshot_stake_distribution()` - Replaced by handler
- `calculate_active_stake()` - Can be queried from handler snapshots
- `calculate_epoch_rewards()` - Replaced by handler

#### Step 2.4: Update advance_slot

Change `advance_slot` to be async:

```rust
pub async fn advance_slot(&mut self, new_slot: SlotNo) -> Result<()> {
    if new_slot.0 <= self.current_slot.0 {
        return Err(ConsensusError::InvalidSlotProgression(format!(
            "Cannot advance from slot {} to {}",
            self.current_slot.0, new_slot.0
        )));
    }

    let old_epoch = self.current_epoch;
    let new_epoch = new_slot.to_epoch(&self.protocol_params);

    self.current_slot = new_slot;
    self.current_epoch = new_epoch;

    // Handle epoch transition if needed
    if new_epoch.0 > old_epoch.0 {
        self.handle_epoch_transition(old_epoch, new_epoch).await?;
    }

    Ok(())
}
```

### Phase 3: Update Call Sites

#### Affected Files

1. **`crates/cardano-node/src/node.rs`**
   - Update `Node::run()` to call `await` on `advance_slot()`
   - Wire VRF output collection from block processing

2. **`crates/cardano-consensus/src/chain.rs`** (if exists)
   - Update chain selection logic
   - Make async calls

3. **Tests**
   - Update all tests that call `advance_slot()`
   - Add `.await` where needed
   - Update mocks if needed

### Phase 4: Block Processing Integration

#### Add VRF Collection Hook

In block validation/processing code:

```rust
// When validating block's VRF proof
if let Ok(vrf_output) = verify_vrf_proof(&block.vrf_proof) {
    ouroboros_state.collect_block_vrf(block.slot, vrf_output)?;
}
```

#### Location

Likely in `crates/cardano-consensus/src/block_validation.rs` or similar.

## Testing Strategy

### Unit Tests

Update existing `ouroboros.rs` tests:

```rust
#[tokio::test]
async fn test_epoch_transition_with_handler() {
    let ledgerdb = Arc::new(MemoryBackend::new());
    let params = ProtocolParameters::testnet();
    let genesis = Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap();

    let mut state = OuroborosState::new(genesis, params.clone(), ledgerdb);

    // Advance through epoch boundary
    state.advance_slot(SlotNo(432000)).await.unwrap();

    // Verify epoch changed
    assert_eq!(state.current_epoch, EpochNo(1));

    // Verify nonce evolved
    assert_ne!(state.epoch_nonce, genesis);
}
```

### Integration Tests

Add to `tests/consensus/test_ouroboros_protocol.rs`:

```rust
#[tokio::test]
async fn test_multi_epoch_with_vrf_collection() {
    // Test VRF collection + epoch transitions
    // Verify nonces evolve correctly
    // Verify snapshots created
}
```

## Breaking Changes

### API Changes

1. **`OuroborosState` becomes generic**
   ```rust
   // Before
   pub struct OuroborosState { ... }

   // After
   pub struct OuroborosState<L: LedgerDatabase> { ... }
   ```

2. **`advance_slot` becomes async**
   ```rust
   // Before
   pub fn advance_slot(&mut self, new_slot: SlotNo) -> Result<()>

   // After
   pub async fn advance_slot(&mut self, new_slot: SlotNo) -> Result<()>
   ```

3. **Constructor requires LedgerDB**
   ```rust
   // Before
   OuroborosState::new(genesis_hash, protocol_params)

   // After
   OuroborosState::new(genesis_hash, protocol_params, ledgerdb)
   ```

### Migration Guide

For code using `OuroborosState`:

```rust
// OLD CODE
let mut state = OuroborosState::new(genesis, params);
state.advance_slot(SlotNo(100))?;

// NEW CODE
let ledgerdb = Arc::new(MemoryBackend::new());
let mut state = OuroborosState::new(genesis, params, ledgerdb);
state.advance_slot(SlotNo(100)).await?;
```

## Rollout Plan

### Stage 1: Preparation (30 min)
- ✅ Create this integration document
- ⏸️ Review breaking changes with team
- ⏸️ Create feature branch: `feat/integrate-epoch-handler`

### Stage 2: Implementation (1 hour)
- ⏸️ Modify `OuroborosState` structure
- ⏸️ Replace stub methods
- ⏸️ Update constructor
- ⏸️ Make `advance_slot` async

### Stage 3: Integration (30 min)
- ⏸️ Update call sites
- ⏸️ Add VRF collection hooks
- ⏸️ Update tests

### Stage 4: Validation (30 min)
- ⏸️ Run full test suite
- ⏸️ Fix any compilation errors
- ⏸️ Verify epoch transitions work

### Stage 5: Documentation (15 min)
- ⏸️ Update API documentation
- ⏸️ Update migration guide
- ⏸️ Create PR

## Success Criteria

- ✅ `OuroborosState` no longer has stub implementations
- ✅ Epoch transitions use `EpochTransitionHandler`
- ✅ VRF outputs collected during block processing
- ✅ All tests pass
- ✅ No regressions in existing functionality
- ✅ Clean build with zero warnings

## Risk Mitigation

### Risk 1: Breaking Changes
**Impact**: High
**Likelihood**: High
**Mitigation**:
- Create comprehensive migration guide
- Use feature flags for gradual rollout
- Keep old API temporarily for backward compatibility

### Risk 2: Async/Await Complexity
**Impact**: Medium
**Likelihood**: Medium
**Mitigation**:
- Use `tokio::test` for all tests
- Ensure all callers are in async contexts
- Add helper synchronous wrappers if needed

### Risk 3: Generic Type Complexity
**Impact**: Low
**Likelihood**: Low
**Mitigation**:
- Use type aliases: `type StandardOuroborosState = OuroborosState<MemoryBackend>`
- Document generic parameters clearly
- Provide concrete examples

## Next Steps

1. **Review this document** with consensus team
2. **Create feature branch** for integration work
3. **Implement Phase 1** (add handler to state)
4. **Run preliminary tests** to catch issues early
5. **Complete Phases 2-4** systematically
6. **Create PR** with full test coverage

## Dependencies

- ✅ `EpochTransitionHandler` implementation complete
- ✅ Integration tests written
- ✅ Documentation created
- ⏸️ Team review and approval
- ⏸️ Feature branch created

## Timeline

- **Preparation**: 30 minutes
- **Implementation**: 1.5 hours
- **Testing**: 30 minutes
- **Documentation**: 15 minutes
- **Total**: ~2.5 hours

## References

- [GAP-001 Implementation Report](./GAP-001_EPOCH_TRANSITION_COMPLETE.md)
- [Integration Tests Documentation](./GAP-001_INTEGRATION_TESTS.md)
- [HASKELL_COMPATIBILITY_GAPS.md](../architecture/HASKELL_COMPATIBILITY_GAPS.md)

---

**Status**: 🔄 READY FOR IMPLEMENTATION
**Priority**: P1
**Assignee**: Consensus Team
**Estimated Completion**: 2-3 hours
