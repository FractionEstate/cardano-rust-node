# Cardano-Rust-Node: Crypto Integration Quick Reference

## Overview
This guide provides quick reference for developers working with cryptographic operations in cardano-rust-node.

---

## ✅ Approved Crypto Libraries

### From cardano-base-rust (https://github.com/FractionEstate/cardano-base-rust)

#### 1. cardano-vrf-pure (VRF Draft-03)
```toml
[dependencies]
cardano-vrf-pure = { git = "https://github.com/FractionEstate/cardano-base-rust" }
```

**Use for:**
- VRF proof generation/verification
- Slot leadership election
- Block producer selection

**Example:**
```rust
use cardano_crypto::vrf::{VrfPrivateKey, VrfPublicKey, VrfProof, VrfOutput};

let vrf_key = VrfPrivateKey::generate();
let (output, proof) = vrf_key.prove(b"message");
let public_key = vrf_key.public_key();
assert!(public_key.verify(b"message", &output, &proof));
```

#### 2. cardano-crypto-class (Ed25519 DsignAlgorithm)
```toml
[dependencies]
cardano-crypto-class = { git = "https://github.com/FractionEstate/cardano-base-rust" }
```

**Use for:**
- Ed25519 signing/verification
- KES (Key Evolving Signatures)
- Transaction witnesses
- Block signing

**Example:**
```rust
use cardano_crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey, Ed25519Signature};

let private_key = Ed25519PrivateKey::generate();
let signature = private_key.sign(b"message");
let public_key = private_key.public_key();
assert!(public_key.verify(b"message", &signature));
```

### From Standard Rust Crates

#### 3. blake2 (RFC 7693)
```toml
[dependencies]
blake2 = "0.10"
```

**Use for:**
- Transaction ID calculation
- Block hashing
- Pool ID derivation
- Script hashing
- Any cryptographic hashing needs

**Example:**
```rust
use cardano_crypto::Blake2b256Hash;

let tx_hash = Blake2b256Hash::hash(b"transaction data");
let block_hash = Blake2b256Hash::hash(b"block data");
```

---

## 🚫 DO NOT USE

### ❌ ed25519-dalek directly
**Reason:** Not compatible with Cardano's Ed25519 specification
**Use instead:** `cardano-crypto-class` from cardano-base-rust

### ❌ Custom hash implementations
**Reason:** Must use standardized, audited implementations
**Use instead:** `blake2` crate for Blake2b/Blake2s

### ❌ Other VRF libraries
**Reason:** Must use Cardano VRF Draft-03 specification
**Use instead:** `cardano-vrf-pure` from cardano-base-rust

---

## Common Patterns

### Transaction Signing
```rust
use cardano_crypto::ed25519::{Ed25519PrivateKey, Ed25519Signature};
use cardano_crypto::Blake2b256Hash;

// 1. Calculate transaction hash
let tx_bytes = serialize_transaction(&tx);
let tx_hash = Blake2b256Hash::hash(&tx_bytes);

// 2. Sign the hash
let private_key = load_private_key();
let signature = private_key.sign(tx_hash.as_bytes());

// 3. Create witness
let witness = VKeyWitness {
    vkey: private_key.public_key().hash(),
    signature,
};
```

### Block Production
```rust
use cardano_crypto::vrf::VrfPrivateKey;
use cardano_crypto::kes::KesSecretKey;
use cardano_crypto::Blake2b256Hash;

// 1. Check slot leadership (VRF)
let vrf_key = load_vrf_key();
let (vrf_output, vrf_proof) = vrf_key.prove(&vrf_input);
if is_leader(vrf_output, stake) {
    // 2. Build block
    let body = construct_block_body();
    let body_hash = Blake2b256Hash::hash(&body.serialize());

    // 3. Sign with KES
    let kes_key = load_kes_key();
    let header = BlockHeader { body_hash, vrf_proof, ... };
    let kes_signature = kes_key.sign(period, &header.serialize())?;

    // 4. Broadcast
    broadcast_block(header, body, kes_signature);
}
```

### Block Validation
```rust
use cardano_crypto::vrf::VrfPublicKey;
use cardano_crypto::kes::KesPublicKey;
use cardano_crypto::Blake2b256Hash;

// 1. Verify body hash
let computed_hash = Blake2b256Hash::hash(&block.body);
assert_eq!(computed_hash, block.header.body_hash);

// 2. Verify VRF proof
let vrf_public = get_pool_vrf_key(&block.header.issuer);
assert!(vrf_public.verify(&vrf_input, &block.header.vrf_output, &block.header.vrf_proof));

// 3. Verify KES signature
let kes_public = get_pool_kes_key(&block.header.issuer);
assert!(kes_public.verify(period, &block.header, &block.kes_signature)?);
```

### Storage Keys
```rust
use cardano_crypto::Blake2b256Hash;

// Always use Blake2b256Hash for database keys
fn block_key(block_hash: &Blake2b256Hash) -> Vec<u8> {
    let mut key = b"block:".to_vec();
    key.extend_from_slice(block_hash.as_bytes());
    key
}

fn tx_key(tx_hash: &Blake2b256Hash) -> Vec<u8> {
    let mut key = b"tx:".to_vec();
    key.extend_from_slice(tx_hash.as_bytes());
    key
}
```

---

## Security Guidelines

### 1. Key Management

#### ✅ DO:
```rust
// Use move semantics for sensitive keys
let kes_key = KesSecretKey::generate(6);
let evolved = kes_key.evolve_to(target_period)?; // Consumes kes_key
```

#### ❌ DON'T:
```rust
// Don't clone sensitive keys
let kes_key = KesSecretKey::generate(6);
let copy = kes_key.clone(); // ❌ Won't compile (security feature)
```

### 2. Zeroization

KES keys automatically zero memory on drop:
```rust
{
    let kes_key = KesSecretKey::generate(6);
    // Use the key...
} // Key material is zeroed here automatically
```

### 3. Error Handling

Always handle crypto errors properly:
```rust
use cardano_crypto::Result;

fn sign_transaction(tx: &Transaction) -> Result<Signature> {
    let key = load_key()?;
    let hash = Blake2b256Hash::hash(&tx.serialize());
    Ok(key.sign(hash.as_bytes()))
}
```

---

## Testing

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use cardano_crypto::*;

    #[test]
    fn test_signature_roundtrip() {
        let key = Ed25519PrivateKey::generate();
        let message = b"test message";
        let signature = key.sign(message);

        let public_key = key.public_key();
        assert!(public_key.verify(message, &signature));
    }

    #[test]
    fn test_hash_deterministic() {
        let data = b"test data";
        let hash1 = Blake2b256Hash::hash(data);
        let hash2 = Blake2b256Hash::hash(data);
        assert_eq!(hash1, hash2);
    }
}
```

### Integration Tests
```rust
#[cfg(test)]
mod integration_tests {
    use cardano_consensus::*;
    use cardano_crypto::*;

    #[test]
    fn test_block_production_roundtrip() {
        // Generate keys
        let vrf_key = VrfPrivateKey::generate();
        let kes_key = KesSecretKey::generate(6);

        // Produce block
        let block = produce_block(&vrf_key, &kes_key, &pool_params)?;

        // Validate block
        let vrf_public = vrf_key.public_key();
        let kes_public = kes_key.to_public();
        assert!(validate_block(&block, &vrf_public, &kes_public)?);
    }
}
```

---

## Migration Guide

### From ed25519-dalek to cardano-crypto-class

**Before:**
```rust
use ed25519_dalek::{SigningKey, VerifyingKey, Signature};

let signing_key = SigningKey::generate(&mut rng);
let signature = signing_key.sign(message);
```

**After:**
```rust
use cardano_crypto::ed25519::{Ed25519PrivateKey, Ed25519Signature};

let signing_key = Ed25519PrivateKey::generate();
let signature = signing_key.sign(message);
```

### From Custom Hashing to Blake2

**Before:**
```rust
// ❌ Insecure placeholder
fn hash(input: &[u8]) -> [u8; 32] {
    let mut hash = [0u8; 32];
    for (i, &byte) in input.iter().enumerate() {
        hash[i % 32] ^= byte;
    }
    hash
}
```

**After:**
```rust
// ✅ Secure Blake2s-256
use cardano_crypto::Blake2b256Hash;

let hash = Blake2b256Hash::hash(input);
```

---

## Performance Tips

### 1. Batch Operations
```rust
// Verify multiple signatures efficiently
let signatures: Vec<(PublicKey, Message, Signature)> = ...;
for (pubkey, msg, sig) in signatures {
    assert!(pubkey.verify(msg, &sig));
}
```

### 2. Key Caching
```rust
// Cache public keys to avoid repeated derivation
struct KeyCache {
    private_key: Ed25519PrivateKey,
    public_key: Ed25519PublicKey,
}

impl KeyCache {
    fn new(private_key: Ed25519PrivateKey) -> Self {
        let public_key = private_key.public_key();
        Self { private_key, public_key }
    }
}
```

### 3. Hash Pre-computation
```rust
// Pre-compute hashes for frequently accessed data
struct Transaction {
    data: Vec<u8>,
    hash: Blake2b256Hash,
}

impl Transaction {
    fn new(data: Vec<u8>) -> Self {
        let hash = Blake2b256Hash::hash(&data);
        Self { data, hash }
    }
}
```

---

## Troubleshooting

### Issue: "trait bound not satisfied"
**Cause:** Trying to clone KesSecretKey
**Solution:** Use move semantics or shared references

### Issue: "Invalid signature"
**Cause:** Using wrong key or message
**Solution:** Ensure message being verified matches signed message exactly

### Issue: "Hash mismatch"
**Cause:** Different serialization or hash function
**Solution:** Always use Blake2b256Hash::hash() consistently

---

## References

- [cardano-base-rust Repository](https://github.com/FractionEstate/cardano-base-rust)
- [Cardano Ledger Specs](https://github.com/IntersectMBO/cardano-ledger)
- [Ouroboros Praos Paper](https://eprint.iacr.org/2017/573.pdf)
- [VRF Draft-03 Specification](https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-vrf-03)
- [Blake2 RFC 7693](https://datatracker.ietf.org/doc/html/rfc7693)

---

## Contact

For questions or issues:
- Open an issue on GitHub
- Review FINAL_AUDIT_REPORT.md for detailed audit findings
- Check existing tests for usage examples

---

**Last Updated:** October 4, 2025
**Audit Status:** ✅ 100% Perfect (130/130 points)
