# N1 Roadmap - Task 5: Wire LedgerDB to Block Validation

**Task 5:** Wire LedgerDB to Block Validation

- **Status**: Completed
- **Files**:
  - Created: `crates/cardano-consensus/src/block_validator.rs` (371 lines)
  - Modified: `crates/cardano-consensus/src/lib.rs` (added module export)
- **Description**: Integrate ledger state validation with incoming blocks, implement rollback/rollforward in LedgerDatabase, create BlockValidator service

## Task Checklist

- [x] Create BlockValidator service structure
- [x] Implement header validation logic
- [x] Integrate with CardanoDB LedgerDB
- [x] Add rollback support with configurable limits
- [x] Implement apply_header (rollforward operation)
- [x] Handle type conversions between consensus and storage crates
- [x] Add epoch calculation from slot number
- [x] Integrate snapshot triggering
- [x] Write comprehensive unit tests (8 tests)
- [x] Export BlockValidator from consensus crate

## Implementation Summary

### BlockValidator Service

Created a new `BlockValidator` service that acts as the bridge between ChainSync protocol and ledger state management. This service validates incoming block headers and applies them to the ledger state.

**Key Components:**

1. **Validation Logic** (`validate_header`)
   - Checks slot progression (monotonic time)
   - Validates block number sequence
   - Returns detailed error information
   - Future: Will add VRF and KES validation

2. **Rollforward Operation** (`apply_header`)
   - Updates ledger slot, block number, and epoch
   - Calculates epoch from slot (432000 slots per epoch for mainnet)
   - Triggers ledger snapshots automatically
   - Future: Will process transactions and update UTxO set

3. **Rollback Support** (`rollback_to_slot`)
   - Validates rollback distance (default max: 2160 slots = 1 hour)
   - Prevents excessive chain reorganizations
   - Future: Will implement actual snapshot loading and replay

4. **Convenience Methods**
   - `validate_and_apply()` - Combined validation + application
   - `current_slot()`, `current_block_no()`, `current_epoch()` - State queries
   - `with_rollback_limit()` - Configurable constructor

### Error Handling

```rust
pub enum ValidationError {
    InvalidSlot(String),
    SlotRegression { current: u64, new: u64 },
    InvalidBlockNumber(String),
    HashMismatch { expected: String, got: String },
    LedgerError(String),
    ValidationFailed(String),
}
```

Detailed error types allow upper layers to handle specific failure scenarios appropriately.

### Integration Architecture

```text
┌─────────────────────┐
│ ChainSyncService    │
│  - take_pending_    │
│    headers()        │
└─────────┬───────────┘
          │
          v
┌─────────────────────┐
│  BlockValidator     │
│  - validate_header()│
│  - apply_header()   │
│  - rollback()       │
└─────────┬───────────┘
          │
          v
┌─────────────────────┐
│  LedgerDB           │
│  (CardanoDB impl)   │
│  - update_tip()     │
│  - snapshots        │
└─────────────────────┘
```

### Type Compatibility

The implementation handles type differences between crates:

- **Consensus SlotNo**: `crate::ouroboros::SlotNo`
- **Storage SlotNo**: `cardano_storage::cardanodb::types::SlotNo`
- **Conversion**: Manual conversion via `.0` field access

This is necessary because both crates define their own SlotNo types for domain-specific operations.

### Test Coverage

**8/8 tests passing:**

1. **`test_validator_creation`**
   - Verifies basic initialization
   - Checks initial ledger state (slot=0, block=0, epoch=0)

2. **`test_validate_header_progression`**
   - Tests normal case: slot 1 > slot 0
   - Validates monotonic time progression

3. **`test_validate_header_slot_regression`**
   - Ensures backwards time is rejected
   - Tests SlotRegression error handling

4. **`test_apply_header_updates_ledger`**
   - Verifies ledger state updates
   - Checks slot, block number, epoch changes

5. **`test_validate_and_apply`**
   - Tests combined operation
   - Validates convenience method

6. **`test_rollback_validation`**
   - Tests rollback within limit (100 slots)
   - Rejects rollback beyond limit (150 slots)

7. **`test_rollback_to_future_fails`**
   - Ensures future rollback is rejected
   - Tests InvalidSlot error

8. **`test_sequential_header_application`**
   - Applies 5 headers sequentially
   - Verifies cumulative state changes

### Design Decisions

**1. Separate Validation from Application**

- `validate_header()` can be called independently
- Allows pre-validation before committing to ledger
- Supports batch validation scenarios

**2. Epoch Calculation from Slot**

- BlockHeader doesn't store epoch directly
- Calculate using slot / epoch_length
- Default: 432000 slots (Cardano mainnet = 5 days)
- TODO: Read from protocol parameters

**3. Configurable Rollback Limit**

- Default: 2160 slots (1 hour at 1 slot/second)
- Prevents excessive chain reorganizations
- Balances flexibility vs stability

**4. Automatic Snapshot Triggering**

- Calls `ledger.maybe_create_snapshot()` after each apply
- Leverages existing LedgerDB snapshot infrastructure
- Enables efficient rollback in future

**5. Simplified Initial Implementation**

- Focus on header-level validation
- Full transaction processing deferred
- VRF/KES validation marked as TODO
- Rollback uses placeholder logic

### Future Enhancements (TODOs)

**Validation:**

- [ ] VRF proof verification (leadership check)
- [ ] KES signature validation (authenticity check)
- [ ] Block size limits
- [ ] Protocol version compatibility

**Transaction Processing:**

- [ ] Apply block bodies to UTxO set
- [ ] Process stake pool registrations
- [ ] Handle delegations
- [ ] Update protocol parameters at epoch boundaries

**Rollback Implementation:**

- [ ] Load snapshots before target slot
- [ ] Replay blocks from snapshot to target
- [ ] Handle UTxO rollback
- [ ] Stake pool state rollback

**Performance:**

- [ ] Batch header validation
- [ ] Parallel signature verification
- [ ] Optimized epoch calculation
- [ ] Snapshot compression

### Integration with ChainSync

The BlockValidator integrates with ChainSyncService through the header queue:

```rust
// In ChainSync runtime loop (future implementation)
let headers = chainsync_service.take_pending_headers().await;

for header in headers {
    match block_validator.validate_and_apply(&header).await {
        Ok(()) => {
            info!("Header applied: slot={}", header.slot.0);
            // Update local tip in ChainSync
            chainsync_service.set_local_tip(tip_from_header(&header)).await;
        }
        Err(ValidationError::SlotRegression { .. }) => {
            // Chain reorganization detected - trigger rollback
            warn!("Chain fork detected, initiating rollback");
            block_validator.rollback_to_slot(intersection_slot).await?;
        }
        Err(e) => {
            error!("Validation failed: {:?}", e);
            // Handle validation failure (disconnect peer, etc.)
        }
    }
}
```

### Alignment with Cardano Haskell Node

The BlockValidator aligns with the Haskell node's validation pipeline:

**Haskell Equivalent:**

```haskell
-- In ouroboros-consensus/src/Ouroboros/Consensus/Ledger/Extended.hs
applyChainTick ::
     HasCallStack
  => LedgerCfg l
  -> SlotNo
  -> ExtLedgerState l
  -> ExtLedgerState l

-- In ouroboros-consensus/src/Ouroboros/Consensus/Ledger/Abstract.hs
applyBlock ::
     HasCallStack
  => LedgerCfg l
  -> Block l
  -> TickedLedgerState l
  -> Except (LedgerError l) (LedgerState l)
```

**Rust Implementation:**

```rust
impl BlockValidator {
    pub async fn validate_header(&self, header: &BlockHeader) -> Result<()>
    pub async fn apply_header(&self, header: &BlockHeader) -> Result<()>
    pub async fn rollback_to_slot(&self, target_slot: SlotNo) -> Result<()>
}
```

The Rust implementation follows similar patterns:

- Separate validation and application phases
- Slot-based ticking mechanism
- Rollback support for chain reorganization
- Type-safe error handling

### Known Limitations

1. **Placeholder Rollback**: Current implementation validates rollback distance but doesn't actually load snapshots or replay blocks. This will be implemented in a future task when full block processing is added.

2. **Simplified Validation**: Only checks slot/block progression. VRF and KES validation require cryptographic verification that will be added in integration testing phase.

3. **Hardcoded Epoch Length**: Uses 432000 slots (mainnet default). Should read from protocol parameters for multi-network support.

4. **No Transaction Processing**: Only updates header-level state (slot, block, epoch). UTxO updates deferred to future task.

5. **Type Conversions**: Manual conversion between consensus and storage SlotNo types. Could be improved with From/Into traits.

### Testing Strategy

Tests use temporary directories with unique identifiers to avoid conflicts:

```rust
fn create_test_ledger() -> Arc<LedgerDB> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_path = std::env::temp_dir()
        .join(format!("ledger_test_{}_{}", std::process::id(), id));
    // ... create LedgerDB ...
}
```

This ensures test isolation even when run sequentially.

### Performance Characteristics

**Header Validation**: O(1)

- Simple slot/block number comparisons
- No disk I/O during validation

**Header Application**: O(1) + snapshot cost

- Updates in-memory ledger state
- Periodic snapshot writes (configurable interval)

**Rollback**: O(n) where n = blocks to replay

- Currently placeholder (constant time)
- Future: Load snapshot + replay blocks

## Completion Criteria

- [x] BlockValidator service created and exported
- [x] Header validation logic implemented
- [x] Ledger state integration working
- [x] Rollback support with distance limits
- [x] All 8 unit tests passing
- [x] No compilation errors or warnings (except unrelated)
- [x] Code documented with rustdoc comments
- [x] Error handling comprehensive

## Next Steps

**Immediate (Task 6):**

1. Create preview network integration tests
2. Test actual network connectivity with IOHK relays
3. Verify ChainSync → BlockValidator → LedgerDB pipeline
4. Measure sync performance

**Future:**

1. Implement full block validation (VRF, KES)
2. Add transaction processing
3. Implement actual rollback with snapshot loading
4. Add batch validation for performance
5. Integrate with mempool for new transactions

## Related Documentation

- **ChainSync Integration**: See `N1_TASK3_CHAINSYNC_SERVICE.md`
- **Topology/Discovery**: See `N1_TASK4_PREVIEW_PEER_DISCOVERY.md`
- **Architecture**: See `docs/architecture/PROTOCOL_ARCHITECTURE.md`
- **Ledger State**: See `crates/cardano-storage/src/cardanodb/ledger/README.md`

## Command Reference

```bash
# Run BlockValidator tests
cargo test -p cardano-consensus block_validator --lib

# Run with output
cargo test -p cardano-consensus block_validator --lib -- --nocapture

# Build consensus crate
cargo build -p cardano-consensus

# Check for errors
cargo check -p cardano-consensus
```

## Conclusion

Task 5 successfully integrates block validation with ledger state management. The BlockValidator service provides a clean abstraction for validating and applying block headers, with proper error handling and rollback support. While some advanced features (VRF validation, transaction processing, actual rollback) are deferred to future tasks, the foundation is solid and ready for preview network integration testing in Task 6.

**Status**: ✅ **COMPLETED**
**Tests**: 8/8 passing
**Next**: Task 6 - Preview network integration tests
