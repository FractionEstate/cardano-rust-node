# KES Replacement Implementation Plan

**Date**: October 5, 2025
**Priority**: P0 (Critical)
**Estimated Effort**: 3-4 days
**Dependencies**: cardano-crypto-class from cardano-base-rust

## Overview

Replace our 713-line temporary KES stub implementation with proper wrappers around `cardano_crypto_class::kes::compact_sum::CompactSum7Kes` (128-period KES).

**Current**: Simplified 64-period implementation (depth=6)
**Target**: Official 128-period CompactSum7Kes (depth=7)
**Benefit**: Haskell-compatible, audited, binary-compatible with cardano-cli

## Phase 1: API Study ✅ COMPLETED

**Status**: ✅ Complete (October 5, 2025)

**Findings**:
- CompactSum7Kes provides full 128-period KES (2^7)
- Stateless API (doesn't track current period internally)
- Uses `DirectSerialise` for binary format (not JSON TextEnvelope directly)
- Context parameter is unit type `()`
- Evolution via `update_kes()` returns `Option<SigningKey>` (None = expired)
- Key generation from 32-byte seed via `gen_key_kes_from_seed_bytes()`

**API Methods** (from `KesAlgorithm` trait):
```rust
impl KesAlgorithm for CompactSum7Kes {
    type VerificationKey = Vec<u8>;  // 32-byte Blake2b-256 hash
    type SigningKey = CompactSumSigningKey<CompactSum6Kes, Blake2b256>;
    type Signature = CompactSumSignature<CompactSum6Kes, Blake2b256>;
    type Context = ();

    fn total_periods() -> Period { 128 }
    fn gen_key_kes_from_seed_bytes(seed: &[u8]) -> Result<SigningKey, KesMError>;
    fn derive_verification_key(key: &SigningKey) -> Result<VerificationKey, KesMError>;
    fn sign_kes(ctx: &(), period: Period, msg: &[u8], key: &SigningKey)
        -> Result<Signature, KesMError>;
    fn verify_kes(ctx: &(), vk: &VerificationKey, period: Period, msg: &[u8], sig: &Signature)
        -> Result<(), KesMError>;
    fn update_kes(ctx: &(), key: SigningKey, period: Period)
        -> Result<Option<SigningKey>, KesMError>;
    fn forget_signing_key_kes(key: SigningKey);
}
```

**Key API Differences**:

| Our API | CompactSum7Kes API | Migration |
|---------|-------------------|-----------|
| `KesSecretKey::generate(depth)` | `gen_key_kes_from_seed_bytes(seed)` | Generate random seed first |
| `key.current_period()` | ❌ Not tracked | Add wrapper field |
| `key.evolve()` → `Result<Self>` | `update_kes(ctx, key, period)` → `Result<Option<Self>>` | Handle Option |
| `key.sign(period, msg)` | `sign_kes(ctx, period, msg, key)` | Reorder params |
| TextEnvelope JSON format | DirectSerialise binary | Add conversion layer |

## Phase 2: Wrapper Design

**Goal**: Create thin wrapper that maintains existing API while using CompactSum7Kes internally

### File Structure

```
crates/cardano-crypto/src/kes/
├── mod.rs              # Public API (keep existing exports)
├── stub.rs             # Current 713-line implementation (rename for fallback)
├── library.rs          # NEW: CompactSum7Kes wrapper (this file)
└── text_envelope.rs    # NEW: CLI format conversion helpers
```

### Core Wrapper Type

```rust
// File: crates/cardano-crypto/src/kes/library.rs

use cardano_crypto_class::kes::{CompactSum7Kes, KesAlgorithm, KesMError};
use cardano_crypto_class::direct_serialise::{DirectSerialise, DirectDeserialise};

/// Wrapper for CompactSum7Kes that maintains API compatibility.
///
/// This wrapper adds:
/// - Current period tracking (library is stateless)
/// - TextEnvelope JSON file I/O (library uses DirectSerialise)
/// - Simpler API matching our existing code
pub struct KesSecretKey {
    /// The underlying CompactSum7Kes signing key
    inner: <CompactSum7Kes as KesAlgorithm>::SigningKey,
    /// Current period (0-127 for CompactSum7Kes)
    current_period: u64,
    /// Maximum period (always 127 for CompactSum7Kes)
    max_period: u64,
    /// Tree depth (always 7 for CompactSum7Kes)
    depth: u32,
}

impl KesSecretKey {
    const MAX_PERIOD: u64 = 127;
    const DEPTH: u32 = 7;

    /// Generate a new KES key at period 0
    pub fn generate(depth: u32) -> Self {
        assert_eq!(depth, Self::DEPTH,
            "Only depth={} (128 periods) supported by CompactSum7Kes", Self::DEPTH);

        // Generate random 32-byte seed
        let mut seed = [0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut seed);

        // Generate key from seed
        let inner = CompactSum7Kes::gen_key_kes_from_seed_bytes(&seed)
            .expect("CompactSum7Kes key generation failed");

        Self {
            inner,
            current_period: 0,
            max_period: Self::MAX_PERIOD,
            depth: Self::DEPTH,
        }
    }

    /// Load KES key from Cardano CLI TextEnvelope JSON format
    ///
    /// Reads JSON file like:
    /// ```json
    /// {
    ///   "type": "KesSigningKey_ed25519_kes_2^7",
    ///   "description": "KES Signing Key",
    ///   "cborHex": "582..."
    /// }
    /// ```
    pub fn from_file(path: &Path, period: u64) -> Result<Self> {
        use crate::kes::text_envelope::{TextEnvelope, parse_kes_signing_key};

        // Read and parse JSON
        let json = std::fs::read_to_string(path)
            .map_err(|e| CryptoError::IoError(e.to_string()))?;

        let envelope: TextEnvelope = serde_json::from_str(&json)
            .map_err(|e| CryptoError::InvalidKeyFormat(e.to_string()))?;

        // Decode CBOR hex and deserialize using DirectDeserialise
        let (inner, depth) = parse_kes_signing_key(&envelope)?;

        Ok(Self {
            inner,
            current_period: period,
            max_period: Self::MAX_PERIOD,
            depth,
        })
    }

    /// Save KES key to Cardano CLI TextEnvelope JSON format
    pub fn to_file(&self, path: &Path) -> Result<()> {
        use crate::kes::text_envelope::{TextEnvelope, serialize_kes_signing_key};

        let envelope = serialize_kes_signing_key(&self.inner, self.depth)?;
        let json = serde_json::to_string_pretty(&envelope)?;

        std::fs::write(path, json)
            .map_err(|e| CryptoError::IoError(e.to_string()))?;

        Ok(())
    }

    /// Evolve the key to the next period
    pub fn evolve(&self) -> Result<Self> {
        let next_period = self.current_period + 1;

        if next_period > self.max_period {
            return Err(CryptoError::InvalidSignature(
                format!("Cannot evolve past max period {}", self.max_period)
            ));
        }

        // ISSUE: update_kes consumes the signing key
        // Options:
        // 1. Change our API to consume self (BREAKING CHANGE)
        // 2. Clone the key (if SigningKey implements Clone)
        // 3. Use unsafe to create a copy
        //
        // For now, we'll change evolve() to consume self
        Err(CryptoError::InvalidSignature(
            "Use evolve_owned() which consumes self".to_string()
        ))
    }

    /// Evolve the key to the next period (consumes self)
    pub fn evolve_owned(self) -> Result<Self> {
        let next_period = self.current_period + 1;

        if next_period > self.max_period {
            return Err(CryptoError::InvalidSignature(
                format!("Cannot evolve past max period {}", self.max_period)
            ));
        }

        // Call update_kes (consumes inner)
        let new_inner = CompactSum7Kes::update_kes(&(), self.inner, self.current_period)?
            .ok_or_else(|| CryptoError::InvalidSignature("Key expired".to_string()))?;

        Ok(Self {
            inner: new_inner,
            current_period: next_period,
            max_period: self.max_period,
            depth: self.depth,
        })
    }

    /// Evolve to a specific period (consumes self)
    pub fn evolve_to(mut self, target_period: u64) -> Result<Self> {
        if target_period < self.current_period {
            return Err(CryptoError::InvalidSignature(
                "Cannot evolve backwards".to_string()
            ));
        }

        if target_period > self.max_period {
            return Err(CryptoError::InvalidSignature(
                format!("Target period {} exceeds max {}", target_period, self.max_period)
            ));
        }

        // Evolve period by period
        while self.current_period < target_period {
            self = self.evolve_owned()?;
        }

        Ok(self)
    }

    /// Sign a message at the current period
    pub fn sign(&self, period: u64, message: &[u8]) -> Result<KesSignature> {
        // Verify period matches current
        if period != self.current_period {
            return Err(CryptoError::InvalidSignature(
                format!("Period mismatch: key at period {}, requested {}",
                    self.current_period, period)
            ));
        }

        // Sign using library
        let sig = CompactSum7Kes::sign_kes(&(), period, message, &self.inner)
            .map_err(|e| CryptoError::InvalidSignature(format!("Signing failed: {:?}", e)))?;

        // Serialize signature
        let signature_bytes = CompactSum7Kes::raw_serialize_signature_kes(&sig);

        // Wrap in our signature type
        Ok(KesSignature {
            period,
            signature: signature_bytes,
            // Note: CompactSumSignature doesn't have these fields
            // We may need to reconstruct or remove these from our API
            period_vkey: vec![], // TODO: Reconstruct or remove
            auth_path: vec![],   // TODO: Reconstruct or remove
        })
    }

    /// Get the public key
    pub fn to_public(&self) -> KesPublicKey {
        let vkey = CompactSum7Kes::derive_verification_key(&self.inner)
            .expect("Failed to derive verification key");

        KesPublicKey {
            vkey,
            max_period: self.max_period,
        }
    }

    /// Get current period
    pub fn current_period(&self) -> u64 {
        self.current_period
    }

    /// Get maximum period
    pub fn max_period(&self) -> u64 {
        self.max_period
    }

    /// Check if key has expired
    pub fn is_expired(&self) -> bool {
        self.current_period > self.max_period
    }

    /// Convert to raw bytes (for serialization)
    pub fn to_bytes(&self) -> Vec<u8> {
        // Use DirectSerialise to serialize
        let mut bytes = Vec::new();
        let mut push = |ptr: *const u8, len: usize| {
            unsafe {
                let slice = std::slice::from_raw_parts(ptr, len);
                bytes.extend_from_slice(slice);
            }
            Ok(())
        };

        self.inner.direct_serialise(&mut push)
            .expect("DirectSerialise failed");

        bytes
    }
}

impl Drop for KesSecretKey {
    fn drop(&mut self) {
        // CompactSum7Kes uses MLocked memory and zeroizes on drop
        // The library handles secure cleanup, but we can't call
        // forget_signing_key_kes because it consumes the key

        // If we need explicit cleanup, we'd need to use ManuallyDrop
        // and handle it in a custom drop implementation

        // For now, rely on the library's Drop implementation
    }
}
```

### TextEnvelope Conversion Helpers

```rust
// File: crates/cardano-crypto/src/kes/text_envelope.rs

use cardano_crypto_class::kes::{CompactSum7Kes, KesAlgorithm};
use cardano_crypto_class::direct_serialise::{DirectSerialise, DirectDeserialise};
use serde::{Deserialize, Serialize};

/// Cardano CLI TextEnvelope format
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEnvelope {
    #[serde(rename = "type")]
    pub type_: String,
    pub description: String,
    pub cbor_hex: String,
}

/// Parse KES signing key from TextEnvelope
pub fn parse_kes_signing_key(
    envelope: &TextEnvelope
) -> Result<(<CompactSum7Kes as KesAlgorithm>::SigningKey, u32), CryptoError> {
    // Decode CBOR hex
    let cbor_bytes = hex::decode(&envelope.cbor_hex)
        .map_err(|_| CryptoError::InvalidKeyFormat("Invalid hex".to_string()))?;

    // Parse CBOR (format: 0x5820 + 32 bytes, or recursive for larger keys)
    let key_bytes = parse_cbor_bytes(&cbor_bytes)?;

    // Deserialize using DirectDeserialise
    let signing_key = deserialize_signing_key(&key_bytes)?;

    // Extract depth from type string
    let depth = extract_depth(&envelope.type_)?;

    Ok((signing_key, depth))
}

/// Serialize KES signing key to TextEnvelope
pub fn serialize_kes_signing_key(
    key: &<CompactSum7Kes as KesAlgorithm>::SigningKey,
    depth: u32
) -> Result<TextEnvelope, CryptoError> {
    // Serialize using DirectSerialise
    let mut key_bytes = Vec::new();
    let mut push = |ptr: *const u8, len: usize| {
        unsafe {
            let slice = std::slice::from_raw_parts(ptr, len);
            key_bytes.extend_from_slice(slice);
        }
        Ok(())
    };

    key.direct_serialise(&mut push)
        .map_err(|e| CryptoError::SerializationError(format!("{:?}", e)))?;

    // Wrap in CBOR byte string format
    let cbor_bytes = wrap_cbor_bytes(&key_bytes)?;
    let cbor_hex = hex::encode(cbor_bytes);

    Ok(TextEnvelope {
        type_: format!("KesSigningKey_ed25519_kes_2^{}", depth),
        description: "KES Signing Key".to_string(),
        cbor_hex,
    })
}

fn parse_cbor_bytes(cbor: &[u8]) -> Result<Vec<u8>, CryptoError> {
    // CBOR byte string format: 0x58 <length_byte> <bytes>
    // or 0x5820 for 32-byte strings
    if cbor.len() < 2 {
        return Err(CryptoError::InvalidKeyFormat("CBOR too short".to_string()));
    }

    if cbor[0] == 0x58 {
        let len = cbor[1] as usize;
        if cbor.len() < 2 + len {
            return Err(CryptoError::InvalidKeyFormat("CBOR length mismatch".to_string()));
        }
        Ok(cbor[2..2+len].to_vec())
    } else {
        Err(CryptoError::InvalidKeyFormat("Invalid CBOR format".to_string()))
    }
}

fn wrap_cbor_bytes(bytes: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let mut cbor = vec![0x58, bytes.len() as u8];
    cbor.extend_from_slice(bytes);
    Ok(cbor)
}

fn deserialize_signing_key(
    bytes: &[u8]
) -> Result<<CompactSum7Kes as KesAlgorithm>::SigningKey, CryptoError> {
    let mut offset = 0;
    let mut pull = |buf: *mut u8, len: usize| {
        if offset + len > bytes.len() {
            return Err(/* DirectError */);
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr().add(offset),
                buf,
                len
            );
        }
        offset += len;
        Ok(())
    };

    <CompactSum7Kes as KesAlgorithm>::SigningKey::direct_deserialise(&mut pull)
        .map_err(|e| CryptoError::DeserializationError(format!("{:?}", e)))
}

fn extract_depth(type_str: &str) -> Result<u32, CryptoError> {
    // Parse "KesSigningKey_ed25519_kes_2^7" -> 7
    if let Some(caret_pos) = type_str.rfind('^') {
        let depth_str = &type_str[caret_pos+1..];
        depth_str.parse::<u32>()
            .map_err(|_| CryptoError::InvalidKeyFormat("Invalid depth".to_string()))
    } else {
        Err(CryptoError::InvalidKeyFormat("Missing depth".to_string()))
    }
}
```

## Phase 3: Implementation Tasks

### Task 1: Create Wrapper Implementation (8 hours)

**Files to Create**:
- [x] `crates/cardano-crypto/src/kes/library.rs` - Main wrapper
- [ ] `crates/cardano-crypto/src/kes/text_envelope.rs` - CLI format helpers
- [ ] Rename `crates/cardano-crypto/src/kes/mod.rs` current impl to `stub.rs`

**Subtasks**:
1. Implement `KesSecretKey` wrapper struct (2 hours)
2. Implement key generation from seed (1 hour)
3. Implement TextEnvelope file I/O (2 hours)
4. Implement evolution (owned API) (1 hour)
5. Implement signing (1 hour)
6. Implement public key derivation (0.5 hours)
7. Update module exports in `mod.rs` (0.5 hours)

### Task 2: Update BlockForger Integration (4 hours)

**Files to Modify**:
- `crates/cardano-consensus/src/block_production.rs`
- `crates/cardano-consensus/src/block_forging.rs`

**Changes**:
1. Update `KesKey` to use new API (2 hours)
   - Handle owned evolution API
   - Update auto-evolution logic
2. Update operational certificate handling (1 hour)
3. Update block signing calls (1 hour)

### Task 3: Migrate Tests (8 hours)

**Files to Update**:
- `crates/cardano-crypto/src/kes/mod.rs` (tests)
- `crates/cardano-consensus/src/block_production.rs` (tests)
- `tests/consensus/test_block_production_integration.rs`

**Test Updates**:
1. Update key generation tests (1 hour)
2. Update evolution tests for 128 periods (2 hours)
3. Update signing/verification tests (1 hour)
4. Update file I/O tests for TextEnvelope (2 hours)
5. Add compatibility tests (load cardano-cli keys) (2 hours)

### Task 4: Integration Testing (4 hours)

1. **Test with cardano-cli keys** (2 hours)
   - Generate keys with cardano-cli
   - Load into our code
   - Sign blocks
   - Verify with cardano-cli

2. **Test full block production** (1 hour)
   - Run block forging with new KES
   - Verify auto-evolution works
   - Check operational certificate updates

3. **Performance testing** (1 hour)
   - Benchmark key generation
   - Benchmark signing speed
   - Compare to stub implementation

## Phase 4: Migration Strategy

### Option A: Feature Flag (Gradual - RECOMMENDED)

```rust
// In Cargo.toml
[features]
default = ["kes-library"]
kes-stub = []
kes-library = []

// In mod.rs
#[cfg(feature = "kes-library")]
mod library;
#[cfg(feature = "kes-library")]
pub use library::*;

#[cfg(feature = "kes-stub")]
mod stub;
#[cfg(feature = "kes-stub")]
pub use stub::*;
```

**Advantages**:
- Can test new implementation side-by-side
- Easy fallback if issues found
- Gradual rollout possible

**Process**:
1. Week 1: Implement wrapper, test in isolation
2. Week 2: Deploy to testnet with feature flag
3. Week 3: Monitor, fix issues
4. Week 4: Switch default, deprecate stub

### Option B: Direct Replacement (Fast)

Replace stub implementation directly.

**Advantages**:
- Clean, no feature flags
- Forces full migration

**Risks**:
- No fallback if issues
- Must be fully tested first

**Process**:
1. Implement wrapper
2. Comprehensive testing
3. Replace in one PR
4. Deploy to testnet

## Phase 5: Success Criteria

### Functional Requirements

- [ ] Can generate KES keys (128 periods)
- [ ] Can load cardano-cli JSON keys
- [ ] Can save to cardano-cli JSON format
- [ ] Can evolve keys forward
- [ ] Cannot evolve backwards
- [ ] Can sign blocks at any period
- [ ] Signatures verify with public key
- [ ] Expires at period 128
- [ ] Auto-evolution during forging works

### Compatibility Requirements

- [ ] Keys generated by cardano-cli load correctly
- [ ] Keys we generate can be used by cardano-cli
- [ ] Signatures we create verify with cardano-cli
- [ ] Blocks we sign accepted by Haskell node

### Performance Requirements

- [ ] Key generation <100ms
- [ ] Evolution <10ms per period
- [ ] Signing <5ms per block
- [ ] Within 20% of stub performance

### Quality Requirements

- [ ] All unit tests passing
- [ ] All integration tests passing
- [ ] No clippy warnings
- [ ] Documentation complete
- [ ] Examples working

## Timeline

**Week 1** (October 6-12):
- Day 1-2: Implement wrapper types
- Day 3-4: Implement TextEnvelope conversion
- Day 5: Integrate with BlockForger

**Week 2** (October 13-19):
- Day 1-2: Migrate all tests
- Day 3-4: Integration testing
- Day 5: Compatibility testing with cardano-cli

**Week 3** (October 20-26):
- Day 1: Deploy to testnet
- Day 2-5: Monitor, fix issues

**Week 4** (October 27 - November 2):
- Day 1: Final verification
- Day 2: Switch default feature
- Day 3-5: Documentation and cleanup

## Risks and Mitigation

### Risk 1: API Breaking Changes

**Risk**: Owned evolution API breaks existing code
**Probability**: High
**Impact**: Medium

**Mitigation**:
- Use feature flag for gradual migration
- Update all call sites systematically
- Provide deprecation warnings

### Risk 2: Binary Incompatibility

**Risk**: Keys not compatible with cardano-cli
**Probability**: Medium
**Impact**: High

**Mitigation**:
- Extensive compatibility testing
- Cross-verify with cardano-cli
- Test with real mainnet keys

### Risk 3: Performance Regression

**Risk**: Library slower than stub
**Probability**: Low
**Impact**: Low

**Mitigation**:
- Benchmark early
- Profile if needed
- Acceptable tradeoff for correctness

## Next Steps

### Immediate (Today)
1. ✅ Complete API study
2. ✅ Document API differences
3. ✅ Design wrapper architecture
4. [ ] Start implementing `library.rs`

### This Week
5. [ ] Complete wrapper implementation
6. [ ] Implement TextEnvelope helpers
7. [ ] Create feature flag structure
8. [ ] Begin test migration

### Next Week
9. [ ] Complete test migration
10. [ ] Integration testing
11. [ ] Compatibility verification
12. [ ] Deploy to testnet

---

**Status**: Phase 1 Complete, Ready for Phase 2 Implementation
**Next Action**: Begin implementing `crates/cardano-crypto/src/kes/library.rs`
**Blocker**: None
**Questions**: None at this time
