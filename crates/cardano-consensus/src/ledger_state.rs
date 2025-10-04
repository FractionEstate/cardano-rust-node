//! Ledger State Integration
//!
//! This module provides the integration layer between the block production service
//! and the ledger state (UTxO set, transaction validation, etc.). It ensures that
//! blocks produced contain only valid transactions and updates the ledger state
//! after blocks are successfully forged.

use crate::{
    block_production::{BlockHeader, Transaction, TxInput, TxOutput},
    ConsensusError, Result,
};
#[cfg(test)]
use cardano_crypto::hash::Blake2b256Hash;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Protocol parameters for transaction validation
#[derive(Debug, Clone)]
pub struct ProtocolParameters {
    /// Minimum fee per transaction (fixed component)
    pub min_fee_a: u64,
    /// Minimum fee per byte (variable component)
    pub min_fee_b: u64,
    /// Maximum transaction size in bytes
    pub max_tx_size: usize,
    /// Maximum block size in bytes
    pub max_block_size: usize,
    /// Minimum UTxO value (lovelace)
    pub min_utxo_value: u64,
    /// Maximum number of transactions per block
    pub max_tx_per_block: usize,
}

impl Default for ProtocolParameters {
    fn default() -> Self {
        Self {
            min_fee_a: 44,      // Conway era default
            min_fee_b: 155381,  // Conway era default
            max_tx_size: 16384, // 16 KB
            max_block_size: 90112, // ~90 KB
            min_utxo_value: 1_000_000, // 1 ADA
            max_tx_per_block: 10000,
        }
    }
}

/// UTxO entry in the ledger
#[derive(Debug, Clone)]
pub struct UtxoEntry {
    /// Transaction output
    pub output: TxOutput,
    /// Whether this UTxO has been spent
    pub spent: bool,
}

/// Ledger state manager
///
/// Manages the current state of the ledger including:
/// - UTxO set (unspent transaction outputs)
/// - Protocol parameters
/// - Current epoch and slot
pub struct LedgerState {
    /// UTxO set (transaction input -> output mapping)
    utxo_set: Arc<RwLock<HashMap<TxInput, UtxoEntry>>>,
    /// Protocol parameters
    protocol_params: Arc<RwLock<ProtocolParameters>>,
    /// Current epoch
    current_epoch: Arc<RwLock<u64>>,
    /// Current slot
    current_slot: Arc<RwLock<u64>>,
}

impl LedgerState {
    /// Create a new ledger state with default parameters
    pub fn new() -> Self {
        Self {
            utxo_set: Arc::new(RwLock::new(HashMap::new())),
            protocol_params: Arc::new(RwLock::new(ProtocolParameters::default())),
            current_epoch: Arc::new(RwLock::new(0)),
            current_slot: Arc::new(RwLock::new(0)),
        }
    }

    /// Create a new ledger state with custom protocol parameters
    pub fn with_protocol_params(params: ProtocolParameters) -> Self {
        Self {
            utxo_set: Arc::new(RwLock::new(HashMap::new())),
            protocol_params: Arc::new(RwLock::new(params)),
            current_epoch: Arc::new(RwLock::new(0)),
            current_slot: Arc::new(RwLock::new(0)),
        }
    }

    /// Update current slot (should be called by slot notifier)
    pub async fn update_slot(&self, slot: u64, epoch: u64) {
        *self.current_slot.write().await = slot;
        *self.current_epoch.write().await = epoch;
    }

    /// Get current slot
    pub async fn current_slot(&self) -> u64 {
        *self.current_slot.read().await
    }

    /// Get current epoch
    pub async fn current_epoch(&self) -> u64 {
        *self.current_epoch.read().await
    }

    /// Get protocol parameters
    pub async fn protocol_params(&self) -> ProtocolParameters {
        self.protocol_params.read().await.clone()
    }

    /// Check if a transaction input exists in the UTxO set
    pub async fn utxo_exists(&self, input: &TxInput) -> bool {
        let utxo_set = self.utxo_set.read().await;
        utxo_set.contains_key(input)
    }

    /// Get a UTxO entry if it exists and is unspent
    pub async fn get_utxo(&self, input: &TxInput) -> Option<TxOutput> {
        let utxo_set = self.utxo_set.read().await;
        utxo_set.get(input).and_then(|entry| {
            if !entry.spent {
                Some(entry.output.clone())
            } else {
                None
            }
        })
    }

    /// Add a UTxO entry to the set
    pub async fn add_utxo(&self, input: TxInput, output: TxOutput) {
        let mut utxo_set = self.utxo_set.write().await;
        utxo_set.insert(input, UtxoEntry { output, spent: false });
    }

    /// Mark a UTxO as spent
    pub async fn spend_utxo(&self, input: &TxInput) -> Result<()> {
        let mut utxo_set = self.utxo_set.write().await;

        if let Some(entry) = utxo_set.get_mut(input) {
            if entry.spent {
                return Err(ConsensusError::InvalidInput(
                    "UTxO already spent".to_string(),
                ));
            }
            entry.spent = true;
            Ok(())
        } else {
            Err(ConsensusError::InvalidInput(
                "UTxO not found".to_string(),
            ))
        }
    }

    /// Calculate minimum fee for a transaction
    pub async fn calculate_min_fee(&self, tx_size: usize) -> u64 {
        let params = self.protocol_params.read().await;
        params.min_fee_a + (params.min_fee_b * tx_size as u64)
    }

    /// Validate a single transaction against the current ledger state
    pub async fn validate_transaction(&self, tx: &Transaction) -> Result<()> {
        // 1. Basic structure validation
        if tx.inputs.is_empty() {
            return Err(ConsensusError::InvalidTransaction(
                "Transaction has no inputs".to_string(),
            ));
        }

        if tx.outputs.is_empty() {
            return Err(ConsensusError::InvalidTransaction(
                "Transaction has no outputs".to_string(),
            ));
        }

        // 2. Size validation
        let params = self.protocol_params.read().await;
        if tx.size as usize > params.max_tx_size {
            return Err(ConsensusError::InvalidTransaction(
                format!("Transaction too large: {} > {}", tx.size, params.max_tx_size),
            ));
        }

        // 3. Input validation - all inputs must exist and be unspent
        let mut total_input: u64 = 0;
        let utxo_set = self.utxo_set.read().await;

        for input in &tx.inputs {
            match utxo_set.get(input) {
                Some(entry) if !entry.spent => {
                    total_input = total_input.checked_add(entry.output.value)
                        .ok_or_else(|| ConsensusError::InvalidTransaction(
                            "Input value overflow".to_string()
                        ))?;
                }
                Some(_) => {
                    return Err(ConsensusError::InvalidInput(
                        format!("Input already spent: {:?}", input.tx_hash),
                    ));
                }
                None => {
                    return Err(ConsensusError::InvalidInput(
                        format!("Input not found: {:?}", input.tx_hash),
                    ));
                }
            }
        }

        // 4. Output validation - must meet minimum UTxO value
        let mut total_output: u64 = 0;
        for output in &tx.outputs {
            if output.value < params.min_utxo_value {
                return Err(ConsensusError::InvalidTransaction(
                    format!("Output below minimum UTxO: {} < {}",
                        output.value, params.min_utxo_value),
                ));
            }

            total_output = total_output.checked_add(output.value)
                .ok_or_else(|| ConsensusError::InvalidTransaction(
                    "Output value overflow".to_string()
                ))?;
        }

        // 5. Fee validation
        let min_fee = params.min_fee_a + (params.min_fee_b * tx.size as u64);
        if tx.fee < min_fee {
            return Err(ConsensusError::InvalidTransaction(
                format!("Fee too low: {} < {}", tx.fee, min_fee),
            ));
        }

        // 6. Value conservation: inputs = outputs + fee
        let expected_total = total_output.checked_add(tx.fee)
            .ok_or_else(|| ConsensusError::InvalidTransaction(
                "Output + fee overflow".to_string()
            ))?;

        if total_input != expected_total {
            return Err(ConsensusError::InvalidTransaction(
                format!("Value not conserved: inputs={}, outputs+fee={}",
                    total_input, expected_total),
            ));
        }

        Ok(())
    }

    /// Apply a validated transaction to the ledger state
    ///
    /// This consumes the inputs and creates new UTxOs for the outputs.
    /// Should only be called after successful validation.
    pub async fn apply_transaction(&self, tx: &Transaction) -> Result<()> {
        let mut utxo_set = self.utxo_set.write().await;

        // 1. Spend all inputs
        for input in &tx.inputs {
            if let Some(entry) = utxo_set.get_mut(input) {
                if entry.spent {
                    return Err(ConsensusError::InvalidInput(
                        "Cannot spend already spent UTxO".to_string(),
                    ));
                }
                entry.spent = true;
            } else {
                return Err(ConsensusError::InvalidInput(
                    "Cannot spend non-existent UTxO".to_string(),
                ));
            }
        }

        // 2. Create new UTxOs for outputs
        for (index, output) in tx.outputs.iter().enumerate() {
            let new_input = TxInput {
                tx_hash: tx.tx_id,
                output_index: index as u32,
            };
            utxo_set.insert(new_input, UtxoEntry {
                output: output.clone(),
                spent: false,
            });
        }

        Ok(())
    }

    /// Apply a block to the ledger state
    ///
    /// This applies all transactions in the block sequentially.
    pub async fn apply_block(&self, header: &BlockHeader, transactions: &[Transaction]) -> Result<()> {
        // Update slot (epoch can be derived from slot if needed, using default 0 for now)
        self.update_slot(header.slot.0, 0).await;

        // Apply each transaction
        for tx in transactions {
            self.apply_transaction(tx).await?;
        }

        Ok(())
    }

    /// Get total UTxO count
    pub async fn utxo_count(&self) -> usize {
        let utxo_set = self.utxo_set.read().await;
        utxo_set.values().filter(|e| !e.spent).count()
    }

    /// Get total value in UTxO set
    pub async fn total_utxo_value(&self) -> u64 {
        let utxo_set = self.utxo_set.read().await;
        utxo_set.values()
            .filter(|e| !e.spent)
            .map(|e| e.output.value)
            .sum()
    }
}

impl Default for LedgerState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tx() -> Transaction {
        let tx_id = Blake2b256Hash::from_bytes(&[1u8; 32]).unwrap();

        Transaction {
            tx_id,
            inputs: vec![TxInput {
                tx_hash: Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap(),
                output_index: 0,
            }],
            outputs: vec![
                TxOutput {
                    address: Blake2b256Hash::from_bytes(&[2u8; 32]).unwrap(),
                    value: 2_000_000, // 2 ADA
                },
                TxOutput {
                    address: Blake2b256Hash::from_bytes(&[3u8; 32]).unwrap(),
                    value: 2_000_000, // 2 ADA
                },
            ],
            fee: 32_000_000, // ~32 ADA (enough for size 200)
            size: 200,
        }
    }

    #[tokio::test]
    async fn test_ledger_state_creation() {
        let ledger = LedgerState::new();

        assert_eq!(ledger.current_slot().await, 0);
        assert_eq!(ledger.current_epoch().await, 0);
        assert_eq!(ledger.utxo_count().await, 0);
    }

    #[tokio::test]
    async fn test_utxo_operations() {
        let ledger = LedgerState::new();

        let input = TxInput {
            tx_hash: Blake2b256Hash::from_bytes(&[1u8; 32]).unwrap(),
            output_index: 0,
        };

        let output = TxOutput {
            address: Blake2b256Hash::from_bytes(&[2u8; 32]).unwrap(),
            value: 5_000_000,
        };

        // Add UTxO
        ledger.add_utxo(input.clone(), output.clone()).await;

        assert!(ledger.utxo_exists(&input).await);
        assert_eq!(ledger.utxo_count().await, 1);

        // Get UTxO
        let retrieved = ledger.get_utxo(&input).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, 5_000_000);

        // Spend UTxO
        ledger.spend_utxo(&input).await.unwrap();
        assert!(ledger.get_utxo(&input).await.is_none());
    }

    #[tokio::test]
    async fn test_transaction_validation_no_inputs() {
        let ledger = LedgerState::new();

        let mut tx = create_test_tx();
        tx.inputs.clear();

        let result = ledger.validate_transaction(&tx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no inputs"));
    }

    #[tokio::test]
    async fn test_transaction_validation_missing_input() {
        let ledger = LedgerState::new();

        let tx = create_test_tx();

        // Don't add the input UTxO
        let result = ledger.validate_transaction(&tx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_transaction_validation_success() {
        let ledger = LedgerState::new();

        let tx = create_test_tx();

        // Add the input UTxO with sufficient value
        let input_utxo = TxOutput {
            address: Blake2b256Hash::from_bytes(&[1u8; 32]).unwrap(),
            value: 36_000_000, // 36 ADA (enough for 2 + 2 + 32 fee)
        };

        ledger.add_utxo(tx.inputs[0].clone(), input_utxo).await;

        // Should validate successfully
        let result = ledger.validate_transaction(&tx).await;
        assert!(result.is_ok(), "Validation failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_transaction_application() {
        let ledger = LedgerState::new();

        let tx = create_test_tx();

        // Add input UTxO
        let input_utxo = TxOutput {
            address: Blake2b256Hash::from_bytes(&[1u8; 32]).unwrap(),
            value: 36_000_000, // 36 ADA
        };
        ledger.add_utxo(tx.inputs[0].clone(), input_utxo).await;

        // Apply transaction
        let result = ledger.apply_transaction(&tx).await;
        assert!(result.is_ok());

        // Input should be spent
        assert!(ledger.get_utxo(&tx.inputs[0]).await.is_none());

        // Outputs should exist
        let output0_input = TxInput {
            tx_hash: tx.tx_id,
            output_index: 0,
        };
        let output1_input = TxInput {
            tx_hash: tx.tx_id,
            output_index: 1,
        };

        assert!(ledger.get_utxo(&output0_input).await.is_some());
        assert!(ledger.get_utxo(&output1_input).await.is_some());

        // Should have 2 unspent UTxOs now (the outputs)
        assert_eq!(ledger.utxo_count().await, 2);
    }

    #[tokio::test]
    async fn test_fee_calculation() {
        let ledger = LedgerState::new();

        let params = ledger.protocol_params().await;
        let tx_size = 200usize;

        let min_fee = ledger.calculate_min_fee(tx_size).await;
        let expected = params.min_fee_a + (params.min_fee_b * tx_size as u64);

        assert_eq!(min_fee, expected);
    }
}
