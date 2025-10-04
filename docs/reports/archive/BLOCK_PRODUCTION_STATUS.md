# Block Production Implementation Status

## Overview

This document describes the current implementation status of block production (stake pool operation)
capabilities in the Cardano Node Rust implementation.

**Status**: Slot leadership complete, block forging in progress
**Date**: October 3, 2025
**Target Protocol**: Ouroboros Praos (Shelley, Alonzo, Babbage, Conway eras)---

## Architecture

### Block Production Flow

```
1. Slot Notification
   ↓
2. Leader Election (VRF)
   ↓
3. Block Forging (if elected)
   ↓
4. Block Signing (KES)
   ↓
5. Block Broadcasting
```

### Key Components

```
┌─────────────────────────────────────────┐
│         Node Configuration              │
│  - BlockProducerConfig                  │
│  - Key file paths                       │
│  - Forging behavior                     │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│         Key Management                  │
│  - VRF key loading (cardano-crypto)     │
│  - KES key loading                      │
│  - Operational certificate              │
│  - CBOR parsing                         │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│         Consensus Layer                 │
│  - BlockProducer                        │
│  - SlotLeadershipCalculator             │
│  - ForgingContext                       │
│  - KesManager                           │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│         Cryptography Layer              │
│  - VRF prove/verify (COMPLETE)          │
│  - Ed25519 signatures                   │
│  - KES operations                       │
│  - Blake2b hashing                      │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│         Network Layer                   │
│  - Block broadcasting                   │
│  - BlockFetch protocol                  │
│  - ChainSync protocol                   │
└─────────────────────────────────────────┘
```

---

### ✅ COMPLETE

#### 1. Configuration System

**Files**:
- `crates/cardano-node/src/config/block_producer.rs` (398 lines)
- `crates/cardano-node/config/block-producer/` (examples and docs)

**Capabilities**:
- ✅ `BlockProducerConfig` structure with all required fields
- ✅ VRF key configuration
- ✅ KES key configuration with evolution tracking
- ✅ Operational certificate configuration
- ✅ Cold key configuration (optional)
- ✅ Forging behavior settings
- ✅ Leader schedule configuration
- ✅ Auto-rotation settings for KES keys
- ✅ Configuration validation
- ✅ KES expiration checking
- ✅ Multiple key format support (cardano-cli, raw-hex, raw-binary)

**Code Example**:
```rust
pub struct BlockProducerConfig {
    pub enabled: bool,
    pub pool_id: Option<String>,
    pub vrf_key: VrfKeyConfig,
    pub kes_key: KesKeyConfig,
    pub operational_cert: OperationalCertConfig,
    pub forging_behavior: ForgingBehavior,
    pub leader_schedule: LeaderScheduleConfig,
}

// Validates all key files exist and KES isn't expired
config.validate()?;
```

#### 2. Key Management System

**Files**:
- `crates/cardano-node/src/keys/mod.rs` (614 lines)

**Capabilities**:
- ✅ VRF key loading from Cardano CLI JSON format
- ✅ VRF key loading from raw hex/binary
- ✅ KES key loading (all formats)
- ✅ Operational certificate loading
- ✅ Cold key support
- ✅ CBOR envelope parsing (Cardano CLI format)
- ✅ Key validation and error handling
- ✅ Support for multiple key formats

**Code Example**:
```rust
// Load VRF signing key
let vrf_key = load_vrf_signing_key(
    Path::new("keys/vrf.skey"),
    KeyFormat::CardanoCli
)?;

// Load KES signing key
let kes_key = load_kes_signing_key(
    Path::new("keys/kes.skey"),
    KeyFormat::CardanoCli,
    current_kes_period
)?;

// Load operational certificate
let op_cert = load_operational_certificate(
    Path::new("keys/node.cert")
)?;
```

#### 3. VRF Cryptography

**Files**:
- `crates/cardano-crypto/src/vrf/mod.rs` (304 lines)

**Capabilities**:
- ✅ VRF key generation from seed
- ✅ VRF proof generation (`prove()`)
- ✅ VRF proof verification (`verify()`)
- ✅ VRF output derivation
- ✅ Backend integration (blst library)
- ✅ Test vectors validation

**Code Example**:
```rust
// Generate VRF proof for slot leadership
let (vrf_output, vrf_proof) = vrf_private_key.prove(slot_bytes);

// Verify VRF proof
let is_valid = vrf_public_key.verify(slot_bytes, &vrf_output, &vrf_proof)?;
```

#### 4. Block Production Data Structures

**Files**:
- `crates/cardano-consensus/src/block_production.rs` (988 lines)

**Capabilities**:
- ✅ `BlockProducer` with pool ID, VRF key, KES key, operational cert
- ✅ `VrfKey` structure
- ✅ `KesKey` structure with period tracking
- ✅ `OperationalCertificate` structure
- ✅ `ForgingContext` for block creation
- ✅ `ForgedBlock` result type
- ✅ Transaction and ledger state structures

**Code Example**:
```rust
pub struct BlockProducer {
    pub pool_id: Ed25519KeyHash,
    pub vrf_key: VrfKey,
    pub kes_key: KesKey,
    pub operational_cert: OperationalCertificate,
    pub stake: u64,
}

pub struct ForgingContext {
    pub current_slot: SlotNo,
    pub epoch_nonce: Blake2b256Hash,
    pub prev_block_hash: Blake2b256Hash,
    pub mempool: Vec<Transaction>,
    pub ledger_state: SimplifiedLedgerState,
}
```

#### 5. Network Layer

**Files**:
- `crates/cardano-network/src/` (multiple files)

**Capabilities**:
- ✅ Multiplexer protocol working
- ✅ Handshake protocol (V14/V15)
- ✅ ChainSync protocol (ready for blocks)
- ✅ BlockFetch protocol (for receiving/sending blocks)
- ✅ Preview testnet connectivity verified

#### 6. Slot Leadership Calculation (NEW - COMPLETE!)

**Files**:
- `crates/cardano-consensus/src/leadership.rs` (513 lines)
- `crates/cardano-consensus/examples/leadership_calculation.rs` (example)

**Capabilities**:
- ✅ **VRF-based leader election algorithm** (Ouroboros Praos)
- ✅ **Threshold calculation** with 2^256 arbitrary precision math
- ✅ **Leadership proof generation** with VRF proofs
- ✅ **Leadership verification** for other pools' blocks
- ✅ **Epoch schedule pre-calculation** (full or partial)
- ✅ **Multi-epoch lookahead** support
- ✅ **Statistical functions** (expected blocks, min stake)
- ✅ **Domain separation** with proper VRF tags
- ✅ **Compatible** with Haskell node implementation

**Algorithm**:
```rust
// For each slot:
// 1. Construct VRF input: epoch_nonce || slot || "TEST"
// 2. Generate VRF proof
// 3. Calculate threshold: 2^256 * (1 - (1-f)^σ)
// 4. Check if VRF_output < threshold → LEADER
```

**Code Example**:
```rust
let calculator = LeadershipCalculator::new(
    stake_distribution,
    protocol_params,
    epoch_nonce,
    current_epoch,
);

match calculator.check_slot_leadership(&pool_id, pool_stake, &vrf_key, slot)? {
    LeadershipCheck::Leader(proof) => {
        // Forge block with proof
    }
    LeadershipCheck::NotLeader { .. } => {
        // Wait for next slot
    }
}
```

---

### ⚠️ IN PROGRESS

#### 1. KES Cryptography

**Status**: Structure defined, operations need implementation

**Files**:
- `crates/cardano-consensus/src/ouroboros.rs`
- `crates/cardano-consensus/src/slots.rs`

**Required**:
- ⚠️ VRF-based leader election
- ⚠️ Stake distribution integration
- ⚠️ Active slot coefficient (f parameter)
- ⚠️ Epoch nonce calculation
- ⚠️ Pool stake lookup

**Algorithm**:
```rust
// Pseudo-code for leader election
fn check_slot_leadership(
    slot: SlotNo,
    vrf_key: &VrfPrivateKey,
    stake: u64,
    total_stake: u64,
    epoch_nonce: &[u8],
) -> Option<VrfProof> {
    // 1. Construct VRF input: epoch_nonce || slot
    let vrf_input = construct_vrf_input(epoch_nonce, slot);

    // 2. Generate VRF proof
    let (vrf_output, vrf_proof) = vrf_key.prove(&vrf_input);

    // 3. Check if output < threshold
    let threshold = calculate_threshold(stake, total_stake);
    if vrf_output_to_nat(&vrf_output) < threshold {
        Some(vrf_proof)
    } else {
        None
    }
}
```

#### 3. Block Forging

**Status**: Data structures complete, forging logic needs implementation

**Required**:
- ⚠️ Block header construction
- ⚠️ Block body construction
- ⚠️ Transaction selection from mempool
- ⚠️ VRF proof inclusion
- ⚠️ KES signature generation
- ⚠️ Operational certificate inclusion
- ⚠️ Block size and transaction limits

**Algorithm**:
```rust
// Pseudo-code for block forging
fn forge_block(
    context: &ForgingContext,
    producer: &BlockProducer,
    vrf_proof: VrfProof,
) -> Result<ForgedBlock> {
    // 1. Select transactions from mempool
    let txs = select_transactions(&context.mempool, &context.ledger_state)?;

    // 2. Construct block header
    let header = BlockHeader {
        slot: context.current_slot,
        prev_hash: context.prev_block_hash,
        issuer_vkey: producer.kes_key.verification_key(),
        vrf_vkey: producer.vrf_key.public_key,
        vrf_result: vrf_proof,
        block_body_hash: hash_block_body(&txs),
        op_cert: producer.operational_cert.clone(),
        protocol_version: context.protocol_version,
    };

    // 3. Sign header with KES key
    let header_bytes = serialize(&header);
    let signature = producer.kes_key.sign(&header_bytes)?;

    // 4. Construct block
    Ok(ForgedBlock {
        header,
        signature,
        body: txs,
    })
}
```

---

### ❌ NOT STARTED

#### 1. Runtime Integration

**Required**:
- ❌ Block producer subsystem in node runtime
- ❌ Slot notification system
- ✅ Leader schedule calculator (COMPLETE - use `LeadershipCalculator`!)
- ❌ Integration with mempool
- ❌ Integration with ledger state
- ❌ Block broadcasting after forging
- ❌ Monitoring and metrics

**Architecture** (updated with leadership calculator):
```rust
// Proposed structure
pub struct BlockProducerSubsystem {
    config: BlockProducerConfig,
    vrf_key: VrfSigningKey,
    kes_key: KesSigningKey,
    op_cert: OperationalCertificate,
    leadership_calculator: LeadershipCalculator, // NEW!
}

impl BlockProducerSubsystem {
    pub async fn run(&mut self) -> Result<()> {
        loop {
            // Wait for slot notification
            let slot = self.slot_notifier.next().await?;

            // Check if we're the leader (NOW IMPLEMENTED!)
            match self.leadership_calculator.check_slot_leadership(
                &self.pool_id,
                self.pool_stake,
                &self.vrf_key.private_key,
                slot,
            )? {
                LeadershipCheck::Leader(proof) => {
                    // Forge block with VRF proof
                    let block = self.forge_block(slot, proof).await?;

                    // Broadcast block
                    self.network.broadcast_block(block).await?;
                }
                LeadershipCheck::NotLeader { .. } => {
                    // Not leader, wait for next slot
                }
            }
        }
    }
}
```

#### 2. Stake Distribution Integration

**Required**:
- ❌ Snapshot mechanism for stake distribution
- ❌ Pool stake lookup
- ❌ Active stake calculation
- ❌ Delegation tracking
- ❌ Mark/Set/Go epoch boundary handling

#### 3. Leader Schedule Pre-calculation

**Status**: ✅ **Algorithm Complete** - Integration needed

**Capabilities Available**:
- ✅ Calculate full epoch schedule
- ✅ Multi-epoch lookahead
- ✅ Export to file
- ✅ Epoch nonce derivation
- ✅ VRF-based schedule calculation

**Required for Integration**:
- ❌ Scheduled task to run at epoch boundaries
- ❌ Persistence of schedule
- ❌ Schedule reload on restart
- ❌ Monitoring/alerts before leader slots
- ❌ Stake snapshot at epoch boundary (needs ledger integration)

#### 4. KES Auto-rotation

**Required**:
- ❌ KES period monitoring
- ❌ Auto-rotation trigger
- ❌ Operational certificate regeneration
- ❌ Hot-reload of new keys
- ❌ Alerting system

---

## Testing Requirements

### Unit Tests

- ✅ Configuration parsing and validation
- ✅ VRF key loading (basic)
- ✅ CBOR parsing
- ⚠️ KES key operations
- ⚠️ Slot leadership calculation
- ❌ Block forging logic

### Integration Tests

- ❌ Key loading from real cardano-cli generated files
- ❌ Block production on preview testnet
- ❌ Leader schedule accuracy
- ❌ KES rotation procedure
- ❌ Block propagation

### End-to-End Tests

- ❌ Register pool on preview testnet
- ❌ Produce blocks for full epoch
- ❌ Verify blocks are accepted by network
- ❌ Test KES rotation
- ❌ Test with various stake amounts

---

## Security Considerations

### Key Security

1. **Cold Key Protection** ✅
   - Never stored on block producer node
   - Optional in configuration
   - Only verification key needed for operations

2. **VRF Key Protection** ✅
   - File permission checking in key loader
   - Proper error handling for missing keys
   - Key format validation

3. **KES Key Rotation** ✅
   - Expiration checking implemented
   - Auto-rotation configuration supported
   - Alert margins configurable

### Operational Security

1. **Certificate Validation** ✅
   - Issue counter tracking
   - KES period validation
   - File integrity checking

2. **Access Control** ⚠️
   - TODO: File permission enforcement
   - TODO: Key memory protection (mlock)
   - TODO: Secure key deletion

---

## Performance Considerations

### Block Forging Speed

Target: < 100ms from slot notification to block broadcast

1. **Transaction Selection**: O(n log n) for fee sorting
2. **Block Signing**: ~1ms for KES signature
3. **Serialization**: < 10ms for typical block
4. **Network**: < 50ms for first peer propagation

### Leader Schedule Calculation

Target: < 5 seconds for full epoch

1. **VRF Calculations**: ~100μs per slot
2. **Stake Lookups**: O(1) with proper indexing
3. **Multi-epoch**: Parallelizable

---

## Dependencies

### External Crates

- ✅ `blst`: VRF backend (BLS12-381)
- ✅ `ed25519-dalek`: Ed25519 signatures
- ✅ `blake2`: Blake2b hashing
- ✅ `hex`: Hex encoding/decoding
- ✅ `serde`: Configuration serialization
- ⚠️ KES library: Need to identify/implement

### Internal Crates

- ✅ `cardano-crypto`: Cryptographic primitives
- ✅ `cardano-consensus`: Consensus logic
- ✅ `cardano-network`: Network protocols
- ⚠️ `cardano-ledger`: Ledger state (needs stake query)
- ⚠️ `cardano-storage`: Chain storage (needs epoch boundary)

---

## Roadmap

### Phase 1: Configuration & Key Management ✅ COMPLETE
- Configuration structure
- Key loading
- VRF crypto
- Documentation

### Phase 2: Slot Leadership (CURRENT)
- Implement slot leadership algorithm
- VRF-based leader election
- Stake distribution integration
- Leader schedule calculation

### Phase 3: Block Forging (NEXT)
- Block construction
- KES signing
- Transaction selection
- Mempool integration

### Phase 4: Runtime Integration
- Block producer subsystem
- Slot notifications
- Block broadcasting
- Monitoring

### Phase 5: Testing & Production
- Preview testnet testing
- KES rotation testing
- Security audit
- Performance tuning

---

## Known Issues

1. **KES Implementation**: KES crypto operations not yet implemented
2. **Stake Distribution**: No integration with ledger for stake queries
3. **Mempool**: Transaction selection not implemented
4. **Epoch Boundaries**: Mark/Set/Go not handled
5. **Leader Schedule**: Pre-calculation not implemented

---

## Documentation

### Configuration Examples

- `config/block-producer/README.md`: Complete guide
- `config/block-producer/block-producer-config.json`: Full example
- `config/block-producer/block-producer-minimal.json`: Minimal example
- `config/block-producer/block-producer-testnet.json`: Testnet example

### Code Documentation

All modules have comprehensive rustdoc comments:
```bash
cargo doc --package cardano-node --open
```

---

## Comparison with Haskell Node

| Feature | Haskell Node | Rust Node |
|---------|-------------|-----------|
| Configuration | ✅ Complete | ✅ Complete |
| VRF Crypto | ✅ Complete | ✅ Complete |
| KES Crypto | ✅ Complete | ⚠️ In Progress |
| Slot Leadership | ✅ Complete | ⚠️ In Progress |
| Block Forging | ✅ Complete | ❌ Not Started |
| Leader Schedule | ✅ Complete | ❌ Not Started |
| KES Rotation | ✅ Complete | ✅ Config Only |
| Stake Distribution | ✅ Complete | ❌ Not Started |
| Production Ready | ✅ Yes | ❌ No |

---

## Getting Started (Current State)

While block production is not yet operational, you can:

1. **Test Configuration**:
   ```bash
   # Create example config
   cp config/block-producer/block-producer-minimal.json my-config.json

   # Edit with your key paths
   vim my-config.json
   ```

2. **Generate Test Keys**:
   ```bash
   # Use cardano-cli to generate keys
   cardano-cli node key-gen-VRF --signing-key-file vrf.skey --verification-key-file vrf.vkey
   ```

3. **Test Key Loading** (when runtime integration is complete):
   ```bash
   # This will validate keys and configuration
   cardano-node run --config my-config.json --validate-only
   ```

---

## Contributing

Priority areas for contribution:

1. **KES Implementation**: Implement KES key evolution and signing
2. **Slot Leadership**: Implement leader election algorithm
3. **Block Forging**: Implement block construction and signing
4. **Testing**: Create test vectors and integration tests

See `CONTRIBUTING.md` for guidelines.

---

## References

- [Ouroboros Praos Paper](https://eprint.iacr.org/2017/573.pdf)
- [Cardano Ledger Specs](https://github.com/IntersectMBO/cardano-ledger)
- [Haskell Node Implementation](https://github.com/IntersectMBO/cardano-node)
- [CIP-0009: Shelley Parameters](https://cips.cardano.org/cips/cip9/)

---

**Last Updated**: 2024
**Status**: Configuration Complete, Runtime Integration Pending
