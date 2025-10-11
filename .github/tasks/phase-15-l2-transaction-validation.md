# L2 Roadmap - Task 15: Transaction Validation

**Task 15:** Comprehensive Transaction Validation and UTxO Management

- **Status**: Not Started
- **Files**:
  - Create: `crates/cardano-ledger/src/tx_validator.rs` (~600 lines)
  - Create: `crates/cardano-ledger/src/utxo_manager.rs` (~400 lines)
  - Create: `crates/cardano-ledger/src/fees.rs` (~300 lines)
  - Create: `tests/ledger/tx_validation_tests.rs`
- **Description**: Implement full transaction validation including input resolution, balance checking, fee validation, certificate processing, and UTxO state management.

## Task Checklist

### Input Resolution and Validation

- [ ] Implement UTxO lookup from state
- [ ] Validate all inputs exist
- [ ] Check inputs are unspent
- [ ] Verify input script witnesses
- [ ] Validate input amounts
- [ ] Implement double-spend detection

### Balance and Fee Validation

- [ ] Calculate total input value
- [ ] Calculate total output value
- [ ] Implement multi-asset balance checking
- [ ] Validate fee sufficiency
- [ ] Implement minimum fee calculation
- [ ] Check for value overflow

### Output Validation

- [ ] Validate output addresses
- [ ] Check minimum UTxO value (ada min)
- [ ] Validate multi-asset bundles
- [ ] Check output size limits
- [ ] Validate datum attachments
- [ ] Verify script references

### Certificate Processing

- [ ] Validate stake registration
- [ ] Process stake deregistration
- [ ] Handle delegation certificates
- [ ] Validate pool registration
- [ ] Process pool retirement
- [ ] Update stake distribution

### Withdrawal Validation

- [ ] Validate reward account exists
- [ ] Check withdrawal amount vs available
- [ ] Update reward balances
- [ ] Process withdrawal witnesses
- [ ] Handle partial withdrawals

### Minting/Burning Validation

- [ ] Validate minting policies
- [ ] Check policy witnesses
- [ ] Process token minting
- [ ] Process token burning
- [ ] Update asset supply tracking
- [ ] Validate policy timelock

### UTxO State Management

- [ ] Efficient UTxO storage (HashMap)
- [ ] Add UTxOs from outputs
- [ ] Remove UTxOs from inputs
- [ ] Track UTxO set size
- [ ] Implement UTxO snapshots
- [ ] Add UTxO pruning

### Testing

- [ ] Test valid transactions
- [ ] Test insufficient fees
- [ ] Test negative balances
- [ ] Test missing inputs
- [ ] Test double-spends
- [ ] Test certificate processing
- [ ] Test minting/burning
- [ ] Validate against test vectors

## Implementation Overview

```rust
// crates/cardano-ledger/src/tx_validator.rs

pub struct TransactionValidator {
    protocol_params: ProtocolParams,
}

impl TransactionValidator {
    pub fn validate_transaction(
        &self,
        tx: &Transaction,
        utxo_set: &UtxoSet,
        era: Era,
    ) -> Result<ValidationResult> {
        // 1. Resolve inputs
        let resolved_inputs = self.resolve_inputs(tx, utxo_set)?;

        // 2. Validate balance
        self.validate_balance(tx, &resolved_inputs)?;

        // 3. Validate fee
        self.validate_fee(tx)?;

        // 4. Validate outputs
        self.validate_outputs(tx)?;

        // 5. Validate certificates
        self.validate_certificates(tx)?;

        // 6. Validate withdrawals
        self.validate_withdrawals(tx, utxo_set)?;

        // 7. Validate minting/burning
        self.validate_minting(tx)?;

        // 8. Validate scripts
        self.validate_scripts(tx, &resolved_inputs)?;

        Ok(ValidationResult::Valid)
    }

    fn validate_balance(
        &self,
        tx: &Transaction,
        inputs: &[ResolvedInput],
    ) -> Result<()> {
        let input_value = self.sum_inputs(inputs)?;
        let output_value = self.sum_outputs(&tx.outputs)?;
        let fee = tx.fee;
        let withdrawals = self.sum_withdrawals(&tx.withdrawals)?;
        let minted = self.sum_minted(&tx.mint)?;

        // Input + Withdrawals + Minted = Output + Fee
        if input_value + withdrawals + minted != output_value + fee {
            return Err(ValidationError::ImbalancedTransaction);
        }

        Ok(())
    }

    fn validate_fee(&self, tx: &Transaction) -> Result<()> {
        let min_fee = self.calculate_minimum_fee(tx)?;

        if tx.fee < min_fee {
            return Err(ValidationError::InsufficientFee {
                actual: tx.fee,
                minimum: min_fee,
            });
        }

        Ok(())
    }

    fn calculate_minimum_fee(&self, tx: &Transaction) -> Result<Coin> {
        // min_fee = a + b * tx_size
        let size = tx.serialized_size()?;
        let a = self.protocol_params.min_fee_a;
        let b = self.protocol_params.min_fee_b;

        Ok(Coin(a + b * size as u64))
    }
}

// crates/cardano-ledger/src/utxo_manager.rs

pub struct UtxoSet {
    utxos: HashMap<TxIn, TxOut>,
    total_ada: Coin,
}

impl UtxoSet {
    pub fn apply_transaction(&mut self, tx: &Transaction) -> Result<()> {
        // Remove spent inputs
        for input in &tx.inputs {
            let output = self.utxos.remove(input)
                .ok_or(UtxoError::InputNotFound)?;
            self.total_ada -= output.value.coin();
        }

        // Add new outputs
        for (index, output) in tx.outputs.iter().enumerate() {
            let tx_in = TxIn {
                tx_id: tx.id(),
                index: index as u32,
            };
            self.utxos.insert(tx_in, output.clone());
            self.total_ada += output.value.coin();
        }

        Ok(())
    }

    pub fn lookup(&self, tx_in: &TxIn) -> Option<&TxOut> {
        self.utxos.get(tx_in)
    }

    pub fn size(&self) -> usize {
        self.utxos.len()
    }
}

// crates/cardano-ledger/src/fees.rs

pub struct FeeCalculator {
    params: ProtocolParams,
}

impl FeeCalculator {
    pub fn min_fee(&self, tx: &Transaction) -> Coin {
        let size = tx.serialized_size().unwrap_or(0);
        let base_fee = self.params.min_fee_a + self.params.min_fee_b * size as u64;

        // Add script execution fees if applicable
        let script_fee = self.calculate_script_fee(tx);

        Coin(base_fee + script_fee)
    }

    fn calculate_script_fee(&self, tx: &Transaction) -> u64 {
        // For Plutus scripts, calculate based on ExUnits
        // For native scripts, no additional fee
        0 // Simplified for now
    }
}
```

## Success Criteria

- [ ] Valid transactions are accepted
- [ ] Invalid transactions are rejected with clear errors
- [ ] Balance validation is accurate
- [ ] Fee calculation matches Haskell node
- [ ] Certificate processing updates state correctly
- [ ] Minting/burning validation works
- [ ] UTxO set is managed efficiently
- [ ] All tests pass (20+ tests)
- [ ] Alignment with Haskell verified
- [ ] Documentation complete

## Dependencies

- Phase 14: Multi-era support
- cardano-base-rust for crypto

## Estimated Effort

- Input validation: 8-10 hours
- Balance/fee validation: 6-8 hours
- Certificate processing: 10-12 hours
- Minting/burning: 6-8 hours
- UTxO management: 8-10 hours
- Testing and validation: 12-15 hours
- **Total: 50-63 hours**

## Future Enhancements

- [ ] Plutus script execution
- [ ] Reference script support
- [ ] Inline datum optimization
