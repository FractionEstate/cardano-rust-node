//! Block Validation Pipeline Tests
//!
//! Tests for the comprehensive block validation pipeline in the Ouroboros consensus protocol.
//! Covers header validation, body validation, transaction validation, witness verification,
//! ledger state transitions, and protocol compliance checks.

use cardano_consensus::{ConsensusError, Result};
use cardano_crypto::{
    Blake2b256Hash,
    Ed25519KeyHash,
    VrfOutput,
    VrfProof,
    VRF_OUTPUT_LENGTH,
};
use std::collections::HashMap;

#[path = "../common/mod.rs"]
mod common;

use common::vrf::{vrf_prove_message, vrf_public_key};

pub use crate::test_ouroboros_protocol::{
    BlockHeader, SlotNumber, EpochNumber, ConsensusState, StakeDistribution,
    OperationalCertificate
};
pub use crate::test_block_production::{
    BlockBody, Transaction, TxInput, TxOutput, ForgedBlock
};

/// Complete block validation pipeline
#[derive(Debug)]
pub struct ValidationPipeline {
    pub consensus_state: ConsensusState,
    pub ledger_state: ValidationLedgerState,
    pub validation_config: ValidationConfig,
}

/// Extended ledger state for validation
#[derive(Debug, Clone)]
pub struct ValidationLedgerState {
    pub utxo_set: HashMap<TxInput, TxOutput>,
    pub stake_distribution: StakeDistribution,
    pub protocol_parameters: ProtocolParameters,
    pub current_epoch: EpochNumber,
    pub epoch_boundary_slot: SlotNumber,
    pub treasury: u64,
    pub reserves: u64,
    pub total_supply: u64,
}

/// Protocol parameters for validation
#[derive(Debug, Clone)]
pub struct ProtocolParameters {
    pub min_fee_a: u64,           // Linear fee coefficient
    pub min_fee_b: u64,           // Constant fee coefficient
    pub max_block_size: u32,      // Maximum block body size
    pub max_tx_size: u32,         // Maximum transaction size
    pub max_block_header_size: u32, // Maximum block header size
    pub key_deposit: u64,         // Stake key deposit
    pub pool_deposit: u64,        // Pool registration deposit
    pub min_utxo_value: u64,      // Minimum value per UTxO
    pub utxo_cost_per_word: u64,  // Cost per word for UTxO
    pub treasury_cut: f64,        // Treasury cut from rewards
    pub monetary_expand_rate: f64, // Monetary expansion rate
    pub pool_pledge_influence: f64, // Pool pledge influence factor
    pub pool_retirement_max_epoch: u64, // Max epochs for pool retirement
    pub desired_number_of_pools: u32, // Desired number of stake pools
    pub pool_influence: f64,      // Pool influence parameter
}

/// Validation configuration
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    pub validate_signatures: bool,     // Enable signature validation
    pub validate_vrf_proofs: bool,     // Enable VRF proof validation
    pub validate_kes_signatures: bool, // Enable KES signature validation
    pub validate_transactions: bool,   // Enable transaction validation
    pub validate_ledger_rules: bool,   // Enable ledger rule validation
    pub max_validation_time_ms: u64,   // Maximum validation time
    pub parallel_tx_validation: bool,  // Enable parallel transaction validation
}

/// Validation result with detailed information
#[derive(Debug)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub validation_time_ms: u64,
    pub new_ledger_state: Option<ValidationLedgerState>,
}

/// Validation error types
#[derive(Debug, Clone)]
pub enum ValidationError {
    HeaderValidation(String),
    BodyValidation(String),
    TransactionValidation(String),
    SignatureValidation(String),
    VrfValidation(String),
    KesValidation(String),
    LedgerRuleValidation(String),
    ProtocolValidation(String),
}

/// Validation warnings (non-fatal issues)
#[derive(Debug, Clone)]
pub enum ValidationWarning {
    SuboptimalFee(String),
    LargeTransaction(String),
    HighMemoryUsage(String),
    SlowValidation(String),
}

/// Transaction witness for validation
#[derive(Debug, Clone)]
pub struct TransactionWitness {
    pub vkey_witnesses: Vec<VKeyWitness>,
    pub native_scripts: Vec<NativeScript>,
    pub plutus_scripts: Vec<PlutusScript>,
    pub plutus_data: Vec<PlutusData>,
    pub redeemers: Vec<Redeemer>,
}

/// Key witness (signature)
#[derive(Debug, Clone)]
pub struct VKeyWitness {
    pub vkey: Ed25519KeyHash,
    pub signature: Blake2b256Hash,
}

/// Native script (multisig, timelock, etc.)
#[derive(Debug, Clone)]
pub enum NativeScript {
    RequireSignature(Ed25519KeyHash),
    RequireAllOf(Vec<NativeScript>),
    RequireAnyOf(Vec<NativeScript>),
    RequireNOf(u32, Vec<NativeScript>),
    RequireTimeBefore(SlotNumber),
    RequireTimeAfter(SlotNumber),
}

/// Plutus script
#[derive(Debug, Clone)]
pub struct PlutusScript {
    pub version: PlutusVersion,
    pub code: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum PlutusVersion {
    V1,
    V2,
}

/// Plutus data (CBOR-encoded)
#[derive(Debug, Clone)]
pub struct PlutusData {
    pub data: Vec<u8>,
}

/// Redeemer for script execution
#[derive(Debug, Clone)]
pub struct Redeemer {
    pub tag: RedeemerTag,
    pub index: u32,
    pub data: PlutusData,
    pub ex_units: ExUnits,
}

#[derive(Debug, Clone)]
pub enum RedeemerTag {
    Spend,
    Mint,
    Cert,
    Reward,
}

/// Execution units for Plutus scripts
#[derive(Debug, Clone)]
pub struct ExUnits {
    pub memory: u64,
    pub steps: u64,
}

impl ValidationPipeline {
    pub fn new(consensus_state: ConsensusState, ledger_state: ValidationLedgerState) -> Self {
        Self {
            consensus_state,
            ledger_state,
            validation_config: ValidationConfig::default(),
        }
    }

    /// Validate a complete block through the full pipeline
    pub fn validate_block(&mut self, block: &ForgedBlock) -> Result<ValidationResult> {
        let start_time = std::time::Instant::now();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Phase 1: Header validation
        if let Err(e) = self.validate_header(&block.header) {
            errors.push(ValidationError::HeaderValidation(e.to_string()));
        }

        // Phase 2: Body validation
        if let Err(e) = self.validate_body(&block.body) {
            errors.push(ValidationError::BodyValidation(e.to_string()));
        }

        // Phase 3: Transaction validation
        for (i, tx) in block.body.transactions.iter().enumerate() {
            if let Err(e) = self.validate_transaction(tx, i) {
                errors.push(ValidationError::TransactionValidation(
                    format!("Transaction {}: {}", i, e)
                ));
            }
        }

        // Phase 4: Cryptographic validation
        if self.validation_config.validate_vrf_proofs {
            if let Err(e) = self.validate_vrf_proof(&block.header, &block.proof_of_leadership) {
                errors.push(ValidationError::VrfValidation(e.to_string()));
            }
        }

        // Phase 5: Ledger state transition
        let new_ledger_state = if errors.is_empty() {
            match self.apply_block_to_ledger(&block.body) {
                Ok(state) => Some(state),
                Err(e) => {
                    errors.push(ValidationError::LedgerRuleValidation(e.to_string()));
                    None
                }
            }
        } else {
            None
        };

        let validation_time = start_time.elapsed().as_millis() as u64;

        // Check for warnings
        if validation_time > self.validation_config.max_validation_time_ms {
            warnings.push(ValidationWarning::SlowValidation(
                format!("Validation took {}ms", validation_time)
            ));
        }

        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            validation_time_ms: validation_time,
            new_ledger_state,
        })
    }

    /// Validate block header
    fn validate_header(&self, header: &BlockHeader) -> Result<()> {
        // Basic header structure validation
        if header.slot <= self.consensus_state.current_slot {
            return Err(ConsensusError::InvalidSlot("Block from past or current slot".to_string()));
        }

        // Protocol magic validation
        if header.protocol_magic != 764824073 { // Mainnet magic
            return Err(ConsensusError::InvalidProtocolMagic("Wrong network".to_string()));
        }

        // Block size validation
        if header.block_size > self.ledger_state.protocol_parameters.max_block_size {
            return Err(ConsensusError::InvalidBlock("Block exceeds maximum size".to_string()));
        }

        // Previous hash validation (simplified - would check against chain)
        if header.prev_hash.as_bytes().is_empty() {
            return Err(ConsensusError::InvalidBlock("Empty previous hash".to_string()));
        }

        // Operational certificate validation
        self.validate_operational_certificate(&header.operational_cert, header.slot)?;

        // VRF output validation
        if header.vrf_output.as_bytes().len() != VRF_OUTPUT_LENGTH {
            return Err(ConsensusError::InvalidVrfProof("Invalid VRF output length".to_string()));
        }

        Ok(())
    }

    /// Validate operational certificate
    fn validate_operational_certificate(&self, cert: &OperationalCertificate, slot: SlotNumber) -> Result<()> {
        // KES period validation
        let expected_kes_period = slot / 129600; // ~36 hours per KES period
        if cert.kes_period != expected_kes_period {
            return Err(ConsensusError::InvalidOperationalCert("Wrong KES period".to_string()));
        }

        // Sequence number should be positive
        if cert.sequence_number == 0 {
            return Err(ConsensusError::InvalidOperationalCert("Invalid sequence number".to_string()));
        }

        // Cold key signature validation (simplified)
        if cert.sigma.as_bytes().is_empty() {
            return Err(ConsensusError::InvalidOperationalCert("Empty cold signature".to_string()));
        }

        Ok(())
    }

    /// Validate block body
    fn validate_body(&self, body: &BlockBody) -> Result<()> {
        // Size validation
        if body.total_size > self.ledger_state.protocol_parameters.max_block_size {
            return Err(ConsensusError::InvalidBlock("Body exceeds maximum size".to_string()));
        }

        // Transaction count validation (reasonable limit)
        if body.transactions.len() > 1000 {
            return Err(ConsensusError::InvalidBlock("Too many transactions in block".to_string()));
        }

        // Fee validation
        let calculated_fees: u64 = body.transactions.iter().map(|tx| tx.fee).sum();
        if calculated_fees != body.total_fee {
            return Err(ConsensusError::InvalidBlock("Fee mismatch in block body".to_string()));
        }

        // Size consistency
        let calculated_size: u32 = body.transactions.iter().map(|tx| tx.size).sum();
        if calculated_size != body.total_size {
            return Err(ConsensusError::InvalidBlock("Size mismatch in block body".to_string()));
        }

        Ok(())
    }

    /// Validate individual transaction
    fn validate_transaction(&self, tx: &Transaction, _index: usize) -> Result<()> {
        // Basic structure validation
        if tx.inputs.is_empty() {
            return Err(ConsensusError::InvalidTransaction("No inputs".to_string()));
        }

        if tx.outputs.is_empty() {
            return Err(ConsensusError::InvalidTransaction("No outputs".to_string()));
        }

        // Size validation
        if tx.size > self.ledger_state.protocol_parameters.max_tx_size {
            return Err(ConsensusError::InvalidTransaction("Transaction too large".to_string()));
        }

        // Fee validation
        let min_fee = self.calculate_minimum_fee(tx);
        if tx.fee < min_fee {
            return Err(ConsensusError::InvalidTransaction(
                format!("Fee {} below minimum {}", tx.fee, min_fee)
            ));
        }

        // UTxO validation
        for output in &tx.outputs {
            if output.value < self.ledger_state.protocol_parameters.min_utxo_value {
                return Err(ConsensusError::InvalidTransaction("Output below minimum UTxO value".to_string()));
            }
        }

        // Input/output balance (simplified - would need full UTxO context)
        self.validate_transaction_balance(tx)?;

        Ok(())
    }

    /// Calculate minimum fee for transaction
    fn calculate_minimum_fee(&self, tx: &Transaction) -> u64 {
        let params = &self.ledger_state.protocol_parameters;
        params.min_fee_a * tx.size as u64 + params.min_fee_b
    }

    /// Validate transaction balance
    fn validate_transaction_balance(&self, tx: &Transaction) -> Result<()> {
        // Calculate total output value
        let total_output: u64 = tx.outputs.iter().map(|o| o.value).sum();

        // For a complete validation, we'd need to look up input values from UTxO set
        // This is a simplified check to ensure outputs aren't zero
        if total_output == 0 {
            return Err(ConsensusError::InvalidTransaction("Zero total output value".to_string()));
        }

        // Check for overflow
        if total_output > 45_000_000_000_000_000 { // Max ADA supply
            return Err(ConsensusError::InvalidTransaction("Output value exceeds maximum supply".to_string()));
        }

        Ok(())
    }

    /// Validate VRF proof of leadership
    fn validate_vrf_proof(&self, header: &BlockHeader, proof: &VrfProof) -> Result<()> {
        // VRF proof structure validation
        if proof.as_bytes().is_empty() {
            return Err(ConsensusError::InvalidVrfProof("Empty VRF proof".to_string()));
        }

        if header.vrf_proof != *proof {
            return Err(ConsensusError::InvalidVrfProof(
                "Proof of leadership does not match header".to_string(),
            ));
        }

        // Check pool is in stake distribution
        if !self.ledger_state.stake_distribution.pools.contains_key(&header.issuer_vkey) {
            return Err(ConsensusError::PoolNotFound("Pool not in stake distribution".to_string()));
        }

        // Reconstruct VRF payload from epoch nonce and slot
        let mut payload = Vec::with_capacity(
            self.consensus_state.epoch_nonce.as_bytes().len() + std::mem::size_of::<SlotNumber>(),
        );
        payload.extend_from_slice(self.consensus_state.epoch_nonce.as_bytes());
        payload.extend_from_slice(&header.slot.to_le_bytes());

        // Verify VRF proof using public key (test harness uses golden key)
        let public_key = vrf_public_key();
        let is_valid = public_key
            .try_verify(&payload, &header.vrf_output, proof)
            .map_err(|err| ConsensusError::InvalidVrfProof(format!("VRF verification error: {}", err)))?;

        if !is_valid {
            return Err(ConsensusError::InvalidVrfProof("VRF proof verification failed".to_string()));
        }

        Ok(())
    }

    /// Apply block to ledger state
    fn apply_block_to_ledger(&self, body: &BlockBody) -> Result<ValidationLedgerState> {
        let mut new_state = self.ledger_state.clone();

        // Apply transactions to UTxO set
        for tx in &body.transactions {
            self.apply_transaction_to_utxo(&mut new_state, tx)?;
        }

        // Update treasury with fees
        new_state.treasury += (body.total_fee as f64 * new_state.protocol_parameters.treasury_cut) as u64;

        // Update reserves (simplified monetary policy)
        let epoch_reward = self.calculate_epoch_rewards(&new_state);
        if new_state.reserves >= epoch_reward {
            new_state.reserves -= epoch_reward;
            new_state.treasury += epoch_reward;
        }

        Ok(new_state)
    }

    /// Apply single transaction to UTxO set
    fn apply_transaction_to_utxo(&self, state: &mut ValidationLedgerState, tx: &Transaction) -> Result<()> {
        // Remove consumed inputs
        for input in &tx.inputs {
            if !state.utxo_set.contains_key(input) {
                return Err(ConsensusError::InvalidInput("Input not found in UTxO set".to_string()));
            }
            state.utxo_set.remove(input);
        }

        // Add new outputs
        for (i, output) in tx.outputs.iter().enumerate() {
            let new_input = TxInput {
                tx_hash: tx.tx_id,
                output_index: i as u32,
            };
            state.utxo_set.insert(new_input, output.clone());
        }

        Ok(())
    }

    /// Calculate epoch rewards (simplified)
    fn calculate_epoch_rewards(&self, state: &ValidationLedgerState) -> u64 {
        (state.reserves as f64 * state.protocol_parameters.monetary_expand_rate / 365.25) as u64
    }

    /// Validate witness data for transaction
    pub fn validate_transaction_witness(&self, tx: &Transaction, witness: &TransactionWitness) -> Result<()> {
        // Validate key witnesses (signatures)
        for vkey_witness in &witness.vkey_witnesses {
            self.validate_vkey_witness(tx, vkey_witness)?;
        }

        // Validate native scripts
        for script in &witness.native_scripts {
            self.validate_native_script(script)?;
        }

        // Validate Plutus scripts (simplified)
        for script in &witness.plutus_scripts {
            self.validate_plutus_script(script)?;
        }

        Ok(())
    }

    /// Validate key witness (signature)
    fn validate_vkey_witness(&self, _tx: &Transaction, witness: &VKeyWitness) -> Result<()> {
        // In real implementation, would verify signature against transaction hash
        if witness.signature.as_bytes().is_empty() {
            return Err(ConsensusError::InvalidSignature("Empty signature".to_string()));
        }

        if witness.vkey.as_ref().is_empty() {
            return Err(ConsensusError::InvalidSignature("Empty verification key".to_string()));
        }

        Ok(())
    }

    /// Validate native script
    fn validate_native_script(&self, script: &NativeScript) -> Result<()> {
        match script {
            NativeScript::RequireSignature(key_hash) => {
                if key_hash.as_ref().is_empty() {
                    return Err(ConsensusError::InvalidScript("Empty key hash in script".to_string()));
                }
            }

            NativeScript::RequireAllOf(scripts) => {
                if scripts.is_empty() {
                    return Err(ConsensusError::InvalidScript("Empty RequireAllOf script".to_string()));
                }
                for subscript in scripts {
                    self.validate_native_script(subscript)?;
                }
            }

            NativeScript::RequireAnyOf(scripts) => {
                if scripts.is_empty() {
                    return Err(ConsensusError::InvalidScript("Empty RequireAnyOf script".to_string()));
                }
                for subscript in scripts {
                    self.validate_native_script(subscript)?;
                }
            }

            NativeScript::RequireNOf(n, scripts) => {
                if *n as usize > scripts.len() {
                    return Err(ConsensusError::InvalidScript("RequireNOf n exceeds script count".to_string()));
                }
                if scripts.is_empty() {
                    return Err(ConsensusError::InvalidScript("Empty RequireNOf script".to_string()));
                }
                for subscript in scripts {
                    self.validate_native_script(subscript)?;
                }
            }

            NativeScript::RequireTimeBefore(slot) => {
                if *slot > 1_000_000_000 { // Reasonable upper bound
                    return Err(ConsensusError::InvalidScript("RequireTimeBefore slot too large".to_string()));
                }
            }

            NativeScript::RequireTimeAfter(slot) => {
                if *slot > 1_000_000_000 { // Reasonable upper bound
                    return Err(ConsensusError::InvalidScript("RequireTimeAfter slot too large".to_string()));
                }
            }
        }

        Ok(())
    }

    /// Validate Plutus script (simplified)
    fn validate_plutus_script(&self, script: &PlutusScript) -> Result<()> {
        if script.code.is_empty() {
            return Err(ConsensusError::InvalidScript("Empty Plutus script code".to_string()));
        }

        // Basic size check
        if script.code.len() > 16384 { // 16KB max script size
            return Err(ConsensusError::InvalidScript("Plutus script too large".to_string()));
        }

        // Version validation
        match script.version {
            PlutusVersion::V1 | PlutusVersion::V2 => {} // Valid versions
        }

        Ok(())
    }
}

impl ValidationConfig {
    pub fn default() -> Self {
        Self {
            validate_signatures: true,
            validate_vrf_proofs: true,
            validate_kes_signatures: true,
            validate_transactions: true,
            validate_ledger_rules: true,
            max_validation_time_ms: 5000, // 5 seconds
            parallel_tx_validation: false, // Disabled for testing
        }
    }

    /// Create config for fast validation (testing)
    pub fn fast_validation() -> Self {
        Self {
            validate_signatures: false,
            validate_vrf_proofs: false,
            validate_kes_signatures: false,
            validate_transactions: true,
            validate_ledger_rules: true,
            max_validation_time_ms: 1000, // 1 second
            parallel_tx_validation: true,
        }
    }
}

impl ProtocolParameters {
    pub fn mainnet() -> Self {
        Self {
            min_fee_a: 44,               // 44 lovelace per byte
            min_fee_b: 155381,           // 155381 lovelace base fee
            max_block_size: 90112,       // ~88KB
            max_tx_size: 16384,          // 16KB
            max_block_header_size: 1100, // 1.1KB
            key_deposit: 2_000_000,      // 2 ADA
            pool_deposit: 500_000_000,   // 500 ADA
            min_utxo_value: 1_000_000,   // 1 ADA
            utxo_cost_per_word: 4310,    // ~0.0043 ADA per word
            treasury_cut: 0.2,           // 20% to treasury
            monetary_expand_rate: 0.003, // 0.3% per year
            pool_pledge_influence: 0.3,  // Pool pledge influence
            pool_retirement_max_epoch: 18, // Max 18 epochs for retirement
            desired_number_of_pools: 500,  // Desired 500 pools
            pool_influence: 0.3,         // Pool influence parameter
        }
    }
}

impl ValidationLedgerState {
    pub fn new() -> Self {
        Self {
            utxo_set: HashMap::new(),
            stake_distribution: StakeDistribution::new(0),
            protocol_parameters: ProtocolParameters::mainnet(),
            current_epoch: 0,
            epoch_boundary_slot: 0,
            treasury: 1_000_000_000_000_000, // 1B ADA
            reserves: 14_000_000_000_000_000, // 14B ADA
            total_supply: 45_000_000_000_000_000, // 45B ADA max
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vrf_pair_for(consensus_state: &ConsensusState, slot: SlotNumber) -> (VrfOutput, VrfProof) {
        let mut payload = Vec::with_capacity(
            consensus_state.epoch_nonce.as_bytes().len() + std::mem::size_of::<SlotNumber>(),
        );
        payload.extend_from_slice(consensus_state.epoch_nonce.as_bytes());
        payload.extend_from_slice(&slot.to_le_bytes());

        super::vrf_prove_message(&payload)
    }

    fn create_test_block(consensus_state: &ConsensusState) -> ForgedBlock {
        let slot = 1000;
        let (vrf_output, vrf_proof) = vrf_pair_for(consensus_state, slot);

        let transactions = vec![
            Transaction {
                tx_id: Blake2b256Hash::new(b"tx1"),
                inputs: vec![TxInput {
                    tx_hash: Blake2b256Hash::new(b"input_tx"),
                    output_index: 0,
                }],
                outputs: vec![TxOutput {
                    address: Blake2b256Hash::new(b"output_addr"),
                    value: 2_000_000, // 2 ADA
                }],
                fee: 200_000, // 0.2 ADA
                size: 300,
            },
        ];

        let body = BlockBody {
            transactions,
            total_fee: 200_000,
            total_size: 300,
        };

        let proof_of_leadership = vrf_proof.clone();

        let header = BlockHeader {
            slot,
            prev_hash: Blake2b256Hash::new(b"prev_hash"),
            issuer_vkey: Ed25519KeyHash::new(b"test_pool"),
            vrf_proof,
            vrf_output,
            block_body_hash: Blake2b256Hash::new(b"body_hash"),
            block_size: 1000,
            operational_cert: OperationalCertificate {
                hot_vkey: Ed25519KeyHash::new(b"hot_key"),
                sequence_number: 1,
                kes_period: slot / 129600,
                sigma: Blake2b256Hash::new(b"cold_signature"),
            },
            protocol_magic: 764824073,
        };

        ForgedBlock {
            header,
            body,
            proof_of_leadership,
        }
    }

    fn create_test_pipeline() -> ValidationPipeline {
        let mut consensus_state = ConsensusState::new();
        consensus_state.active_slot_coeff = 1.0; // ensure deterministic leadership in tests
        let mut ledger_state = ValidationLedgerState::new();

        // Add test pool to stake distribution
        let pool_id = Ed25519KeyHash::new(b"test_pool");
        let vrf_key_hash = Blake2b256Hash::new(super::vrf_public_key().to_bytes());
        let pool_stake = crate::test_ouroboros_protocol::PoolStake {
            stake: 1_000_000_000_000, // 1M ADA
            vrf_key: vrf_key_hash,
            pool_params: crate::test_ouroboros_protocol::PoolParams {
                pledge: 100_000_000_000,
                cost: 340_000_000,
                margin: crate::test_ouroboros_protocol::Rational::new(3, 100).unwrap(),
                reward_account: Blake2b256Hash::new(b"reward_account"),
            },
        };
        ledger_state.stake_distribution.add_pool(pool_id, pool_stake);

        // Add test UTxO
        let input = TxInput {
            tx_hash: Blake2b256Hash::new(b"input_tx"),
            output_index: 0,
        };
        let output = TxOutput {
            address: Blake2b256Hash::new(b"input_addr"),
            value: 5_000_000, // 5 ADA
        };
        ledger_state.utxo_set.insert(input, output);

        ValidationPipeline::new(consensus_state, ledger_state)
    }

    #[test]
    fn test_valid_block_validation() {
        let mut pipeline = create_test_pipeline();
        let block = create_test_block(&pipeline.consensus_state);

        let result = pipeline.validate_block(&block).unwrap();

        // Block should be valid
        assert!(result.is_valid, "Block validation failed: {:?}", result.errors);
        assert!(result.errors.is_empty());
        assert!(result.new_ledger_state.is_some());

        println!("Validation time: {}ms", result.validation_time_ms);
        println!("Warnings: {:?}", result.warnings);
    }

    #[test]
    fn test_invalid_header_validation() {
        let mut pipeline = create_test_pipeline();
        let mut block = create_test_block(&pipeline.consensus_state);

        // Make header invalid - wrong protocol magic
        block.header.protocol_magic = 12345;

        let result = pipeline.validate_block(&block).unwrap();

        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());

        // Should have header validation error
        let has_header_error = result.errors.iter().any(|e| matches!(e, ValidationError::HeaderValidation(_)));
        assert!(has_header_error, "Expected header validation error");
    }

    #[test]
    fn test_invalid_transaction_validation() {
        let mut pipeline = create_test_pipeline();
        let mut block = create_test_block(&pipeline.consensus_state);

        // Make transaction invalid - no inputs
        block.body.transactions[0].inputs.clear();

        let result = pipeline.validate_block(&block).unwrap();

        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());

        // Should have transaction validation error
        let has_tx_error = result.errors.iter().any(|e| matches!(e, ValidationError::TransactionValidation(_)));
        assert!(has_tx_error, "Expected transaction validation error");
    }

    #[test]
    fn test_fee_calculation() {
        let pipeline = create_test_pipeline();
        let block = create_test_block(&pipeline.consensus_state);
        let tx = &block.body.transactions[0];

        let min_fee = pipeline.calculate_minimum_fee(tx);

        // min_fee = min_fee_a * size + min_fee_b = 44 * 300 + 155381 = 168581
        let expected_fee = 44 * 300 + 155381;
        assert_eq!(min_fee, expected_fee);
    }

    #[test]
    fn test_insufficient_fee() {
        let mut pipeline = create_test_pipeline();
        let mut block = create_test_block(&pipeline.consensus_state);

        // Set fee below minimum
        block.body.transactions[0].fee = 100_000; // Below minimum
        block.body.total_fee = 100_000;

        let result = pipeline.validate_block(&block).unwrap();

        assert!(!result.is_valid);

        // Should have transaction validation error about fee
        let has_fee_error = result.errors.iter().any(|e| {
            if let ValidationError::TransactionValidation(msg) = e {
                msg.contains("below minimum")
            } else {
                false
            }
        });
        assert!(has_fee_error, "Expected fee validation error");
    }

    #[test]
    fn test_utxo_application() {
        let pipeline = create_test_pipeline();
        let block = create_test_block(&pipeline.consensus_state);

        let initial_utxo_count = pipeline.ledger_state.utxo_set.len();

        let new_state = pipeline.apply_block_to_ledger(&block.body).unwrap();

        // Should have one less UTxO (consumed input) plus one new UTxO (output)
        // Net effect: same number of UTxOs
        assert_eq!(new_state.utxo_set.len(), initial_utxo_count);

        // Treasury should increase with fees
        let fee_to_treasury = (block.body.total_fee as f64 * pipeline.ledger_state.protocol_parameters.treasury_cut) as u64;
        let expected_treasury = pipeline.ledger_state.treasury + fee_to_treasury;
        assert_eq!(new_state.treasury, expected_treasury);
    }

    #[test]
    fn test_native_script_validation() {
        let pipeline = create_test_pipeline();

        // Valid scripts
        let valid_scripts = vec![
            NativeScript::RequireSignature(Ed25519KeyHash::new(b"valid_key")),
            NativeScript::RequireAllOf(vec![
                NativeScript::RequireSignature(Ed25519KeyHash::new(b"key1")),
                NativeScript::RequireSignature(Ed25519KeyHash::new(b"key2")),
            ]),
            NativeScript::RequireAnyOf(vec![
                NativeScript::RequireSignature(Ed25519KeyHash::new(b"key1")),
                NativeScript::RequireTimeBefore(500000),
            ]),
            NativeScript::RequireNOf(2, vec![
                NativeScript::RequireSignature(Ed25519KeyHash::new(b"key1")),
                NativeScript::RequireSignature(Ed25519KeyHash::new(b"key2")),
                NativeScript::RequireSignature(Ed25519KeyHash::new(b"key3")),
            ]),
        ];

        for script in valid_scripts {
            assert!(pipeline.validate_native_script(&script).is_ok());
        }

        // Invalid scripts
        let invalid_scripts = vec![
            NativeScript::RequireAllOf(vec![]), // Empty
            NativeScript::RequireAnyOf(vec![]), // Empty
            NativeScript::RequireNOf(5, vec![ // n > script count
                NativeScript::RequireSignature(Ed25519KeyHash::new(b"key1")),
            ]),
        ];

        for script in invalid_scripts {
            assert!(pipeline.validate_native_script(&script).is_err());
        }
    }

    #[test]
    fn test_plutus_script_validation() {
        let pipeline = create_test_pipeline();

        // Valid Plutus scripts
        let valid_scripts = vec![
            PlutusScript {
                version: PlutusVersion::V1,
                code: vec![1, 2, 3, 4, 5], // Some code
            },
            PlutusScript {
                version: PlutusVersion::V2,
                code: vec![0x61; 1000], // 1KB script
            },
        ];

        for script in valid_scripts {
            assert!(pipeline.validate_plutus_script(&script).is_ok());
        }

        // Invalid Plutus scripts
        let invalid_scripts = vec![
            PlutusScript {
                version: PlutusVersion::V1,
                code: vec![], // Empty code
            },
            PlutusScript {
                version: PlutusVersion::V2,
                code: vec![0x61; 20000], // Too large (>16KB)
            },
        ];

        for script in invalid_scripts {
            assert!(pipeline.validate_plutus_script(&script).is_err());
        }
    }

    #[test]
    fn test_validation_config() {
        let fast_config = ValidationConfig::fast_validation();
        assert!(!fast_config.validate_signatures);
        assert!(!fast_config.validate_vrf_proofs);
        assert!(fast_config.parallel_tx_validation);

        let default_config = ValidationConfig::default();
        assert!(default_config.validate_signatures);
        assert!(default_config.validate_vrf_proofs);
        assert!(!default_config.parallel_tx_validation);
    }

    #[test]
    fn test_protocol_parameters() {
        let params = ProtocolParameters::mainnet();

        assert_eq!(params.min_fee_a, 44);
        assert_eq!(params.min_fee_b, 155381);
        assert_eq!(params.max_block_size, 90112);
        assert_eq!(params.key_deposit, 2_000_000);
        assert_eq!(params.pool_deposit, 500_000_000);
    }

    #[test]
    fn test_vkey_witness_validation() {
        let pipeline = create_test_pipeline();
        let block = create_test_block(&pipeline.consensus_state);
        let tx = &block.body.transactions[0];

        // Valid witness
        let valid_witness = VKeyWitness {
            vkey: Ed25519KeyHash::new(b"valid_key"),
            signature: Blake2b256Hash::new(b"valid_signature"),
        };

        assert!(pipeline.validate_vkey_witness(tx, &valid_witness).is_ok());

        // Invalid witnesses
        let empty_sig_witness = VKeyWitness {
            vkey: Ed25519KeyHash::new(b"valid_key"),
            signature: Blake2b256Hash::new(b""), // Empty signature
        };

        assert!(pipeline.validate_vkey_witness(tx, &empty_sig_witness).is_err());
    }

    #[test]
    fn test_oversized_block_rejection() {
        let mut pipeline = create_test_pipeline();
        let mut block = create_test_block(&pipeline.consensus_state);

        // Make block too large
        block.header.block_size = 100_000; // Exceeds max_block_size
        block.body.total_size = 100_000;

        let result = pipeline.validate_block(&block).unwrap();

        assert!(!result.is_valid);

        // Should have header validation error about size
        let has_size_error = result.errors.iter().any(|e| {
            if let ValidationError::HeaderValidation(msg) = e {
                msg.contains("exceeds maximum size")
            } else {
                false
            }
        });
        assert!(has_size_error, "Expected block size validation error");
    }

    #[test]
    fn test_validation_timing() {
        let mut pipeline = create_test_pipeline();
        pipeline.validation_config.max_validation_time_ms = 1; // Very low limit

        let block = create_test_block(&pipeline.consensus_state);

        let result = pipeline.validate_block(&block).unwrap();

        // Should have a timing warning if validation took too long
        if result.validation_time_ms > 1 {
            let has_timing_warning = result.warnings.iter().any(|w| matches!(w, ValidationWarning::SlowValidation(_)));
            assert!(has_timing_warning, "Expected slow validation warning");
        }
    }
}
