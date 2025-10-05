# Cardano-Base-Rust Integration Audit

**Date**: October 5, 2025
**Priority**: P0 (Critical)
**Status**: In Progress

## Executive Summary

This audit assesses the current usage of [cardano-base-rust](https://github.com/FractionEstate/cardano-base-rust) libraries in the codebase to identify:
1. **Reimplemented functionality** that exists in official libraries
2. **Missing integrations** where official libraries should be used
3. **Replacement priorities** and effort estimates

### Critical Findings

🚨 **HIGH PRIORITY**: KES implementation (713 lines) is a **temporary stub** with explicit TODO to replace with `cardano-crypto-class::kes::compact_sum::CompactSum7Kes`

✅ **GOOD**: VRF properly using `cardano-vrf-pure`
✅ **GOOD**: Basic types (SlotNo, EpochNo, BlockNo) using `cardano-slotting`
⚠️ **NEEDS REVIEW**: CBOR serialization using `minicbor` directly
⚠️ **NEEDS REVIEW**: Epoch calculations may have manual logic

---

## 1. Current Cardano-Base-Rust Usage

### Imported Dependencies

From workspace `Cargo.toml`:
```toml
[workspace.dependencies]
# Cardano Base Rust - Official VRF implementation
cardano-vrf-pure = { git = "https://github.com/FractionEstate/cardano-base-rust", branch = "master" }
cardano-crypto-class = { git = "https://github.com/FractionEstate/cardano-base-rust", branch = "master" }
```

From `crates/cardano-storage/Cargo.toml`:
```toml
cardano-slotting = { git = "https://github.com/FractionEstate/cardano-base-rust", branch = "master" }
cardano-binary = { git = "https://github.com/FractionEstate/cardano-base-rust", branch = "master" }
cardano-crypto-class = { git = "https://github.com/FractionEstate/cardano-base-rust", branch = "master" }
```

From `crates/cardano-crypto/Cargo.toml`:
```toml
cardano-crypto-class.workspace = true
```

### Current Usage Patterns

#### ✅ VRF (GOOD - Properly Integrated)

**Location**: `crates/cardano-crypto/src/vrf/backend.rs`

```rust
use cardano_crypto_class::seed::mk_seed_from_bytes;
use cardano_crypto_class::vrf::praos_batch::PraosBatchCompatVRF;
use cardano_crypto_class::vrf::VRFAlgorithm;
```

**Status**: ✅ Complete integration with official VRF library
**Compatibility**: High - using official Praos batch-compatible VRF

#### ✅ Ed25519 (GOOD - Using Library)

**Location**: `crates/cardano-crypto/src/ed25519/mod.rs`

```rust
use cardano_crypto_class::dsign::ed25519::{
    Ed25519, Ed25519SigningKey, Ed25519VerifyingKey,
};
use cardano_crypto_class::dsign::DsignAlgorithm;
```

**Status**: ✅ Using official Ed25519 implementation
**Compatibility**: High - standard DSA interface

#### ✅ Slotting Types (GOOD - Using Library Types)

**Location**: `crates/cardano-storage/src/cardanodb/types.rs`

```rust
pub use cardano_slotting::{BlockNo, EpochNo, SlotNo};
```

**Status**: ✅ Using official types for slots, epochs, blocks
**Compatibility**: High - ensures type compatibility

---

## 2. Critical Issue: KES Implementation

### 🚨 Problem: Temporary Stub Implementation

**Location**: `crates/cardano-crypto/src/kes/mod.rs` (713 lines)

**Explicit TODO in File Header**:
```rust
//! KES (Key Evolving Signature) Implementation
//!
//! **IMPORTANT: These are temporary stub implementations for compatibility.**
//! The real KES implementation (CompactSum7Kes with 128 periods) is available in
//! the `cardano-crypto-class` crate from cardano-base-rust.
//!
//! TODO: Replace these stubs with proper wrappers around cardano-crypto-class types:
//! - KesSecretKey → Wrap cardano_crypto_class::kes::compact_sum::CompactSum7Kes signing key
//! - KesPublicKey → Wrap cardano_crypto_class::kes::compact_sum::CompactSum7Kes verifying key
//! - KesSignature → Wrap SignedKes structure
```

### Current Stub Implementation

Our implementation:
- **713 lines** of custom KES code
- Uses Ed25519 + Blake2b-512 for key derivation
- Implements depth=6 (64 periods, max_period=62)
- Manual evolution logic
- File I/O for Cardano CLI format (TextEnvelope JSON)

### Target: cardano-crypto-class CompactSum7Kes

The official implementation:
- `cardano_crypto_class::kes::compact_sum::CompactSum7Kes`
- Depth=7 (128 periods) - **More compatible with mainnet**
- Fully tested against Haskell implementation
- Proper MMM tree-based KES with sum composition
- Binary-compatible with cardano-cli keys

#### CompactSum7Kes Type Structure

```rust
// Type alias chain (recursive composition)
pub type CompactSum0Kes = CompactSingleKes<Ed25519>;
pub type CompactSum1Kes = CompactSumKes<CompactSum0Kes, Blake2b256>;
pub type CompactSum2Kes = CompactSumKes<CompactSum1Kes, Blake2b256>;
pub type CompactSum3Kes = CompactSumKes<CompactSum2Kes, Blake2b256>;
pub type CompactSum4Kes = CompactSumKes<CompactSum3Kes, Blake2b256>;
pub type CompactSum5Kes = CompactSumKes<CompactSum4Kes, Blake2b256>;
pub type CompactSum6Kes = CompactSumKes<CompactSum5Kes, Blake2b256>;
pub type CompactSum7Kes = CompactSumKes<CompactSum6Kes, Blake2b256>;

// Tree structure: 7 levels = 2^7 = 128 periods (0-127)
```

#### API Methods (KesAlgorithm Trait)

```rust
impl KesAlgorithm for CompactSum7Kes {
    type VerificationKey = Vec<u8>;  // Blake2b-256 hash (32 bytes)
    type SigningKey = CompactSumSigningKey<CompactSum6Kes, Blake2b256>;
    type Signature = CompactSumSignature<CompactSum6Kes, Blake2b256>;
    type Context = ();

    // Constants
    const ALGORITHM_NAME: &'static str = "Ed25519"; // Base algorithm
    const SEED_SIZE: usize = 32;
    const VERIFICATION_KEY_SIZE: usize = 32; // Blake2b-256
    const SIGNING_KEY_SIZE: usize = /* calculated */;
    const SIGNATURE_SIZE: usize = /* calculated */;

    // Core methods
    fn total_periods() -> Period { 128 }

    fn gen_key_kes_from_seed_bytes(seed: &[u8]) -> Result<Self::SigningKey, KesMError>;

    fn derive_verification_key(signing_key: &Self::SigningKey)
        -> Result<Self::VerificationKey, KesMError>;

    fn sign_kes(
        context: &Self::Context,
        period: Period,
        message: &[u8],
        signing_key: &Self::SigningKey,
    ) -> Result<Self::Signature, KesMError>;

    fn verify_kes(
        context: &Self::Context,
        verification_key: &Self::VerificationKey,
        period: Period,
        message: &[u8],
        signature: &Self::Signature,
    ) -> Result<(), KesMError>;

    fn update_kes(
        context: &Self::Context,
        signing_key: Self::SigningKey,
        period: Period,
    ) -> Result<Option<Self::SigningKey>, KesMError>;

    fn forget_signing_key_kes(signing_key: Self::SigningKey);

    // Serialization
    fn raw_serialize_verification_key_kes(key: &Self::VerificationKey) -> Vec<u8>;
    fn raw_deserialize_verification_key_kes(bytes: &[u8]) -> Option<Self::VerificationKey>;
    fn raw_serialize_signature_kes(signature: &Self::Signature) -> Vec<u8>;
    fn raw_deserialize_signature_kes(bytes: &[u8]) -> Option<Self::Signature>;
}
```

#### DirectSerialise for File I/O

```rust
// CompactSumSigningKey serializes to:
// 1. Child signing key (recursive)
// 2. Right subtree seed (32 bytes, MLocked)
// 3. Left subtree verification key (32 bytes)
// 4. Right subtree verification key (32 bytes)

impl DirectSerialise for CompactSumSigningKey<D, H> {
    fn direct_serialise(&self, push: &mut dyn FnMut(*const u8, usize) -> DirectResult<()>)
        -> DirectResult<()>;
}

impl DirectDeserialise for CompactSumSigningKey<D, H> {
    fn direct_deserialise(pull: &mut dyn FnMut(*mut u8, usize) -> DirectResult<()>)
        -> DirectResult<Self>;
}
```

#### Usage Example (from tests)

```rust
use cardano_crypto_class::kes::{CompactSum7Kes, KesAlgorithm};

// Generate key from seed
let seed_bytes = [42u8; 32];
let signing_key = CompactSum7Kes::gen_key_kes_from_seed_bytes(&seed_bytes)?;

// Derive public key
let vk = CompactSum7Kes::derive_verification_key(&signing_key)?;

// Sign at period 0
let message = b"block data";
let signature = CompactSum7Kes::sign_kes(&(), 0, message, &signing_key)?;

// Verify signature
CompactSum7Kes::verify_kes(&(), &vk, 0, message, &signature)?;

// Evolve to period 1
let signing_key = CompactSum7Kes::update_kes(&(), signing_key, 0)?
    .ok_or(KesMError::Kes(KesError::KeyExpired))?;

// Sign at period 1
let signature = CompactSum7Kes::sign_kes(&(), 1, message, &signing_key)?;
```

### Impact Assessment

| Aspect | Current Stub | Official Library | Impact |
|--------|-------------|------------------|--------|
| **Periods** | 64 (depth=6) | 128 (depth=7) | Medium - May cause compatibility issues |
| **Algorithm** | Simplified | Full MMM Sum Composition | High - Correctness critical |
| **Testing** | Our tests only | Haskell-verified | High - Security critical |
| **Maintenance** | 713 lines to maintain | Zero maintenance | High - Technical debt |
| **Compatibility** | Best-effort | Binary-compatible | High - Essential for mainnet |

#### API Comparison

| Feature | Our Implementation | CompactSum7Kes | Migration Complexity |
|---------|-------------------|----------------|---------------------|
| **Key Generation** | `KesSecretKey::generate(depth: u32)` | `CompactSum7Kes::gen_key_kes_from_seed_bytes(seed: &[u8])` | Medium - Different API |
| **From File** | `KesSecretKey::from_file(path, period)` | Need wrapper + DirectDeserialise | Medium - Need file I/O wrapper |
| **Evolution** | `key.evolve()` returns `Result<Self>` | `update_kes(ctx, key, period)` returns `Result<Option<Self>>` | Low - Similar semantics |
| **Signing** | `key.sign(period, message)` | `sign_kes(ctx, period, message, key)` | Low - Just reorder params |
| **Verification** | `public_key.verify(period, message, sig)` | `verify_kes(ctx, vk, period, message, sig)` | Low - Just reorder params |
| **Current Period** | `key.current_period()` | Track separately (not in lib) | Low - Add wrapper field |
| **Is Expired** | `key.is_expired()` | Check period >= 128 | Low - Simple logic |
| **To Public** | `key.to_public()` | `derive_verification_key(&key)` | Low - Direct mapping |
| **File Format** | TextEnvelope JSON + CBOR | DirectSerialise binary | High - Different format |

#### Key Differences

1. **Stateless API**: CompactSum7Kes doesn't track current period - we need to track it ourselves
2. **Context Parameter**: All methods take `&()` context (unit type for CompactSum7Kes)
3. **Evolution Returns Option**: `update_kes` returns `Option<SigningKey>` (None when expired)
4. **Seed-Based Generation**: Uses 32-byte seed instead of random generation
5. **Binary Serialization**: Uses DirectSerialise, not JSON TextEnvelope (but we can add wrapper)

### Replacement Priority

**Priority**: P0 (Critical)
**Effort**: 3-4 days
**Risk**: Medium (requires careful testing)
**Blocker for**: Production mainnet deployment

---

## 3. Potential Issue: Epoch Calculations

### Current Implementation

**Location**: `crates/cardano-consensus/src/ouroboros.rs`

Manual epoch calculations:
```rust
impl SlotNo {
    pub fn to_epoch(&self, params: &ProtocolParameters) -> EpochNo {
        EpochNo(self.0 / params.epoch_length)
    }
}

impl EpochNo {
    pub fn first_slot(&self, params: &ProtocolParameters) -> SlotNo {
        SlotNo(self.0 * params.epoch_length)
    }

    pub fn last_slot(&self, params: &ProtocolParameters) -> SlotNo {
        SlotNo((self.0 + 1) * params.epoch_length - 1)
    }
}
```

### Cardano-Slotting Library

**Question**: Does `cardano-slotting` provide `EpochInfo` or similar utilities for:
- Slot → Epoch conversion
- Epoch boundaries
- Epoch start/end calculations
- Shelley-era epoch length handling

**TODO**: Investigate if cardano-slotting has:
- `EpochInfo` struct
- Conversion utilities
- Byron/Shelley era handling

**Priority**: P1 (High)
**Effort**: 1-2 days investigation + 1-2 days refactoring

---

## 4. Potential Issue: CBOR Serialization

### Current Implementation

Using `minicbor` extensively:

**Locations**:
- `crates/cardano-storage/src/chaindb/mod.rs`
- `crates/cardano-storage/src/ledgerdb/mod.rs`
- `crates/cardano-node/src/keys/mod.rs`
- `tests/network/integration.rs`

**Example**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, minicbor::Encode, minicbor::Decode)]
pub struct BlockMetadata {
    pub slot: u64,
    pub block_hash: Vec<u8>,
    // ...
}

let data = minicbor::to_vec(metadata)?;
let metadata = minicbor::decode(&data)?;
```

### Cardano-Binary Library

**Available**: `cardano-binary` imported in `cardano-storage`

**Question**: Should we use `cardano-binary` for Cardano-specific CBOR?

**Benefits of cardano-binary**:
- Guaranteed compatibility with Cardano CBOR formats
- Handles Cardano-specific encodings (e.g., tag 24 for embedded CBOR)
- Tested against Haskell serialization

**Trade-offs**:
- `minicbor` may be fine for internal formats
- Cardano wire protocol should use `cardano-binary`

**TODO**:
- Audit CBOR usage to categorize:
  - Wire protocol (should use cardano-binary)
  - Internal storage (minicbor may be OK)
  - Cardano CLI compatibility (should use cardano-binary)

**Priority**: P2 (Medium)
**Effort**: 2-3 days audit + 1-2 days refactoring

---

## 5. Available Cardano-Base-Rust Modules

From the repository structure, cardano-base-rust likely provides:

### ✅ Currently Using
- `cardano-vrf-pure` - VRF for leader selection
- `cardano-crypto-class` - Crypto primitives (DSA, KES, VRF)
- `cardano-slotting` - Slot/Epoch/Block number types
- `cardano-binary` - CBOR serialization

### ❓ Potentially Available (Need to Check)
- `cardano-ledger-core` - Core ledger types
- `cardano-ledger-shelley` - Shelley-era ledger
- `cardano-ledger-byron` - Byron-era ledger
- `cardano-protocol-tpraos` - TPraos protocol
- `cardano-protocol-praos` - Praos protocol

**TODO**: Review full cardano-base-rust repository to identify additional modules

---

## 6. Replacement Plan

### Phase 1: KES Replacement (P0 - Critical)

**Goal**: Replace 713-line KES stub with cardano-crypto-class wrappers

**Steps**:
1. **Study cardano-crypto-class KES API** (4 hours)
   - Read CompactSum7Kes documentation
   - Understand key generation, evolution, signing
   - Document API differences

2. **Create wrapper types** (8 hours)
   ```rust
   // New: crates/cardano-crypto/src/kes/wrappers.rs
   use cardano_crypto_class::kes::compact_sum::{
       CompactSum7KesSigningKey,
       CompactSum7KesVerifyingKey,
       SignedKes,
   };

   pub struct KesSecretKey {
       inner: CompactSum7KesSigningKey,
       // Compatibility fields
   }

   impl KesSecretKey {
       pub fn generate(depth: u32) -> Self { /* wrap library */ }
       pub fn from_file(path: &Path, period: u64) -> Result<Self> { /* parse CLI format */ }
       pub fn evolve(&self) -> Result<Self> { /* wrap library */ }
       pub fn sign(&self, period: u64, message: &[u8]) -> Result<KesSignature> { /* wrap library */ }
       // ... maintain existing API
   }
   ```

3. **Update BlockForger** (4 hours)
   - Update KesKey in block_production.rs
   - Ensure auto-evolution still works
   - Update operational certificate handling

4. **Migrate tests** (8 hours)
   - Update all KES tests
   - Add compatibility tests (CLI format, evolution, signing)
   - Verify depth=7 (128 periods) works

5. **Verify integration** (4 hours)
   - Test with real cardano-cli keys
   - Verify block signing
   - Test evolution during forging

**Total Effort**: 3-4 days
**Risk**: Medium (careful API migration needed)

### Phase 2: Slotting Audit (P1 - High)

**Goal**: Ensure proper use of cardano-slotting utilities

**Steps**:
1. Investigate cardano-slotting API (4 hours)
2. Compare to our manual calculations (2 hours)
3. Refactor if needed (8-16 hours, depends on findings)

**Total Effort**: 1-3 days

### Phase 3: CBOR Audit (P2 - Medium)

**Goal**: Use cardano-binary for Cardano-specific serialization

**Steps**:
1. Categorize CBOR usage (4 hours)
2. Identify wire protocol serialization (2 hours)
3. Replace with cardano-binary where needed (8-16 hours)

**Total Effort**: 2-3 days

### Phase 4: Documentation (P2 - Medium)

**Goal**: Document cardano-base-rust integration

**Steps**:
1. Create integration guide (4 hours)
2. Update architecture docs (2 hours)
3. Add examples (2 hours)

**Total Effort**: 1 day

---

## 7. Testing Strategy

### KES Replacement Testing

**Critical Tests**:
1. **Key Generation**
   - Generate KES keys
   - Verify depth=7 (128 periods)
   - Check public key derivation

2. **Key Evolution**
   - Evolve forward 1, 10, 50, 100 periods
   - Verify can't evolve backwards
   - Check expiration at period 127

3. **Block Signing**
   - Sign blocks at various periods
   - Verify signatures with public key
   - Test auto-evolution during forging

4. **CLI Compatibility**
   - Load keys from cardano-cli JSON format
   - Save keys to cardano-cli format
   - Verify operational certificate parsing

5. **Integration Tests**
   - Full block production workflow
   - KES evolution service (GAP-005)
   - Network block propagation

### Compatibility Testing

**Haskell Node Compatibility**:
- Generate keys with our code, verify with cardano-cli
- Generate keys with cardano-cli, use in our code
- Sign blocks with our code, validate with Haskell node
- Cross-verify signatures

---

## 8. Risk Assessment

### High-Risk Areas

1. **KES Replacement** (Medium Risk)
   - **Risk**: Breaking block signing
   - **Mitigation**: Comprehensive testing, gradual rollout
   - **Fallback**: Keep stub as fallback during transition

2. **Binary Compatibility** (Medium Risk)
   - **Risk**: Keys/signatures not compatible with cardano-cli
   - **Mitigation**: Extensive cross-testing
   - **Fallback**: Verify formats match Haskell node exactly

3. **Performance Impact** (Low Risk)
   - **Risk**: Library may be slower than stub
   - **Mitigation**: Benchmark before/after
   - **Fallback**: Profile and optimize if needed

### Low-Risk Areas

1. **VRF** - Already using library ✅
2. **Ed25519** - Already using library ✅
3. **Slotting types** - Already using library types ✅

---

## 9. Success Criteria

### KES Replacement Success

- [ ] All KES tests passing with cardano-crypto-class
- [ ] Can load cardano-cli generated keys
- [ ] Can save keys in cardano-cli format
- [ ] Block signing works with evolved keys
- [ ] Signatures verify with cardano-cli
- [ ] Haskell node accepts our signed blocks
- [ ] 128-period KES (depth=7) working
- [ ] Performance within 10% of stub

### Overall Integration Success

- [ ] Zero duplicate implementations of cardano-base-rust functionality
- [ ] All crypto using official libraries
- [ ] CBOR using cardano-binary for wire protocol
- [ ] Comprehensive documentation of integration
- [ ] Updated examples and guides
- [ ] All tests passing
- [ ] Mainnet compatibility verified

---

## 10. Next Actions

### Immediate (This Week)

1. **Study cardano-crypto-class KES API**
   - Clone cardano-base-rust repository
   - Read CompactSum7Kes source code
   - Document API and differences

2. **Create KES wrapper prototype**
   - Implement basic wrapper for KesSecretKey
   - Test key generation and evolution
   - Verify CLI format compatibility

### Short-Term (Next 2 Weeks)

3. **Complete KES replacement**
   - Full wrapper implementation
   - Migrate all tests
   - Integration testing
   - Deploy to testnet

4. **Audit slotting and CBOR**
   - Complete investigation
   - Implement necessary changes
   - Update documentation

### Medium-Term (Next Month)

5. **Full integration review**
   - Verify all cardano-base-rust usage
   - Add any missing integrations
   - Performance optimization
   - Mainnet readiness assessment

---

## 11. References

### Repositories
- [cardano-base-rust](https://github.com/FractionEstate/cardano-base-rust) - Official Cardano Rust libraries
- [cardano-base](https://github.com/input-output-hk/cardano-base) - Original Haskell implementation

### Documentation
- Cardano KES Specification
- Sum Composition KES (MMM) Paper
- Cardano CLI Key Formats

### Related Documents
- `docs/reports/GAP-005_KES_EVOLUTION_ASSESSMENT.md` - KES evolution service design
- `docs/architecture/HASKELL_COMPATIBILITY_GAPS.md` - Overall compatibility status
- `docs/reports/PROJECT_STATUS_OCTOBER_2025.md` - Project status

---

## Changelog

- **2025-10-05**: Initial audit started
  - Identified KES stub with explicit TODO
  - Confirmed VRF, Ed25519, slotting types using libraries
  - Created replacement plan for KES (P0 priority)
  - Identified areas needing investigation (slotting, CBOR)

---

**Next Update**: After KES API study (2025-10-06)
