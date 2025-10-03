//! Allegra Era Validation Tests
//!
//! Tests for Allegra era ledger rules validation.
//! Allegra introduced native scripts and transaction validity intervals:
//! - Native script support (multisig, time locks, etc.)
//! - Transaction validity intervals (not before/not after)
//! - Script validation in transaction processing

use cardano_ledger::{LedgerError, Result};
use cardano_crypto::{Ed25519Signature, Blake2b256Hash, Ed25519KeyHash};
use std::collections::HashMap;

/// Re-export Shelley types that Allegra extends
pub use crate::test_shelley_validation::{
    ShelleyAddress, ShelleyTxIn, ShelleyTxOut, Certificate,
    ShelleyLedgerState, NetworkId, Credential, StakeReference
};

/// Native script types introduced in Allegra
#[derive(Debug, Clone, PartialEq)]
pub enum NativeScript {
    /// Require signature from specific key
    RequireSignature(Ed25519KeyHash),
    /// Require all sub-scripts to be satisfied
    RequireAllOf(Vec<NativeScript>),
    /// Require any one of the sub-scripts to be satisfied
    RequireAnyOf(Vec<NativeScript>),
    /// Require N of M sub-scripts to be satisfied
    RequireNOf {
        n: u32,
        scripts: Vec<NativeScript>,
    },
    /// Script valid only after this slot
    RequireTimeBefore(u64), // slot number
    /// Script valid only before this slot
    RequireTimeAfter(u64),  // slot number
}

/// Validity interval for transactions
#[derive(Debug, Clone)]
pub struct ValidityInterval {
    /// Transaction invalid before this slot
    pub invalid_before: Option<u64>,
    /// Transaction invalid after this slot (inclusive)
    pub invalid_hereafter: Option<u64>,
}

/// Allegra transaction output with script support
#[derive(Debug, Clone)]
pub struct AllegraTransactionOutput {
    pub address: ShelleyAddress,
    pub value: u64,
    pub script: Option<NativeScript>, // Script that locks the output
}

/// Allegra transaction with native script support and validity intervals
#[derive(Debug, Clone)]
pub struct AllegraTransaction {
    pub inputs: Vec<ShelleyTxIn>,
    pub outputs: Vec<AllegraTransactionOutput>,
    pub fee: u64,
    pub validity_interval: ValidityInterval,
    pub certificates: Vec<Certificate>,
    pub withdrawals: HashMap<ShelleyAddress, u64>,
    pub native_scripts: Vec<NativeScript>, // Scripts referenced in transaction
}

/// Witness set for Allegra transactions
#[derive(Debug, Clone)]
pub struct AllegraWitnessSet {
    pub vkey_witnesses: Vec<VKeyWitness>,
    pub native_scripts: Vec<NativeScript>,
}

#[derive(Debug, Clone)]
pub struct VKeyWitness {
    pub vkey: Ed25519KeyHash,
    pub signature: Ed25519Signature,
}

/// Allegra UTXO entry
#[derive(Debug, Clone)]
pub struct AllegraUtxo {
    pub tx_out: AllegraTransactionOutput,
    pub spent: bool,
}

/// Extended Shelley ledger state for Allegra
#[derive(Debug, Clone)]
pub struct AllegraLedgerState {
    pub base: ShelleyLedgerState,
    pub allegra_utxo_set: HashMap<ShelleyTxIn, AllegraUtxo>,
    pub current_slot: u64,
}

impl NativeScript {
    /// Evaluate native script against witness set and current slot
    pub fn evaluate(&self, witnesses: &AllegraWitnessSet, current_slot: u64) -> bool {
        match self {
            NativeScript::RequireSignature(key_hash) => {
                witnesses.vkey_witnesses.iter()
                    .any(|witness| witness.vkey == *key_hash)
            }

            NativeScript::RequireAllOf(scripts) => {
                scripts.iter().all(|script| script.evaluate(witnesses, current_slot))
            }

            NativeScript::RequireAnyOf(scripts) => {
                !scripts.is_empty() &&
                scripts.iter().any(|script| script.evaluate(witnesses, current_slot))
            }

            NativeScript::RequireNOf { n, scripts } => {
                let satisfied_count = scripts.iter()
                    .filter(|script| script.evaluate(witnesses, current_slot))
                    .count() as u32;
                satisfied_count >= *n && *n > 0 && (*n as usize) <= scripts.len()
            }

            NativeScript::RequireTimeBefore(slot) => {
                current_slot < *slot
            }

            NativeScript::RequireTimeAfter(slot) => {
                current_slot >= *slot
            }
        }
    }

    /// Calculate script hash for address derivation
    pub fn hash(&self) -> Blake2b256Hash {
        Blake2b256Hash::new(&format!("{:?}", self).as_bytes())
    }
}

impl ValidityInterval {
    pub fn new() -> Self {
        Self {
            invalid_before: None,
            invalid_hereafter: None,
        }
    }

    pub fn is_valid_at_slot(&self, slot: u64) -> bool {
        if let Some(before) = self.invalid_before {
            if slot < before {
                return false;
            }
        }

        if let Some(after) = self.invalid_hereafter {
            if slot > after {
                return false;
            }
        }

        true
    }
}

impl AllegraTransaction {
    /// Validate transaction against Allegra era rules
    pub fn validate(&self, witnesses: &AllegraWitnessSet, ledger_state: &AllegraLedgerState) -> Result<()> {
        // Check validity interval
        if !self.validity_interval.is_valid_at_slot(ledger_state.current_slot) {
            return Err(LedgerError::InvalidTransaction("Transaction outside validity interval".to_string()));
        }

        // Check inputs exist and are unspent
        for input in &self.inputs {
            if !ledger_state.allegra_utxo_set.contains_key(input) {
                return Err(LedgerError::InvalidInput(format!("Input {:?} not found", input.tx_id)));
            }

            if ledger_state.allegra_utxo_set[input].spent {
                return Err(LedgerError::InvalidInput("Input already spent".to_string()));
            }
        }

        // Validate native scripts for inputs
        for input in &self.inputs {
            let utxo = &ledger_state.allegra_utxo_set[input];
            if let Some(script) = &utxo.tx_out.script {
                if !script.evaluate(witnesses, ledger_state.current_slot) {
                    return Err(LedgerError::InvalidScript("Script evaluation failed".to_string()));
                }
            }
        }

        // Validate all referenced native scripts are included
        let mut required_scripts = std::collections::HashSet::new();
        for input in &self.inputs {
            let utxo = &ledger_state.allegra_utxo_set[input];
            if let Some(script) = &utxo.tx_out.script {
                required_scripts.insert(script.hash());
            }
        }

        let provided_scripts: std::collections::HashSet<_> = witnesses.native_scripts.iter()
            .map(|s| s.hash())
            .collect();

        if !required_scripts.is_subset(&provided_scripts) {
            return Err(LedgerError::InvalidScript("Missing required native scripts".to_string()));
        }

        // Check value conservation
        let input_sum: u64 = self.inputs.iter()
            .map(|input| ledger_state.allegra_utxo_set[input].tx_out.value)
            .sum();

        let output_sum: u64 = self.outputs.iter()
            .map(|output| output.value)
            .sum();

        let withdrawal_sum: u64 = self.withdrawals.values().sum();

        if input_sum + withdrawal_sum != output_sum + self.fee {
            return Err(LedgerError::InvalidTransaction("Value not conserved".to_string()));
        }

        Ok(())
    }

    pub fn hash(&self) -> Blake2b256Hash {
        Blake2b256Hash::new(&format!("{:?}", self).as_bytes())
    }
}

impl AllegraLedgerState {
    pub fn new() -> Self {
        Self {
            base: ShelleyLedgerState::new(),
            allegra_utxo_set: HashMap::new(),
            current_slot: 0,
        }
    }

    pub fn apply_transaction(&mut self, tx: &AllegraTransaction, witnesses: &AllegraWitnessSet) -> Result<()> {
        tx.validate(witnesses, self)?;

        // Remove spent outputs
        for input in &tx.inputs {
            if let Some(utxo) = self.allegra_utxo_set.get_mut(input) {
                utxo.spent = true;
            }
        }

        // Add new outputs
        let tx_hash = tx.hash();
        for (index, output) in tx.outputs.iter().enumerate() {
            let tx_in = ShelleyTxIn {
                tx_id: tx_hash.clone(),
                output_index: index as u32,
            };
            let utxo = AllegraUtxo {
                tx_out: output.clone(),
                spent: false,
            };
            self.allegra_utxo_set.insert(tx_in, utxo);
        }

        // Update base ledger state for non-script operations
        // (certificates, withdrawals, etc.)
        self.base.treasury += tx.fee;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_address() -> ShelleyAddress {
        ShelleyAddress {
            payment_credential: Credential::Key(Ed25519KeyHash::new(b"test_payment_key")),
            stake_credential: Some(StakeReference::Credential(
                Credential::Key(Ed25519KeyHash::new(b"test_stake_key"))
            )),
            network_id: NetworkId::Testnet,
        }
    }

    #[test]
    fn test_native_script_require_signature() {
        let key_hash = Ed25519KeyHash::new(b"test_key");
        let script = NativeScript::RequireSignature(key_hash.clone());

        let witnesses = AllegraWitnessSet {
            vkey_witnesses: vec![VKeyWitness {
                vkey: key_hash,
                signature: Ed25519Signature::new(&[1; 64]),
            }],
            native_scripts: vec![],
        };

        assert!(script.evaluate(&witnesses, 1000));

        // Test with wrong key
        let wrong_witnesses = AllegraWitnessSet {
            vkey_witnesses: vec![VKeyWitness {
                vkey: Ed25519KeyHash::new(b"wrong_key"),
                signature: Ed25519Signature::new(&[1; 64]),
            }],
            native_scripts: vec![],
        };

        assert!(!script.evaluate(&wrong_witnesses, 1000));
    }

    #[test]
    fn test_native_script_require_all_of() {
        let key1 = Ed25519KeyHash::new(b"key1");
        let key2 = Ed25519KeyHash::new(b"key2");

        let script = NativeScript::RequireAllOf(vec![
            NativeScript::RequireSignature(key1.clone()),
            NativeScript::RequireSignature(key2.clone()),
        ]);

        // Test with both signatures
        let witnesses = AllegraWitnessSet {
            vkey_witnesses: vec![
                VKeyWitness {
                    vkey: key1.clone(),
                    signature: Ed25519Signature::new(&[1; 64]),
                },
                VKeyWitness {
                    vkey: key2.clone(),
                    signature: Ed25519Signature::new(&[2; 64]),
                },
            ],
            native_scripts: vec![],
        };

        assert!(script.evaluate(&witnesses, 1000));

        // Test with only one signature
        let partial_witnesses = AllegraWitnessSet {
            vkey_witnesses: vec![VKeyWitness {
                vkey: key1,
                signature: Ed25519Signature::new(&[1; 64]),
            }],
            native_scripts: vec![],
        };

        assert!(!script.evaluate(&partial_witnesses, 1000));
    }

    #[test]
    fn test_native_script_require_n_of() {
        let key1 = Ed25519KeyHash::new(b"key1");
        let key2 = Ed25519KeyHash::new(b"key2");
        let key3 = Ed25519KeyHash::new(b"key3");

        let script = NativeScript::RequireNOf {
            n: 2,
            scripts: vec![
                NativeScript::RequireSignature(key1.clone()),
                NativeScript::RequireSignature(key2.clone()),
                NativeScript::RequireSignature(key3.clone()),
            ],
        };

        // Test with 2 out of 3 signatures
        let witnesses = AllegraWitnessSet {
            vkey_witnesses: vec![
                VKeyWitness {
                    vkey: key1.clone(),
                    signature: Ed25519Signature::new(&[1; 64]),
                },
                VKeyWitness {
                    vkey: key2.clone(),
                    signature: Ed25519Signature::new(&[2; 64]),
                },
            ],
            native_scripts: vec![],
        };

        assert!(script.evaluate(&witnesses, 1000));

        // Test with only 1 signature
        let insufficient_witnesses = AllegraWitnessSet {
            vkey_witnesses: vec![VKeyWitness {
                vkey: key1,
                signature: Ed25519Signature::new(&[1; 64]),
            }],
            native_scripts: vec![],
        };

        assert!(!script.evaluate(&insufficient_witnesses, 1000));
    }

    #[test]
    fn test_native_script_time_locks() {
        let before_script = NativeScript::RequireTimeBefore(2000);
        let after_script = NativeScript::RequireTimeAfter(1000);

        let witnesses = AllegraWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
        };

        // Test at slot 1500 (between 1000 and 2000)
        assert!(after_script.evaluate(&witnesses, 1500));   // >= 1000
        assert!(before_script.evaluate(&witnesses, 1500));  // < 2000

        // Test at slot 500 (before both)
        assert!(!after_script.evaluate(&witnesses, 500));  // < 1000
        assert!(before_script.evaluate(&witnesses, 500));  // < 2000

        // Test at slot 2500 (after both)
        assert!(after_script.evaluate(&witnesses, 2500));  // >= 1000
        assert!(!before_script.evaluate(&witnesses, 2500)); // >= 2000
    }

    #[test]
    fn test_validity_interval_validation() {
        let mut ledger = AllegraLedgerState::new();
        ledger.current_slot = 1500;

        // Create transaction with validity interval [1000, 2000]
        let tx = AllegraTransaction {
            inputs: vec![],
            outputs: vec![],
            fee: 0,
            validity_interval: ValidityInterval {
                invalid_before: Some(1000),
                invalid_hereafter: Some(2000),
            },
            certificates: vec![],
            withdrawals: HashMap::new(),
            native_scripts: vec![],
        };

        let witnesses = AllegraWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
        };

        // Should be valid at current slot 1500
        assert!(tx.validate(&witnesses, &ledger).is_ok());

        // Test outside interval
        ledger.current_slot = 500; // Before invalid_before
        assert!(tx.validate(&witnesses, &ledger).is_err());

        ledger.current_slot = 2500; // After invalid_hereafter
        assert!(tx.validate(&witnesses, &ledger).is_err());
    }

    #[test]
    fn test_script_locked_output_validation() {
        let mut ledger = AllegraLedgerState::new();
        ledger.current_slot = 1500;

        let key_hash = Ed25519KeyHash::new(b"test_key");
        let script = NativeScript::RequireSignature(key_hash.clone());

        // Create a script-locked UTXO
        let input_tx_in = ShelleyTxIn {
            tx_id: Blake2b256Hash::new(b"input_tx"),
            output_index: 0,
        };

        let script_output = AllegraTransactionOutput {
            address: create_test_address(),
            value: 1_000_000,
            script: Some(script.clone()),
        };

        ledger.allegra_utxo_set.insert(input_tx_in.clone(), AllegraUtxo {
            tx_out: script_output,
            spent: false,
        });

        // Create transaction spending the script-locked output
        let tx = AllegraTransaction {
            inputs: vec![input_tx_in],
            outputs: vec![AllegraTransactionOutput {
                address: create_test_address(),
                value: 900_000,
                script: None,
            }],
            fee: 100_000,
            validity_interval: ValidityInterval::new(),
            certificates: vec![],
            withdrawals: HashMap::new(),
            native_scripts: vec![script.clone()],
        };

        // Test with correct witness
        let witnesses = AllegraWitnessSet {
            vkey_witnesses: vec![VKeyWitness {
                vkey: key_hash.clone(),
                signature: Ed25519Signature::new(&[1; 64]),
            }],
            native_scripts: vec![script],
        };

        assert!(tx.validate(&witnesses, &ledger).is_ok());

        // Test with missing witness
        let no_witnesses = AllegraWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
        };

        assert!(tx.validate(&no_witnesses, &ledger).is_err());
    }

    #[test]
    fn test_complex_native_script() {
        // Create a complex script: (key1 AND key2) OR (key3 AND time_after_1000)
        let key1 = Ed25519KeyHash::new(b"key1");
        let key2 = Ed25519KeyHash::new(b"key2");
        let key3 = Ed25519KeyHash::new(b"key3");

        let script = NativeScript::RequireAnyOf(vec![
            NativeScript::RequireAllOf(vec![
                NativeScript::RequireSignature(key1.clone()),
                NativeScript::RequireSignature(key2.clone()),
            ]),
            NativeScript::RequireAllOf(vec![
                NativeScript::RequireSignature(key3.clone()),
                NativeScript::RequireTimeAfter(1000),
            ]),
        ]);

        // Test first branch: key1 + key2 at early time
        let witnesses1 = AllegraWitnessSet {
            vkey_witnesses: vec![
                VKeyWitness {
                    vkey: key1,
                    signature: Ed25519Signature::new(&[1; 64]),
                },
                VKeyWitness {
                    vkey: key2,
                    signature: Ed25519Signature::new(&[2; 64]),
                },
            ],
            native_scripts: vec![],
        };

        assert!(script.evaluate(&witnesses1, 500)); // Before time lock

        // Test second branch: key3 after time lock
        let witnesses2 = AllegraWitnessSet {
            vkey_witnesses: vec![VKeyWitness {
                vkey: key3.clone(),
                signature: Ed25519Signature::new(&[3; 64]),
            }],
            native_scripts: vec![],
        };

        assert!(script.evaluate(&witnesses2, 1500)); // After time lock
        assert!(!script.evaluate(&witnesses2, 500)); // Before time lock
    }
}
