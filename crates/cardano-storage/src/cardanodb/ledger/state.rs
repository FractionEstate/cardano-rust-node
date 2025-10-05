//! Ledger state representation
//!
//! This module defines the in-memory ledger state structure containing
//! the UTxO set, stake distribution, and other protocol state.

use crate::cardanodb::types::{Blake2b256Hash, BlockNo, EpochNo, SlotNo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Transaction input (reference to a previous output)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TxInput {
    /// Transaction ID (hash) that created this output
    pub tx_id: Blake2b256Hash,
    /// Index of the output within that transaction
    pub output_index: u32,
}

impl TxInput {
    /// Create a new transaction input
    pub fn new(tx_id: Blake2b256Hash, output_index: u32) -> Self {
        Self {
            tx_id,
            output_index,
        }
    }
}

/// Transaction output (unspent output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOutput {
    /// Address receiving the funds
    pub address: Vec<u8>, // Generic address bytes (can be Byron, Shelley, etc.)
    /// Value in lovelace (1 ADA = 1,000,000 lovelace)
    pub value: u64,
    /// Optional datum hash (for Plutus scripts)
    pub datum_hash: Option<Blake2b256Hash>,
    /// Optional inline datum (Babbage+)
    pub inline_datum: Option<Vec<u8>>,
    /// Optional script reference (Babbage+)
    pub script_ref: Option<Vec<u8>>,
}

impl TxOutput {
    /// Create a simple payment output
    pub fn new(address: Vec<u8>, value: u64) -> Self {
        Self {
            address,
            value,
            datum_hash: None,
            inline_datum: None,
            script_ref: None,
        }
    }

    /// Create a script output with datum hash (Alonzo+)
    pub fn with_datum_hash(address: Vec<u8>, value: u64, datum_hash: Blake2b256Hash) -> Self {
        Self {
            address,
            value,
            datum_hash: Some(datum_hash),
            inline_datum: None,
            script_ref: None,
        }
    }
}

/// In-memory ledger state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerState {
    /// Current slot
    pub slot: SlotNo,

    /// Current block number
    pub block_no: BlockNo,

    /// Current epoch
    pub epoch: EpochNo,

    /// UTxO set: mapping from transaction inputs to outputs
    pub utxo: HashMap<TxInput, TxOutput>,
    // TODO: Add stake distribution, delegations, etc. in future phases
    // pub stake: HashMap<StakeCredential, Coin>,
    // pub delegations: HashMap<StakeCredential, PoolId>,
    // pub pools: HashMap<PoolId, PoolParameters>,
    // pub protocol_params: ProtocolParameters,
}

impl LedgerState {
    /// Create the genesis ledger state
    pub fn genesis() -> Self {
        Self {
            slot: SlotNo(0),
            block_no: BlockNo(0),
            epoch: EpochNo(0),
            utxo: HashMap::new(),
        }
    }

    /// Check if this is the genesis state
    pub fn is_genesis(&self) -> bool {
        self.block_no == BlockNo(0)
    }

    /// Get a UTxO by input reference
    pub fn get_utxo(&self, input: &TxInput) -> Option<&TxOutput> {
        self.utxo.get(input)
    }

    /// Check if a UTxO exists
    pub fn has_utxo(&self, input: &TxInput) -> bool {
        self.utxo.contains_key(input)
    }

    /// Get total UTxO count
    pub fn utxo_count(&self) -> usize {
        self.utxo.len()
    }

    /// Get total value locked in UTxO set (in lovelace)
    pub fn total_utxo_value(&self) -> u64 {
        self.utxo.values().map(|output| output.value).sum()
    }

    /// Insert a new UTxO
    pub fn insert_utxo(&mut self, input: TxInput, output: TxOutput) {
        self.utxo.insert(input, output);
    }

    /// Remove a UTxO (when spent)
    pub fn remove_utxo(&mut self, input: &TxInput) -> Option<TxOutput> {
        self.utxo.remove(input)
    }

    /// Apply a transaction to the UTxO set
    ///
    /// This consumes inputs and creates new outputs
    pub fn apply_transaction(
        &mut self,
        tx_id: Blake2b256Hash,
        inputs: &[TxInput],
        outputs: Vec<TxOutput>,
    ) -> Result<(), String> {
        // Validate all inputs exist
        for input in inputs {
            if !self.has_utxo(input) {
                return Err(format!("Input not found in UTxO set: {:?}", input));
            }
        }

        // Remove consumed inputs
        for input in inputs {
            self.remove_utxo(input);
        }

        // Add new outputs
        for (index, output) in outputs.into_iter().enumerate() {
            let new_input = TxInput::new(tx_id, index as u32);
            self.insert_utxo(new_input, output);
        }

        Ok(())
    }

    /// Rollback a transaction (inverse of apply)
    ///
    /// This adds back the consumed inputs and removes the created outputs
    pub fn rollback_transaction(
        &mut self,
        tx_id: Blake2b256Hash,
        inputs: Vec<(TxInput, TxOutput)>,
        output_count: u32,
    ) {
        // Remove the outputs that were created
        for index in 0..output_count {
            let output_input = TxInput::new(tx_id, index);
            self.remove_utxo(&output_input);
        }

        // Add back the consumed inputs
        for (input, output) in inputs {
            self.insert_utxo(input, output);
        }
    }
}

impl Default for LedgerState {
    fn default() -> Self {
        Self::genesis()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_state_is_at_block_zero() {
        let state = LedgerState::genesis();
        assert_eq!(state.block_no, BlockNo(0));
        assert_eq!(state.slot, SlotNo(0));
        assert_eq!(state.epoch, EpochNo(0));
        assert!(state.is_genesis());
        assert_eq!(state.utxo_count(), 0);
    }

    #[test]
    fn ledger_state_serialization_roundtrip() {
        // Test with empty UTxO set (JSON works fine with empty HashMaps)
        let state = LedgerState {
            slot: SlotNo(12345),
            block_no: BlockNo(6789),
            epoch: EpochNo(123),
            utxo: HashMap::new(),
        };

        let bytes = serde_json::to_vec(&state).unwrap();
        let decoded: LedgerState = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded.slot, state.slot);
        assert_eq!(decoded.block_no, state.block_no);
        assert_eq!(decoded.epoch, state.epoch);
        assert_eq!(decoded.utxo_count(), 0);
    }

    #[test]
    fn utxo_basic_operations() {
        let mut state = LedgerState::genesis();

        let tx_id = Blake2b256Hash::new([1u8; 32]);
        let input = TxInput::new(tx_id, 0);
        let output = TxOutput::new(vec![0x01, 0x02, 0x03], 5_000_000);

        // Insert
        state.insert_utxo(input.clone(), output.clone());
        assert_eq!(state.utxo_count(), 1);
        assert!(state.has_utxo(&input));

        // Get
        let retrieved = state.get_utxo(&input).unwrap();
        assert_eq!(retrieved.value, 5_000_000);
        assert_eq!(retrieved.address, vec![0x01, 0x02, 0x03]);

        // Remove
        let removed = state.remove_utxo(&input).unwrap();
        assert_eq!(removed.value, 5_000_000);
        assert_eq!(state.utxo_count(), 0);
        assert!(!state.has_utxo(&input));
    }

    #[test]
    fn utxo_total_value_calculation() {
        let mut state = LedgerState::genesis();

        let tx_id = Blake2b256Hash::new([1u8; 32]);

        // Add multiple UTxOs
        state.insert_utxo(TxInput::new(tx_id, 0), TxOutput::new(vec![0x01], 1_000_000));
        state.insert_utxo(TxInput::new(tx_id, 1), TxOutput::new(vec![0x02], 2_000_000));
        state.insert_utxo(TxInput::new(tx_id, 2), TxOutput::new(vec![0x03], 3_000_000));

        assert_eq!(state.utxo_count(), 3);
        assert_eq!(state.total_utxo_value(), 6_000_000);
    }

    #[test]
    fn apply_transaction_success() {
        let mut state = LedgerState::genesis();

        // Create initial UTxO
        let initial_tx_id = Blake2b256Hash::new([1u8; 32]);
        let input1 = TxInput::new(initial_tx_id, 0);
        state.insert_utxo(input1.clone(), TxOutput::new(vec![0x01], 10_000_000));

        // Apply a transaction that spends input1 and creates two outputs
        let new_tx_id = Blake2b256Hash::new([2u8; 32]);
        let new_outputs = vec![
            TxOutput::new(vec![0x02], 4_000_000),
            TxOutput::new(vec![0x03], 5_000_000),
        ];

        let result = state.apply_transaction(new_tx_id, &[input1.clone()], new_outputs);
        assert!(result.is_ok());

        // Old input should be gone
        assert!(!state.has_utxo(&input1));

        // New outputs should exist
        assert!(state.has_utxo(&TxInput::new(new_tx_id, 0)));
        assert!(state.has_utxo(&TxInput::new(new_tx_id, 1)));
        assert_eq!(state.utxo_count(), 2);

        // Check values
        assert_eq!(
            state.get_utxo(&TxInput::new(new_tx_id, 0)).unwrap().value,
            4_000_000
        );
        assert_eq!(
            state.get_utxo(&TxInput::new(new_tx_id, 1)).unwrap().value,
            5_000_000
        );
    }

    #[test]
    fn apply_transaction_missing_input_fails() {
        let mut state = LedgerState::genesis();

        let tx_id = Blake2b256Hash::new([1u8; 32]);
        let non_existent_input = TxInput::new(Blake2b256Hash::new([99u8; 32]), 0);

        let result = state.apply_transaction(
            tx_id,
            &[non_existent_input],
            vec![TxOutput::new(vec![0x01], 1_000_000)],
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found in UTxO set"));
    }

    #[test]
    fn rollback_transaction() {
        let mut state = LedgerState::genesis();

        // Setup initial UTxO
        let initial_tx_id = Blake2b256Hash::new([1u8; 32]);
        let input1 = TxInput::new(initial_tx_id, 0);
        let output1 = TxOutput::new(vec![0x01], 10_000_000);
        state.insert_utxo(input1.clone(), output1.clone());

        // Apply transaction
        let new_tx_id = Blake2b256Hash::new([2u8; 32]);
        state
            .apply_transaction(
                new_tx_id,
                &[input1.clone()],
                vec![
                    TxOutput::new(vec![0x02], 6_000_000),
                    TxOutput::new(vec![0x03], 3_000_000),
                ],
            )
            .unwrap();

        assert_eq!(state.utxo_count(), 2);

        // Rollback the transaction
        state.rollback_transaction(new_tx_id, vec![(input1.clone(), output1.clone())], 2);

        // Should be back to initial state
        assert_eq!(state.utxo_count(), 1);
        assert!(state.has_utxo(&input1));
        assert_eq!(state.get_utxo(&input1).unwrap().value, 10_000_000);
        assert!(!state.has_utxo(&TxInput::new(new_tx_id, 0)));
        assert!(!state.has_utxo(&TxInput::new(new_tx_id, 1)));
    }

    #[test]
    fn tx_output_with_datum_hash() {
        let address = vec![0x01, 0x02, 0x03];
        let value = 5_000_000;
        let datum_hash = Blake2b256Hash::new([0xABu8; 32]);

        let output = TxOutput::with_datum_hash(address.clone(), value, datum_hash);

        assert_eq!(output.address, address);
        assert_eq!(output.value, value);
        assert_eq!(output.datum_hash, Some(datum_hash));
        assert!(output.inline_datum.is_none());
        assert!(output.script_ref.is_none());
    }

    #[test]
    fn multiple_transactions_in_sequence() {
        let mut state = LedgerState::genesis();

        // TX 1: Genesis creates outputs
        let tx1_id = Blake2b256Hash::new([1u8; 32]);
        state.insert_utxo(
            TxInput::new(tx1_id, 0),
            TxOutput::new(vec![0x01], 100_000_000),
        );
        state.insert_utxo(
            TxInput::new(tx1_id, 1),
            TxOutput::new(vec![0x02], 50_000_000),
        );

        assert_eq!(state.utxo_count(), 2);
        assert_eq!(state.total_utxo_value(), 150_000_000);

        // TX 2: Spend first output
        let tx2_id = Blake2b256Hash::new([2u8; 32]);
        state
            .apply_transaction(
                tx2_id,
                &[TxInput::new(tx1_id, 0)],
                vec![
                    TxOutput::new(vec![0x03], 70_000_000),
                    TxOutput::new(vec![0x04], 29_000_000),
                ],
            )
            .unwrap();

        assert_eq!(state.utxo_count(), 3); // Removed 1, added 2, still have 1 from TX1
        assert_eq!(state.total_utxo_value(), 149_000_000); // Lost 1M to fees

        // TX 3: Spend two outputs
        let tx3_id = Blake2b256Hash::new([3u8; 32]);
        state
            .apply_transaction(
                tx3_id,
                &[TxInput::new(tx1_id, 1), TxInput::new(tx2_id, 1)],
                vec![TxOutput::new(vec![0x05], 78_000_000)],
            )
            .unwrap();

        assert_eq!(state.utxo_count(), 2); // Removed 2, added 1
        assert_eq!(state.total_utxo_value(), 148_000_000); // Lost 1M to fees
    }
}
