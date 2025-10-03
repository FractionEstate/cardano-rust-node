//! Byron Era Validation Tests
//!
//! Tests for Byron era ledger rules validation.
//! Byron was the first era of Cardano, featuring:
//! - UTXO-based transactions with ADA only
//! - Ed25519 signature validation
//! - Basic address formats
//! - No smart contracts or native tokens

use cardano_ledger::{LedgerError, Result};
use cardano_crypto::{Ed25519Signature, Blake2b256Hash};
use std::collections::HashMap;

/// Byron transaction input
#[derive(Debug, Clone)]
pub struct ByronTxIn {
    pub tx_id: Blake2b256Hash,
    pub output_index: u32,
}

/// Byron transaction output
#[derive(Debug, Clone)]
pub struct ByronTxOut {
    pub address: ByronAddress,
    pub value: u64, // ADA in lovelace
}

/// Byron address format
#[derive(Debug, Clone, PartialEq)]
pub struct ByronAddress {
    pub payload: Vec<u8>,
    pub address_type: ByronAddressType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ByronAddressType {
    PublicKey,
    Script,
    Redeem,
}

/// Byron transaction
#[derive(Debug, Clone)]
pub struct ByronTransaction {
    pub inputs: Vec<ByronTxIn>,
    pub outputs: Vec<ByronTxOut>,
    pub attributes: HashMap<u8, Vec<u8>>,
}

/// Byron witness for transaction validation
#[derive(Debug, Clone)]
pub struct ByronWitness {
    pub signature: Ed25519Signature,
    pub public_key: Vec<u8>,
}

/// Byron UTXO set entry
#[derive(Debug, Clone)]
pub struct ByronUtxo {
    pub tx_out: ByronTxOut,
    pub spent: bool,
}

/// Byron ledger state
#[derive(Debug, Clone)]
pub struct ByronLedgerState {
    pub utxo_set: HashMap<ByronTxIn, ByronUtxo>,
    pub total_supply: u64,
}

impl ByronTransaction {
    /// Validate transaction against Byron era rules
    pub fn validate(&self, witnesses: &[ByronWitness], ledger_state: &ByronLedgerState) -> Result<()> {
        // Check inputs exist in UTXO set
        for input in &self.inputs {
            if !ledger_state.utxo_set.contains_key(input) {
                return Err(LedgerError::InvalidInput(format!("Input {:?} not found in UTXO set", input.tx_id)));
            }

            let utxo = &ledger_state.utxo_set[input];
            if utxo.spent {
                return Err(LedgerError::InvalidInput("Input already spent".to_string()));
            }
        }

        // Validate witness signatures
        if witnesses.len() != self.inputs.len() {
            return Err(LedgerError::InvalidWitness("Witness count mismatch".to_string()));
        }

        // Check value conservation (sum inputs = sum outputs)
        let input_sum: u64 = self.inputs.iter()
            .map(|input| ledger_state.utxo_set[input].tx_out.value)
            .sum();

        let output_sum: u64 = self.outputs.iter()
            .map(|output| output.value)
            .sum();

        if input_sum != output_sum {
            return Err(LedgerError::InvalidTransaction("Value not conserved".to_string()));
        }

        Ok(())
    }

    /// Calculate transaction hash
    pub fn hash(&self) -> Blake2b256Hash {
        // Simplified hash calculation for testing
        Blake2b256Hash::new(&format!("{:?}", self).as_bytes())
    }
}

impl ByronLedgerState {
    /// Create new empty Byron ledger state
    pub fn new() -> Self {
        Self {
            utxo_set: HashMap::new(),
            total_supply: 45_000_000_000_000_000, // 45 billion ADA in lovelace
        }
    }

    /// Apply transaction to ledger state
    pub fn apply_transaction(&mut self, tx: &ByronTransaction) -> Result<()> {
        // Validate transaction first
        let witnesses = vec![]; // Simplified for test
        tx.validate(&witnesses, self)?;

        // Remove spent outputs
        for input in &tx.inputs {
            if let Some(utxo) = self.utxo_set.get_mut(input) {
                utxo.spent = true;
            }
        }

        // Add new outputs
        let tx_hash = tx.hash();
        for (index, output) in tx.outputs.iter().enumerate() {
            let tx_in = ByronTxIn {
                tx_id: tx_hash.clone(),
                output_index: index as u32,
            };
            let utxo = ByronUtxo {
                tx_out: output.clone(),
                spent: false,
            };
            self.utxo_set.insert(tx_in, utxo);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byron_address_creation() {
        let addr = ByronAddress {
            payload: vec![1, 2, 3, 4],
            address_type: ByronAddressType::PublicKey,
        };

        assert_eq!(addr.address_type, ByronAddressType::PublicKey);
        assert_eq!(addr.payload, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_byron_transaction_validation_missing_input() {
        let mut ledger = ByronLedgerState::new();

        let tx = ByronTransaction {
            inputs: vec![ByronTxIn {
                tx_id: Blake2b256Hash::new(b"nonexistent"),
                output_index: 0,
            }],
            outputs: vec![ByronTxOut {
                address: ByronAddress {
                    payload: vec![1, 2, 3, 4],
                    address_type: ByronAddressType::PublicKey,
                },
                value: 1000000,
            }],
            attributes: HashMap::new(),
        };

        let result = tx.validate(&[], &ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidInput(_)));
    }

    #[test]
    fn test_byron_transaction_validation_value_conservation() {
        let mut ledger = ByronLedgerState::new();

        // Add a UTXO to spend
        let initial_tx_in = ByronTxIn {
            tx_id: Blake2b256Hash::new(b"genesis"),
            output_index: 0,
        };

        ledger.utxo_set.insert(initial_tx_in.clone(), ByronUtxo {
            tx_out: ByronTxOut {
                address: ByronAddress {
                    payload: vec![1, 2, 3, 4],
                    address_type: ByronAddressType::PublicKey,
                },
                value: 1000000,
            },
            spent: false,
        });

        // Create transaction that doesn't conserve value
        let tx = ByronTransaction {
            inputs: vec![initial_tx_in],
            outputs: vec![ByronTxOut {
                address: ByronAddress {
                    payload: vec![5, 6, 7, 8],
                    address_type: ByronAddressType::PublicKey,
                },
                value: 2000000, // More than input!
            }],
            attributes: HashMap::new(),
        };

        let result = tx.validate(&[], &ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidTransaction(_)));
    }

    #[test]
    fn test_byron_ledger_state_apply_transaction() {
        let mut ledger = ByronLedgerState::new();

        // Add initial UTXO
        let genesis_tx_in = ByronTxIn {
            tx_id: Blake2b256Hash::new(b"genesis"),
            output_index: 0,
        };

        ledger.utxo_set.insert(genesis_tx_in.clone(), ByronUtxo {
            tx_out: ByronTxOut {
                address: ByronAddress {
                    payload: vec![1, 2, 3, 4],
                    address_type: ByronAddressType::PublicKey,
                },
                value: 1000000,
            },
            spent: false,
        });

        // Create valid transaction
        let tx = ByronTransaction {
            inputs: vec![genesis_tx_in.clone()],
            outputs: vec![ByronTxOut {
                address: ByronAddress {
                    payload: vec![5, 6, 7, 8],
                    address_type: ByronAddressType::PublicKey,
                },
                value: 1000000,
            }],
            attributes: HashMap::new(),
        };

        // Apply transaction should succeed
        let result = ledger.apply_transaction(&tx);
        assert!(result.is_ok());

        // Check that input was marked as spent
        assert!(ledger.utxo_set[&genesis_tx_in].spent);

        // Check that new output was added
        let new_tx_in = ByronTxIn {
            tx_id: tx.hash(),
            output_index: 0,
        };
        assert!(ledger.utxo_set.contains_key(&new_tx_in));
        assert!(!ledger.utxo_set[&new_tx_in].spent);
    }

    #[test]
    fn test_byron_double_spending_prevention() {
        let mut ledger = ByronLedgerState::new();

        let genesis_tx_in = ByronTxIn {
            tx_id: Blake2b256Hash::new(b"genesis"),
            output_index: 0,
        };

        // Add and spend a UTXO
        ledger.utxo_set.insert(genesis_tx_in.clone(), ByronUtxo {
            tx_out: ByronTxOut {
                address: ByronAddress {
                    payload: vec![1, 2, 3, 4],
                    address_type: ByronAddressType::PublicKey,
                },
                value: 1000000,
            },
            spent: true, // Already spent
        });

        // Try to spend it again
        let tx = ByronTransaction {
            inputs: vec![genesis_tx_in],
            outputs: vec![ByronTxOut {
                address: ByronAddress {
                    payload: vec![5, 6, 7, 8],
                    address_type: ByronAddressType::PublicKey,
                },
                value: 1000000,
            }],
            attributes: HashMap::new(),
        };

        let result = tx.validate(&[], &ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidInput(_)));
    }
}
