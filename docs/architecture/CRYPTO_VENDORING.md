# Crypto Module Vendoring

## Overview

The cryptographic modules from [cardano-base-rust](https://github.com/FractionEstate/cardano-base-rust) have been **vendored** (copied directly into this repository) to simplify dependency management and provide greater control over the crypto implementation.

**Status:** ✅ Complete (October 2025)

## Vendored Modules

The following modules are now located in `crates/cardano-crypto/src/vendor/`:

### `crypto_class/` - Core Cryptographic Primitives

From `cardano-crypto-class` crate:

- **Direct Serialization** (`direct_serialise.rs`)
  - Zero-copy serialization/deserialization for cryptographic types
  - Implements `DirectSerialise` and `DirectDeserialise` traits for `Vec<u8>` and other types

- **KES (Key Evolving Signatures)** (`kes/`)
  - `CompactSum7Kes` - 128-period KES scheme (used by Cardano)
  - `single.rs` - Single-period KES base scheme (Ed25519)
  - `sum.rs` - Sum composition for multi-period KES
  - `compact_sum.rs` - Compact 7-level sum KES
  - `compact_single.rs` - Compact single KES

- **Digital Signatures** (`dsign/`)
  - `ed25519.rs` - Ed25519 signature scheme
  - Wraps `ed25519-dalek` with Cardano-specific serialization

- **Supporting Types**
  - `seed.rs` - Deterministic seed management and `SeedRng`
  - `mlocked_bytes.rs` - Memory-locked storage for sensitive data
  - `mlocked_seed.rs` - Memory-locked seed storage
  - `util.rs` - Utility functions for hex encoding, byte operations
  - `hash.rs` - Cryptographic hash functions (Blake2b, SHA256, SHA3)
  - `packed_bytes.rs` - Fixed-size byte arrays with efficient storage
  - `pinned_sized_bytes.rs` - Pinned memory for fixed-size byte arrays
  - `ffi.rs` - FFI utilities for C interop

### `vrf_pure/` - VRF Implementation

From `cardano-vrf-pure` crate:

- **VRF Protocols**
  - `common.rs` - Common VRF types and traits
  - `draft13.rs` - IETF Draft-13 VRF (Praos leader election)
  - `cardano_compat/` - Cardano-specific VRF compatibility layer

- **Cardano Compatibility** (`cardano_compat/`)
  - `mod.rs` - Main compatibility interface
  - `point.rs` - Curve25519 point operations
  - `scalar.rs` - Scalar field operations
  - `tests.rs` - Official test vectors from Cardano

## Test Vectors

Official Cardano VRF test vectors are stored in `crates/cardano-crypto/test-vectors/`:
- `vrf_ver03_standard_*` - Standard test vectors for VRF version 03
- `vrf_ver03_generated_*` - Generated test vectors for VRF version 03
- `vrf_ver13_standard_*` - Standard test vectors for Draft-13 VRF
- `vrf_ver13_generated_*` - Generated test vectors for Draft-13 VRF

## Cargo Features

The vendored code includes several optional features (currently disabled):

```toml
[features]
serde = []                      # Serde serialization support
kes-metrics = []                # KES operation metrics
mlocked-metrics = []            # Memory-locking metrics
vrf-debug = []                  # VRF debugging output
field-h2c-experimental = []     # Experimental hash-to-curve
```

These can be enabled in the future if needed.

## Dependencies

The vendored crypto code requires:

- `rand_core = "0.6"` - RNG traits (compatible with existing dependencies)
- `digest = "0.10"` - Cryptographic hash trait
- `sha3 = "0.10"` - SHA3 hash functions
- `ripemd = "0.1"` - RIPEMD hash functions
- `libc = "0.2"` - For `mlock()` system calls
- `num-bigint` - Arbitrary precision integers
- `num-traits` - Numeric trait abstractions

Cryptographic backends:
- `ed25519-dalek = "2.2"` - Ed25519 signatures
- `curve25519-dalek = "4.0"` - Curve25519 operations
- `blake2 = "0.10"` - Blake2b hashing
- `sha2 = "0.10"` - SHA256 hashing
- `blstrs = "0.7"` - BLS12-381 signatures

## Compatibility Notes

### rand_core Version

The vendored code has been adapted to work with **rand_core 0.6** instead of 0.9:

- Other dependencies (blstrs, ed25519-dalek, ff, group) use rand_core 0.6
- Upgrading to 0.9 would cause trait incompatibility issues
- Key changes made:
  - `OsRng` imported from `rand::rngs::OsRng` (not `rand_core::OsRng`)
  - Implemented `try_fill_bytes` for `SeedRng` (required by RngCore 0.6)
  - Changed `fill_random()` to use `fill_bytes()` which panics on error (0.6 behavior)

### Path Updates

All internal imports have been updated:
- `crate::something` → `crate::vendor::crypto_class::something`
- `crate::util` → `crate::vendor::crypto_class::util`

### Removed Modules

The following modules were removed as they're not used by Cardano:
- `dsign/ecdsa_secp256k1.rs` - ECDSA signatures (Bitcoin/Ethereum)
- `dsign/schnorr_secp256k1.rs` - Schnorr signatures (Bitcoin)

## Testing

All 131 tests pass:
- Unit tests from the original cardano-base-rust modules
- Official Cardano test vectors
- Property-based tests using `proptest`

Run tests:
```bash
cargo test -p cardano-crypto --lib
```

## Rationale for Vendoring

### Advantages

1. **No Git Submodule Issues** - Avoids submodule permission and sync problems
2. **Full Control** - Can modify vendored code without affecting upstream
3. **Simplified Build** - No external git dependencies for crypto
4. **Easier Debugging** - Code is directly in the repository
5. **Version Stability** - Not affected by upstream changes

### Disadvantages

1. **Manual Updates** - Need to manually sync upstream changes if needed
2. **Code Duplication** - Could diverge from upstream over time

## Maintenance

### Updating Vendored Code

If upstream cardano-base-rust has critical fixes:

1. Copy updated files from `external/cardano-base-rust/` to `crates/cardano-crypto/src/vendor/`
2. Update import paths: `crate::` → `crate::vendor::crypto_class::`
3. Ensure rand_core 0.6 compatibility
4. Run tests: `cargo test -p cardano-crypto`
5. Update test vectors if needed

### Attribution

All vendored code maintains original copyright headers:
```rust
// Vendored from cardano-base-rust (https://github.com/FractionEstate/cardano-base-rust)
// Licensed under Apache-2.0
```

## Related Documentation

- [Cardano Base Rust Integration](./CARDANO_BASE_RUST_MIGRATION_COMPLETE.md)
- [Crypto Integration Guide](../guides/CRYPTO_INTEGRATION_GUIDE.md)
- [Haskell Compatibility](./HASKELL_COMPATIBILITY_VERIFIED.md)
