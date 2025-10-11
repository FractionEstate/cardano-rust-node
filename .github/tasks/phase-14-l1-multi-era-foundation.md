# L1 Roadmap - Task 14: Multi-Era Support Foundation

**Task 14:** Implement Multi-Era Block and Transaction Support

- **Status**: Not Started
- **Files**:
  - Create: `crates/cardano-ledger/src/eras/mod.rs`
  - Create: `crates/cardano-ledger/src/eras/byron.rs` (~400 lines)
  - Create: `crates/cardano-ledger/src/eras/shelley.rs` (~500 lines)
  - Create: `crates/cardano-ledger/src/eras/allegra.rs` (~300 lines)
  - Create: `crates/cardano-ledger/src/eras/mary.rs` (~350 lines)
  - Modify: `crates/cardano-ledger/src/lib.rs`
  - Create: `tests/ledger/era_tests.rs`
- **Description**: Implement multi-era support for Cardano blockchain, including Byron, Shelley, Allegra, and Mary eras with era-specific validation rules and transitions.

## Task Checklist

### Era Detection and Switching

- [ ] Define Era enum (Byron, Shelley, Allegra, Mary, Alonzo, Babbage, Conway)
- [ ] Implement era detection from block header
- [ ] Create era transition logic
- [ ] Add era-specific protocol parameters
- [ ] Implement hard fork combinator pattern
- [ ] Track current era in ledger state

### Byron Era Support

- [ ] Implement Byron block structure
- [ ] Add Byron transaction format
- [ ] Create Byron CBOR encoding/decoding
- [ ] Implement Byron address format
- [ ] Add Byron-specific validation rules
- [ ] Support Byron update proposals

### Shelley Era Support

- [ ] Implement Shelley block structure
- [ ] Add Shelley transaction format
- [ ] Create Shelley CBOR encoding/decoding
- [ ] Implement Shelley address format
- [ ] Add stake pool registration certificates
- [ ] Support delegation certificates
- [ ] Implement reward withdrawals

### Allegra Era Support (Timelock Scripts)

- [ ] Extend Shelley with timelock features
- [ ] Implement native script validation
- [ ] Add script hash addresses
- [ ] Support multi-signature scripts
- [ ] Implement time validity intervals
- [ ] Add script CBOR encoding

### Mary Era Support (Multi-Asset)

- [ ] Extend Allegra with multi-asset
- [ ] Implement AssetId and PolicyId types
- [ ] Add minting/burning policies
- [ ] Support token bundles in outputs
- [ ] Implement multi-asset CBOR encoding
- [ ] Add multi-asset validation rules

### Era Transitions

- [ ] Implement Byron→Shelley transition
- [ ] Implement Shelley→Allegra transition
- [ ] Implement Allegra→Mary transition
- [ ] Add transition validation
- [ ] Create transition tests
- [ ] Handle protocol version updates

### Testing

- [ ] Test era detection
- [ ] Test Byron block validation
- [ ] Test Shelley features
- [ ] Test Allegra timelocks
- [ ] Test Mary multi-asset
- [ ] Test era transitions
- [ ] Test backward compatibility
- [ ] Validate against Haskell node

## Implementation Overview

```rust
// crates/cardano-ledger/src/eras/mod.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Era {
    Byron,
    Shelley,
    Allegra,
    Mary,
    Alonzo,
    Babbage,
    Conway,
}

impl Era {
    pub fn detect_from_block(block: &[u8]) -> Result<Self> {
        // Detect era from block header CBOR structure
    }

    pub fn protocol_params(&self) -> ProtocolParams {
        // Return era-specific protocol parameters
    }
}

// crates/cardano-ledger/src/eras/byron.rs

pub struct ByronBlock {
    pub header: ByronBlockHeader,
    pub body: ByronBlockBody,
}

pub struct ByronTransaction {
    pub inputs: Vec<TxIn>,
    pub outputs: Vec<TxOut>,
    pub attributes: TxAttributes,
}

impl ByronBlock {
    pub fn validate(&self, state: &LedgerState) -> Result<()> {
        // Byron-specific validation
    }
}

// crates/cardano-ledger/src/eras/shelley.rs

pub struct ShelleyBlock {
    pub header: ShelleyBlockHeader,
    pub body: ShelleyBlockBody,
}

pub struct ShelleyTransaction {
    pub inputs: Vec<TxIn>,
    pub outputs: Vec<TxOut>,
    pub certificates: Vec<Certificate>,
    pub withdrawals: HashMap<RewardAccount, Coin>,
    pub fee: Coin,
    pub ttl: Option<SlotNo>,
}

#[derive(Debug, Clone)]
pub enum Certificate {
    StakeRegistration(StakeCredential),
    StakeDeregistration(StakeCredential),
    StakeDelegation(StakeCredential, PoolId),
    PoolRegistration(PoolParams),
    PoolRetirement(PoolId, EpochNo),
}
```

## Success Criteria

- [ ] Era detection works for all eras
- [ ] Byron blocks validate correctly
- [ ] Shelley features (staking, delegation) work
- [ ] Allegra timelocks validate properly
- [ ] Mary multi-asset transactions work
- [ ] Era transitions execute correctly
- [ ] All tests pass (15+ tests)
- [ ] Alignment with Haskell node verified
- [ ] Documentation complete

## Dependencies

- cardano-base-rust for crypto primitives
- External: minicbor for CBOR encoding

## Estimated Effort

- Era framework: 5-6 hours
- Byron era: 8-10 hours
- Shelley era: 12-15 hours
- Allegra era: 6-8 hours
- Mary era: 8-10 hours
- Era transitions: 5-6 hours
- Testing and validation: 10-12 hours
- **Total: 54-67 hours**

## Future Enhancements

- [ ] Alonzo era (Plutus V1)
- [ ] Babbage era (Plutus V2, reference inputs)
- [ ] Conway era (governance)
