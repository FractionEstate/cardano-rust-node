//! Byron Era Ledger Implementation
//!
//! The Byron era represents the initial implementation of the Cardano blockchain,
//! featuring a simple UTXO model, basic transaction validation, and hierarchical
//! deterministic (HD) wallet addressing scheme.

use crate::{LedgerError, Result};
use cardano_crypto::{Blake2b256Hash, Ed25519KeyHash, Ed25519Signature};
use std::collections::HashMap;

/// Byron era transaction
#[derive(Debug, Clone)]
pub struct ByronTransaction {
    pub inputs: Vec<ByronTxIn>,
    pub outputs: Vec<ByronTxOut>,
    pub attributes: ByronAttributes,
}

/// Byron transaction input
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ByronTxIn {
    pub tx_id: Blake2b256Hash,
    pub output_index: u32,
}

/// Byron transaction output
#[derive(Debug, Clone)]
pub struct ByronTxOut {
    pub address: ByronAddress,
    pub value: ByronValue,
}

/// Byron address (hierarchical deterministic)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ByronAddress {
    pub address_id: Blake2b256Hash,
    pub address_attributes: ByronAddressAttributes,
    pub address_type: ByronAddressType,
}

/// Byron address attributes (encrypted derivation path)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ByronAddressAttributes {
    pub derivation_path: Option<Vec<u8>>, // Encrypted derivation path
    pub network_magic: Option<u32>,       // Network discriminant
}

/// Byron address types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ByronAddressType {
    PublicKey(Ed25519KeyHash), // Single key address
    Script(Blake2b256Hash),    // Script address (limited in Byron)
    Redeem(Ed25519KeyHash),    // Redemption address (for bootstrap era)
}

/// Byron value (simple ADA amounts)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByronValue {
    pub coin: u64, // ADA in lovelace (1 ADA = 1,000,000 lovelace)
}

/// Byron transaction attributes (metadata)
#[derive(Debug, Clone)]
pub struct ByronAttributes {
    pub metadata: Option<Vec<u8>>, // Optional encrypted metadata
}

/// Byron witness (transaction signatures)
#[derive(Debug, Clone)]
pub struct ByronWitness {
    pub vkey_witnesses: Vec<ByronVKeyWitness>,
    pub script_witnesses: Vec<ByronScriptWitness>,
    pub redeem_witnesses: Vec<ByronRedeemWitness>,
}

/// Byron verification key witness
#[derive(Debug, Clone)]
pub struct ByronVKeyWitness {
    pub vkey: Ed25519KeyHash,
    pub signature: Ed25519Signature,
}

/// Byron script witness (minimal scripting in Byron)
#[derive(Debug, Clone)]
pub struct ByronScriptWitness {
    pub validator: Blake2b256Hash, // Script hash
    pub redeemer: Blake2b256Hash,  // Simple redeemer
}

/// Byron redemption witness (for bootstrap/genesis funds)
#[derive(Debug, Clone)]
pub struct ByronRedeemWitness {
    pub redeem_key: Ed25519KeyHash,
    pub signature: Ed25519Signature,
}

/// Byron UTXO set
#[derive(Debug, Clone)]
pub struct ByronUtxo {
    pub utxo_map: HashMap<ByronTxIn, ByronTxOut>,
}

/// Byron ledger state
#[derive(Debug, Clone)]
pub struct ByronLedgerState {
    pub utxo: ByronUtxo,
    pub current_slot: u64,
    pub network_magic: u32,
    pub total_supply: ByronValue,
    pub genesis_hash: Blake2b256Hash,
}

/// Byron block
#[derive(Debug, Clone)]
pub struct ByronBlock {
    pub header: ByronBlockHeader,
    pub body: ByronBlockBody,
}

/// Byron block header
#[derive(Debug, Clone)]
pub struct ByronBlockHeader {
    pub prev_hash: Blake2b256Hash,
    pub body_proof: ByronBodyProof,
    pub consensus_data: ByronConsensusData,
    pub extra_data: ByronExtraData,
}

/// Byron body proof (Merkle tree root)
#[derive(Debug, Clone)]
pub struct ByronBodyProof {
    pub tx_proof: Blake2b256Hash,  // Merkle root of transactions
    pub ssc_proof: Blake2b256Hash, // Shared Seed Computation proof
    pub dlg_proof: Blake2b256Hash, // Delegation proof
    pub upd_proof: Blake2b256Hash, // Update proposal proof
}

/// Byron consensus data
#[derive(Debug, Clone)]
pub struct ByronConsensusData {
    pub slot_id: ByronSlotId,
    pub leader_key: Ed25519KeyHash,
    pub body_signature: Ed25519Signature,
}

/// Byron slot identifier
#[derive(Debug, Clone)]
pub struct ByronSlotId {
    pub epoch: u64,
    pub slot: u16, // Slot within epoch (0-21599 for Byron)
}

/// Byron extra data (protocol version, software version)
#[derive(Debug, Clone)]
pub struct ByronExtraData {
    pub protocol_version: ByronProtocolVersion,
    pub software_version: ByronSoftwareVersion,
    pub attributes: ByronAttributes,
}

/// Byron protocol version
#[derive(Debug, Clone)]
pub struct ByronProtocolVersion {
    pub major: u16,
    pub minor: u16,
    pub alt: u8,
}

/// Byron software version
#[derive(Debug, Clone)]
pub struct ByronSoftwareVersion {
    pub application_name: String,
    pub application_version: u32,
}

/// Byron block body
#[derive(Debug, Clone)]
pub struct ByronBlockBody {
    pub tx_payload: Vec<ByronTxAux>,
    pub ssc_payload: ByronSscPayload,
    pub dlg_payload: Vec<ByronProxyCert>,
    pub upd_payload: ByronUpdatePayload,
}

/// Byron transaction auxiliary data
#[derive(Debug, Clone)]
pub struct ByronTxAux {
    pub tx: ByronTransaction,
    pub witness: ByronWitness,
}

/// Byron Shared Secret Computation payload (for randomness)
#[derive(Debug, Clone)]
pub struct ByronSscPayload {
    // Simplified - Byron SSC is complex
    pub commitments: Vec<Blake2b256Hash>,
    pub openings: Vec<Blake2b256Hash>,
    pub shares: Vec<Blake2b256Hash>,
    pub certificates: Vec<Blake2b256Hash>,
}

/// Byron delegation certificate (proxy certificate)
#[derive(Debug, Clone)]
pub struct ByronProxyCert {
    pub issuer: Ed25519KeyHash,
    pub delegate: Ed25519KeyHash,
    pub signature: Ed25519Signature,
}

/// Byron update payload (protocol updates)
#[derive(Debug, Clone)]
pub struct ByronUpdatePayload {
    pub proposal: Option<ByronUpdateProposal>,
    pub votes: Vec<ByronUpdateVote>,
}

/// Byron update proposal
#[derive(Debug, Clone)]
pub struct ByronUpdateProposal {
    pub protocol_version: ByronProtocolVersion,
    pub protocol_parameters: ByronProtocolParameters,
    pub software_version: ByronSoftwareVersion,
}

/// Byron protocol parameters
#[derive(Debug, Clone)]
pub struct ByronProtocolParameters {
    pub slot_duration: u64,              // Slot duration in milliseconds
    pub security_parameter: u64,         // Security parameter k
    pub max_block_size: u64,             // Maximum block size
    pub max_tx_size: u64,                // Maximum transaction size
    pub max_proposal_size: u64,          // Maximum update proposal size
    pub mpc_threshold: f64,              // MPC threshold
    pub heavy_del_threshold: f64,        // Heavy delegation threshold
    pub update_vote_threshold: f64,      // Update vote threshold
    pub update_proposal_threshold: f64,  // Update proposal threshold
    pub unlock_stake_epoch: u64,         // Epoch when stake unlocks
    pub tx_fee_policy: ByronTxFeePolicy, // Transaction fee policy
}

/// Byron transaction fee policy
#[derive(Debug, Clone)]
pub struct ByronTxFeePolicy {
    pub summand: u64,    // Base fee
    pub multiplier: f64, // Fee per byte multiplier
}

/// Byron update vote
#[derive(Debug, Clone)]
pub struct ByronUpdateVote {
    pub voter: Ed25519KeyHash,
    pub proposal_id: Blake2b256Hash,
    pub decision: bool, // True for accept, false for reject
    pub signature: Ed25519Signature,
}

impl ByronValue {
    /// Create new Byron value
    pub fn new(coin: u64) -> Self {
        Self { coin }
    }

    /// Zero value
    pub fn zero() -> Self {
        Self { coin: 0 }
    }

    /// Add two values
    pub fn add(&self, other: &ByronValue) -> Result<ByronValue> {
        self.coin
            .checked_add(other.coin)
            .map(|coin| ByronValue { coin })
            .ok_or_else(|| LedgerError::ValueOverflow("Addition overflow".to_string()))
    }

    /// Subtract values
    pub fn subtract(&self, other: &ByronValue) -> Result<ByronValue> {
        self.coin
            .checked_sub(other.coin)
            .map(|coin| ByronValue { coin })
            .ok_or_else(|| LedgerError::ValueUnderflow("Subtraction underflow".to_string()))
    }

    /// Check if value is zero
    pub fn is_zero(&self) -> bool {
        self.coin == 0
    }
}

impl ByronAddress {
    /// Create new public key address
    pub fn new_pubkey(key_hash: Ed25519KeyHash, network_magic: Option<u32>) -> Self {
        let attributes = ByronAddressAttributes {
            derivation_path: None,
            network_magic,
        };

        let address_type = ByronAddressType::PublicKey(key_hash);
        let address_id = Self::compute_address_id(&address_type, &attributes);

        Self {
            address_id,
            address_attributes: attributes,
            address_type,
        }
    }

    /// Compute address ID from type and attributes
    fn compute_address_id(
        address_type: &ByronAddressType,
        attributes: &ByronAddressAttributes,
    ) -> Blake2b256Hash {
        let data = format!("{:?}{:?}", address_type, attributes);
        Blake2b256Hash::hash(data.as_bytes())
    }

    /// Verify address format and checksum
    pub fn verify(&self) -> bool {
        let computed_id = Self::compute_address_id(&self.address_type, &self.address_attributes);
        self.address_id == computed_id
    }
}

impl ByronTransaction {
    /// Calculate transaction ID (hash)
    pub fn tx_id(&self) -> Blake2b256Hash {
        let tx_data = format!("{:?}", self);
        Blake2b256Hash::hash(tx_data.as_bytes())
    }

    /// Validate transaction structure
    pub fn validate(&self) -> Result<()> {
        // Check inputs
        if self.inputs.is_empty() {
            return Err(LedgerError::InvalidTransaction(
                "Transaction has no inputs".to_string(),
            ));
        }

        // Check outputs
        if self.outputs.is_empty() {
            return Err(LedgerError::InvalidTransaction(
                "Transaction has no outputs".to_string(),
            ));
        }

        // Check for duplicate inputs
        let mut seen_inputs = std::collections::HashSet::new();
        for input in &self.inputs {
            if !seen_inputs.insert(input) {
                return Err(LedgerError::InvalidTransaction(
                    "Duplicate input".to_string(),
                ));
            }
        }

        // Validate output values
        for output in &self.outputs {
            if output.value.is_zero() {
                return Err(LedgerError::InvalidTransaction(
                    "Output has zero value".to_string(),
                ));
            }

            if !output.address.verify() {
                return Err(LedgerError::InvalidTransaction(
                    "Invalid output address".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Calculate total input value (requires UTxO context)
    pub fn total_input_value(&self, utxo: &ByronUtxo) -> Result<ByronValue> {
        let mut total = ByronValue::zero();

        for input in &self.inputs {
            let output = utxo.utxo_map.get(input).ok_or_else(|| {
                LedgerError::InvalidInput("Input not found in UTxO set".to_string())
            })?;
            total = total.add(&output.value)?;
        }

        Ok(total)
    }

    /// Calculate total output value
    pub fn total_output_value(&self) -> Result<ByronValue> {
        let mut total = ByronValue::zero();

        for output in &self.outputs {
            total = total.add(&output.value)?;
        }

        Ok(total)
    }

    /// Calculate transaction fee
    pub fn fee(&self, utxo: &ByronUtxo) -> Result<ByronValue> {
        let input_value = self.total_input_value(utxo)?;
        let output_value = self.total_output_value()?;
        input_value.subtract(&output_value)
    }
}

impl Default for ByronUtxo {
    fn default() -> Self {
        Self::new()
    }
}

impl ByronUtxo {
    /// Create new empty UTxO set
    pub fn new() -> Self {
        Self {
            utxo_map: HashMap::new(),
        }
    }

    /// Add output to UTxO set
    pub fn add_output(&mut self, tx_in: ByronTxIn, tx_out: ByronTxOut) {
        self.utxo_map.insert(tx_in, tx_out);
    }

    /// Remove output from UTxO set
    pub fn remove_output(&mut self, tx_in: &ByronTxIn) -> Option<ByronTxOut> {
        self.utxo_map.remove(tx_in)
    }

    /// Check if output exists
    pub fn contains(&self, tx_in: &ByronTxIn) -> bool {
        self.utxo_map.contains_key(tx_in)
    }

    /// Get output
    pub fn get(&self, tx_in: &ByronTxIn) -> Option<&ByronTxOut> {
        self.utxo_map.get(tx_in)
    }

    /// Apply transaction to UTxO set
    pub fn apply_transaction(&mut self, tx: &ByronTransaction) -> Result<()> {
        // Validate transaction first
        tx.validate()?;

        // Check all inputs exist
        for input in &tx.inputs {
            if !self.contains(input) {
                return Err(LedgerError::InvalidInput(format!(
                    "Input {:?} not found in UTxO set",
                    input.tx_id
                )));
            }
        }

        // Remove consumed inputs
        for input in &tx.inputs {
            self.remove_output(input);
        }

        // Add new outputs
        let tx_id = tx.tx_id();
        for (index, output) in tx.outputs.iter().enumerate() {
            let new_input = ByronTxIn {
                tx_id,
                output_index: index as u32,
            };
            self.add_output(new_input, output.clone());
        }

        Ok(())
    }

    /// Calculate total value in UTxO set
    pub fn total_value(&self) -> Result<ByronValue> {
        let mut total = ByronValue::zero();

        for output in self.utxo_map.values() {
            total = total.add(&output.value)?;
        }

        Ok(total)
    }
}

impl ByronLedgerState {
    /// Create new Byron ledger state
    pub fn new(genesis_hash: Blake2b256Hash, network_magic: u32) -> Self {
        Self {
            utxo: ByronUtxo::new(),
            current_slot: 0,
            network_magic,
            total_supply: ByronValue::new(31_112_484_646_000_000), // Initial Byron supply
            genesis_hash,
        }
    }

    /// Apply transaction to ledger state
    pub fn apply_transaction(&mut self, tx: &ByronTransaction) -> Result<()> {
        // Validate transaction
        tx.validate()?;

        // Check balance (inputs >= outputs)
        let input_value = tx.total_input_value(&self.utxo)?;
        let output_value = tx.total_output_value()?;

        if input_value.coin < output_value.coin {
            return Err(LedgerError::InsufficientFunds(
                "Transaction outputs exceed inputs".to_string(),
            ));
        }

        // Apply to UTxO set
        self.utxo.apply_transaction(tx)?;

        Ok(())
    }

    /// Apply block to ledger state
    pub fn apply_block(&mut self, block: &ByronBlock) -> Result<()> {
        // Validate block structure
        self.validate_block(block)?;

        // Apply each transaction
        for tx_aux in &block.body.tx_payload {
            self.apply_transaction(&tx_aux.tx)?;
        }

        // Update current slot
        let slot_id = &block.header.consensus_data.slot_id;
        self.current_slot = slot_id.epoch * 21600 + slot_id.slot as u64;

        Ok(())
    }

    /// Validate Byron block
    fn validate_block(&self, block: &ByronBlock) -> Result<()> {
        // Check previous hash (simplified - would check against actual chain)
        if block.header.prev_hash.as_bytes().is_empty() {
            return Err(LedgerError::InvalidBlock("Empty previous hash".to_string()));
        }

        // Validate slot progression
        let slot_id = &block.header.consensus_data.slot_id;
        let block_slot = slot_id.epoch * 21600 + slot_id.slot as u64;

        if block_slot <= self.current_slot {
            return Err(LedgerError::InvalidSlot(
                "Block slot not greater than current".to_string(),
            ));
        }

        // Validate transactions
        for tx_aux in &block.body.tx_payload {
            tx_aux.tx.validate()?;
        }

        Ok(())
    }
}

impl ByronSlotId {
    /// Create new slot ID
    pub fn new(epoch: u64, slot: u16) -> Result<Self> {
        if slot >= 21600 {
            // Byron has 21600 slots per epoch (20 second slots)
            return Err(LedgerError::InvalidSlot(
                "Slot exceeds epoch boundary".to_string(),
            ));
        }

        Ok(Self { epoch, slot })
    }

    /// Convert to absolute slot number
    pub fn to_absolute_slot(&self) -> u64 {
        self.epoch * 21600 + self.slot as u64
    }
}

impl ByronProtocolParameters {
    /// Default Byron protocol parameters
    pub fn mainnet() -> Self {
        Self {
            slot_duration: 20000,           // 20 seconds
            security_parameter: 2160,       // k = 2160
            max_block_size: 2097152,        // 2MB
            max_tx_size: 4096,              // 4KB
            max_proposal_size: 700,         // 700 bytes
            mpc_threshold: 0.5,             // 50%
            heavy_del_threshold: 0.005,     // 0.5%
            update_vote_threshold: 0.6,     // 60%
            update_proposal_threshold: 0.1, // 10%
            unlock_stake_epoch: 0,          // Immediate
            tx_fee_policy: ByronTxFeePolicy {
                summand: 155381,    // Base fee in lovelace
                multiplier: 43.946, // Fee per byte
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byron_value_operations() {
        let v1 = ByronValue::new(1000000); // 1 ADA
        let v2 = ByronValue::new(500000); // 0.5 ADA

        let sum = v1.add(&v2).unwrap();
        assert_eq!(sum.coin, 1500000);

        let diff = v1.subtract(&v2).unwrap();
        assert_eq!(diff.coin, 500000);

        assert!(!v1.is_zero());
        assert!(ByronValue::zero().is_zero());
    }

    #[test]
    fn test_byron_value_overflow() {
        let v1 = ByronValue::new(u64::MAX);
        let v2 = ByronValue::new(1);

        assert!(v1.add(&v2).is_err());
    }

    #[test]
    fn test_byron_value_underflow() {
        let v1 = ByronValue::new(100);
        let v2 = ByronValue::new(200);

        assert!(v1.subtract(&v2).is_err());
    }

    #[test]
    fn test_byron_address_creation() {
        let key_hash = Ed25519KeyHash::from_test_data(b"test_key_hash");
        let address = ByronAddress::new_pubkey(key_hash, Some(764824073));

        assert!(address.verify());
        assert_eq!(address.address_attributes.network_magic, Some(764824073));

        if let ByronAddressType::PublicKey(addr_key) = address.address_type {
            assert_eq!(addr_key, key_hash);
        } else {
            panic!("Expected PublicKey address type");
        }
    }

    #[test]
    fn test_byron_transaction_validation() {
        let input = ByronTxIn {
            tx_id: Blake2b256Hash::hash(b"input_tx"),
            output_index: 0,
        };

        let output = ByronTxOut {
            address: ByronAddress::new_pubkey(Ed25519KeyHash::from_test_data(b"output_key"), None),
            value: ByronValue::new(1000000),
        };

        let tx = ByronTransaction {
            inputs: vec![input],
            outputs: vec![output],
            attributes: ByronAttributes { metadata: None },
        };

        assert!(tx.validate().is_ok());
    }

    #[test]
    fn test_invalid_byron_transaction() {
        // Transaction with no inputs
        let tx_no_inputs = ByronTransaction {
            inputs: vec![],
            outputs: vec![ByronTxOut {
                address: ByronAddress::new_pubkey(Ed25519KeyHash::from_test_data(b"key"), None),
                value: ByronValue::new(1000000),
            }],
            attributes: ByronAttributes { metadata: None },
        };

        assert!(tx_no_inputs.validate().is_err());

        // Transaction with no outputs
        let tx_no_outputs = ByronTransaction {
            inputs: vec![ByronTxIn {
                tx_id: Blake2b256Hash::hash(b"input"),
                output_index: 0,
            }],
            outputs: vec![],
            attributes: ByronAttributes { metadata: None },
        };

        assert!(tx_no_outputs.validate().is_err());
    }

    #[test]
    fn test_byron_utxo_operations() {
        let mut utxo = ByronUtxo::new();

        let tx_in = ByronTxIn {
            tx_id: Blake2b256Hash::hash(b"test_tx"),
            output_index: 0,
        };

        let tx_out = ByronTxOut {
            address: ByronAddress::new_pubkey(Ed25519KeyHash::from_test_data(b"test_key"), None),
            value: ByronValue::new(1000000),
        };

        // Add output
        utxo.add_output(tx_in.clone(), tx_out.clone());
        assert!(utxo.contains(&tx_in));
        assert_eq!(utxo.get(&tx_in).unwrap().value.coin, 1000000);

        // Remove output
        let removed = utxo.remove_output(&tx_in);
        assert!(removed.is_some());
        assert!(!utxo.contains(&tx_in));
    }

    #[test]
    fn test_byron_slot_id() {
        let slot_id = ByronSlotId::new(1, 1000).unwrap();
        assert_eq!(slot_id.to_absolute_slot(), 21600 + 1000);

        // Invalid slot (>= 21600)
        assert!(ByronSlotId::new(1, 21600).is_err());
    }

    #[test]
    fn test_byron_ledger_state() {
        let genesis_hash = Blake2b256Hash::hash(b"genesis");
        let mut ledger = ByronLedgerState::new(genesis_hash, 764824073);

        // Create test UTxO
        let tx_in = ByronTxIn {
            tx_id: Blake2b256Hash::hash(b"genesis_tx"),
            output_index: 0,
        };
        let tx_out = ByronTxOut {
            address: ByronAddress::new_pubkey(
                Ed25519KeyHash::from_test_data(b"genesis_key"),
                Some(764824073),
            ),
            value: ByronValue::new(5000000), // 5 ADA
        };
        ledger.utxo.add_output(tx_in.clone(), tx_out);

        // Create valid transaction
        let new_output = ByronTxOut {
            address: ByronAddress::new_pubkey(
                Ed25519KeyHash::from_test_data(b"new_key"),
                Some(764824073),
            ),
            value: ByronValue::new(4000000), // 4 ADA (1 ADA fee)
        };

        let tx = ByronTransaction {
            inputs: vec![tx_in],
            outputs: vec![new_output],
            attributes: ByronAttributes { metadata: None },
        };

        let result = ledger.apply_transaction(&tx);
        assert!(
            result.is_ok(),
            "Transaction application failed: {:?}",
            result.err()
        );

        // UTxO should now contain the new output
        let new_tx_id = tx.tx_id();
        let new_tx_in = ByronTxIn {
            tx_id: new_tx_id,
            output_index: 0,
        };
        assert!(ledger.utxo.contains(&new_tx_in));
    }

    #[test]
    fn test_insufficient_funds() {
        let genesis_hash = Blake2b256Hash::hash(b"genesis");
        let mut ledger = ByronLedgerState::new(genesis_hash, 764824073);

        // Create test UTxO with small amount
        let tx_in = ByronTxIn {
            tx_id: Blake2b256Hash::hash(b"genesis_tx"),
            output_index: 0,
        };
        let tx_out = ByronTxOut {
            address: ByronAddress::new_pubkey(
                Ed25519KeyHash::from_test_data(b"genesis_key"),
                Some(764824073),
            ),
            value: ByronValue::new(1000000), // 1 ADA
        };
        ledger.utxo.add_output(tx_in.clone(), tx_out);

        // Try to spend more than available
        let large_output = ByronTxOut {
            address: ByronAddress::new_pubkey(
                Ed25519KeyHash::from_test_data(b"new_key"),
                Some(764824073),
            ),
            value: ByronValue::new(2000000), // 2 ADA (more than input)
        };

        let tx = ByronTransaction {
            inputs: vec![tx_in],
            outputs: vec![large_output],
            attributes: ByronAttributes { metadata: None },
        };

        let result = ledger.apply_transaction(&tx);
        assert!(result.is_err());
    }

    #[test]
    fn test_protocol_parameters() {
        let params = ByronProtocolParameters::mainnet();

        assert_eq!(params.slot_duration, 20000); // 20 seconds
        assert_eq!(params.security_parameter, 2160); // k = 2160
        assert_eq!(params.max_block_size, 2097152); // 2MB
        assert_eq!(params.tx_fee_policy.summand, 155381);
        assert!((params.tx_fee_policy.multiplier - 43.946).abs() < f64::EPSILON);
    }
}
