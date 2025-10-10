# KES Rotation Alignment with Haskell Node (C2 Audit)

**Date**: 2025-10-10
**Status**: ✅ VERIFIED
**Roadmap Item**: C2 - Audit KES/VRF state transitions

## Summary

Audited the Rust node's KES rotation warning logic against the official Haskell `cardano-node` implementation to ensure operational compatibility.

## Haskell Node Behavior

Source: [`cardano-node/src/Cardano/Node/Tracing/Tracers/KESInfo.hs`](https://github.com/IntersectMBO/cardano-node/blob/main/cardano-node/src/Cardano/Node/Tracing/Tracers/KESInfo.hs)

### Warning Thresholds

```haskell
severityFor (Namespace _ _) (Just forgeStateInfo) = Just $
  let kesPeriodsUntilExpiry = max 0 (expiryKesPeriod - currKesPeriod')
  in if kesPeriodsUntilExpiry > 7
    then Info
    else if kesPeriodsUntilExpiry <= 1
      then Alert
      else Warning
```

**Threshold Summary:**

- **Info**: `> 7` periods remaining
- **Warning**: `2-7` periods remaining
- **Alert**: `<= 1` period remaining

### Log Messages

```haskell
forHuman forgeStateInfo =
  if kesPeriodsUntilExpiry > 7
  then "KES info startPeriod X currPeriod Y endPeriod Z, N KES periods until expiry."
  else "Operational key will expire in N KES periods."
```

## Rust Node Behavior

Source: `crates/cardano-consensus/src/block_production_service.rs:350-375`

### Current Implementation

```rust
// Warn if approaching expiration (within 10 periods)
if forger.is_kes_approaching_expiration(10) {
    warn!(
        "⚠️  KES key approaching expiration! Current: {}, Max: {}, Remaining: {}",
        current, max, remaining
    );

    self.emit_event(BlockProductionEvent::KesApproachingExpiration {
        current_period: current,
        max_period: max,
        remaining_periods: remaining,
    });
} else if remaining % 10 == 0 && remaining <= 50 {
    // Log periodic status when getting low
    info!(
        "KES status: period {}/{}, {} periods remaining",
        forger.current_kes_period(),
        forger.kes_max_period(),
        remaining
    );
}
```

**Threshold Summary:**

- **Warning**: `<= 10` periods remaining (more conservative than Haskell)
- **Periodic info**: Every 10 periods when `<= 50` remaining

## Comparison Analysis

| Aspect | Haskell Node | Rust Node | Assessment |
|--------|--------------|-----------|------------|
| **Warning threshold** | 7 periods | 10 periods | ✅ Rust more conservative (safer) |
| **Alert threshold** | 1 period | Not implemented | ⚠️ Could add explicit alert level |
| **Log message format** | "Operational key will expire in N KES periods" | "KES key approaching expiration! Current: X, Max: Y, Remaining: Z" | ✅ More detailed in Rust |
| **Periodic logging** | Not explicit | Every 10 periods when ≤50 | ✅ Additional visibility |
| **Event emission** | Trace events | `BlockProductionEvent::KesApproachingExpiration` | ✅ Rust has structured events |

## Verification Results

### ✅ **Core Functionality Aligned**

1. KES period tracking is implemented correctly
2. Warning emissions occur before expiration
3. Operators receive adequate notice for key rotation

### ✅ **Rust Implementation is More Conservative**

- Warning at 10 periods vs. Haskell's 7 gives operators more lead time
- This is **beneficial** and aligns with operational best practices

### ✅ **Additional Features in Rust**

1. Structured event system (`BlockProductionEvent`)
2. Periodic status logs for long-running operations
3. Explicit current/max period reporting

## Recommendations

### Optional Enhancements (Not Required for Compatibility)

1. **Add Alert Level** (Nice-to-have)

   ```rust
   if remaining == 1 {
       error!("🚨 CRITICAL: KES key expires in 1 period!");
   } else if remaining <= 7 {
       warn!("⚠️  KES key approaching expiration...");
   } else if remaining <= 50 {
       info!("KES status: {} periods remaining", remaining);
   }
   ```

2. **Harmonize Log Format** (Optional)
   - Keep current detailed format for structured logging
   - Add human-readable summary matching Haskell for familiarity

### Decision: Keep Current Implementation

**Rationale:**

- Current thresholds (10 periods) are **more conservative** than Haskell (7 periods), providing better operator safety
- Structured events enable programmatic monitoring (not available in Haskell)
- Log format provides more diagnostic information
- No operational compatibility issues identified

## Integration Test Coverage

Existing tests validate KES rotation logic:

```bash
cargo test -p cardano-consensus kes_rotation_warn
```

**Test Scenarios:**

- [x] Warning emitted when approaching expiration
- [x] Event contains correct period information
- [x] No warning when plenty of periods remain

## Conclusion

**Status**: ✅ **COMPLIANT**

The Rust node's KES rotation warning system is **fully compatible** with the Haskell node's operational behavior. The more conservative threshold (10 vs. 7 periods) provides **enhanced safety** for stake pool operators without introducing compatibility issues.

### Exit Criteria Met

- [x] KES rotation warnings match upstream logic structure
- [x] Warning thresholds are adequate for operator awareness
- [x] Test coverage validates expiration detection
- [x] No discrepancies that affect block production

### References

- Haskell implementation: `cardano-node/src/Cardano/Node/Tracing/Tracers/KESInfo.hs`
- Rust implementation: `crates/cardano-consensus/src/block_production_service.rs`
- Test suite: `cargo test -p cardano-consensus kes_rotation_warn`
- Roadmap: `docs/development/ROADMAP.md` (Item C2)
