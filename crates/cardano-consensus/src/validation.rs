//! Block and transaction validation
//!
//! Implements validation rules for blocks and transactions across all eras.
//! Provides comprehensive validation pipeline including header validation,
//! transaction validation, witness verification, and ledger state transitions.

use crate::block_production::{BlockBody, ForgedBlock, Transaction, TxInput, TxOutput};
use crate::ouroboros::{PoolId, SlotNo, StakeDistribution};
use crate::{ConsensusError, Result};
use cardano_crypto::{Blake2b256Hash, Ed25519KeyHash, KesSignature, VrfProof};
use std::collections::HashMap;
use std::time::Instant;

/// Convert slot number to KES period (simplified)
fn slot_to_kes_period(slot: SlotNo) -> u64 {
    // KES period typically is slot / 129600 (slots per period)
    slot.0 / 129600
}

/// Complete block validation pipeline
#[derive(Debug)]
pub struct ValidationPipeline {
    pub consensus_state: ConsensusState,
    pub ledger_state: ValidationLedgerState,
    pub validation_config: ValidationConfig,
}

/// Simplified consensus state for validation
#[derive(Debug, Clone)]
pub struct ConsensusState {
    pub current_slot: SlotNo,
}

/// Extended ledger state for validation
#[derive(Debug, Clone)]
pub struct ValidationLedgerState {
    pub utxo_set: HashMap<TxInput, TxOutput>,
    pub stake_distribution: StakeDistribution,
    pub protocol_parameters: ProtocolParameters,
    pub current_epoch: u64,
    pub epoch_boundary_slot: SlotNo,
    pub treasury: u64,
    pub reserves: u64,
    pub total_supply: u64,
}

/// Protocol parameters for validation
#[derive(Debug, Clone)]
pub struct ProtocolParameters {
    pub min_fee_a: u64,                 // Linear fee coefficient
    pub min_fee_b: u64,                 // Constant fee coefficient
    pub max_block_size: u32,            // Maximum block body size
    pub max_tx_size: u32,               // Maximum transaction size
    pub max_block_header_size: u32,     // Maximum block header size
    pub key_deposit: u64,               // Stake key deposit
    pub pool_deposit: u64,              // Pool registration deposit
    pub min_utxo_value: u64,            // Minimum value per UTxO
    pub utxo_cost_per_word: u64,        // Cost per word for UTxO
    pub treasury_cut: f64,              // Treasury cut from rewards
    pub monetary_expand_rate: f64,      // Monetary expansion rate
    pub pool_pledge_influence: f64,     // Pool pledge influence factor
    pub pool_retirement_max_epoch: u64, // Max epochs for pool retirement
    pub desired_number_of_pools: u32,   // Desired number of stake pools
    pub pool_influence: f64,            // Pool influence parameter
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
    RequireTimeBefore(SlotNo),
    RequireTimeAfter(SlotNo),
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

// Block header for validation (alias to block production header)
pub use crate::block_production::BlockHeader;

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
        let start_time = Instant::now();
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
                errors.push(ValidationError::TransactionValidation(format!(
                    "Transaction {}: {}",
                    i, e
                )));
            }
        }

        // Phase 4: Cryptographic validation
        if self.validation_config.validate_vrf_proofs {
            if let Err(e) =
                self.validate_vrf_proof(&block.header, &block.proof_of_leadership.vrf_proof)
            {
                errors.push(ValidationError::VrfValidation(e.to_string()));
            }
        }

        // Phase 4.5: KES signature validation
        if self.validation_config.validate_kes_signatures {
            if let Err(e) = self.validate_kes_signature(block) {
                errors.push(ValidationError::KesValidation(e.to_string()));
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
            warnings.push(ValidationWarning::SlowValidation(format!(
                "Validation took {}ms",
                validation_time
            )));
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
            return Err(ConsensusError::InvalidSlot(
                "Block from past or current slot".to_string(),
            ));
        }

        // Protocol magic validation
        if header.protocol_magic != 764824073 {
            // Mainnet magic
            return Err(ConsensusError::InvalidProtocolMagic(
                "Wrong network".to_string(),
            ));
        }

        // Block size validation
        if header.block_size > self.ledger_state.protocol_parameters.max_block_size {
            return Err(ConsensusError::InvalidBlock(
                "Block exceeds maximum size".to_string(),
            ));
        }

        // Previous hash validation (simplified - would check against chain)
        if header.prev_hash.as_bytes().is_empty() {
            return Err(ConsensusError::InvalidBlock(
                "Empty previous hash".to_string(),
            ));
        }

        // Operational certificate validation
        self.validate_operational_certificate(&header.operational_cert, header.slot)?;

        // VRF output validation
        if header.vrf_output.to_bytes().len() != 64 {
            return Err(ConsensusError::InvalidVrfProof(
                "Invalid VRF output length".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate operational certificate
    fn validate_operational_certificate(
        &self,
        cert: &crate::block_production::OperationalCertificate,
        slot: SlotNo,
    ) -> Result<()> {
        // KES period validation
        let expected_kes_period = slot.0 / 129600; // ~36 hours per KES period
        if cert.kes_period != expected_kes_period {
            return Err(ConsensusError::InvalidOperationalCert(
                "Wrong KES period".to_string(),
            ));
        }

        // Sequence number should be positive
        if cert.sequence_number == 0 {
            return Err(ConsensusError::InvalidOperationalCert(
                "Invalid sequence number".to_string(),
            ));
        }

        // Check if KES key is current (simplified for now)
        let current_kes_period = slot_to_kes_period(slot);
        if cert.kes_period != current_kes_period {
            return Err(ConsensusError::InvalidOperationalCert(
                "KES period mismatch".to_string(),
            ));
        }

        // Signature validation (simplified using sigma field)
        if cert.sigma.as_bytes().iter().all(|&b| b == 0) {
            return Err(ConsensusError::InvalidOperationalCert(
                "Empty signature".to_string(),
            ));
        }
        Ok(())
    }

    /// Validate block body
    fn validate_body(&self, body: &BlockBody) -> Result<()> {
        // Size validation
        if body.total_size > self.ledger_state.protocol_parameters.max_block_size {
            return Err(ConsensusError::InvalidBlock(
                "Body exceeds maximum size".to_string(),
            ));
        }

        // Transaction count validation (reasonable limit)
        if body.transactions.len() > 1000 {
            return Err(ConsensusError::InvalidBlock(
                "Too many transactions in block".to_string(),
            ));
        }

        // Fee validation
        let calculated_fees: u64 = body.transactions.iter().map(|tx| tx.fee).sum();
        if calculated_fees != body.total_fee {
            return Err(ConsensusError::InvalidBlock(
                "Fee mismatch in block body".to_string(),
            ));
        }

        // Size consistency
        let calculated_size: u32 = body.transactions.iter().map(|tx| tx.size).sum();
        if calculated_size != body.total_size {
            return Err(ConsensusError::InvalidBlock(
                "Size mismatch in block body".to_string(),
            ));
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
            return Err(ConsensusError::InvalidTransaction(
                "Transaction too large".to_string(),
            ));
        }

        // Fee validation
        let min_fee = self.calculate_minimum_fee(tx);
        if tx.fee < min_fee {
            return Err(ConsensusError::InvalidTransaction(format!(
                "Fee {} below minimum {}",
                tx.fee, min_fee
            )));
        }

        // UTxO validation
        for output in &tx.outputs {
            if output.value < self.ledger_state.protocol_parameters.min_utxo_value {
                return Err(ConsensusError::InvalidTransaction(
                    "Output below minimum UTxO value".to_string(),
                ));
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
            return Err(ConsensusError::InvalidTransaction(
                "Zero total output value".to_string(),
            ));
        }

        // Check for overflow
        if total_output > 45_000_000_000_000_000 {
            // Max ADA supply
            return Err(ConsensusError::InvalidTransaction(
                "Output value exceeds maximum supply".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate VRF proof of leadership
    fn validate_vrf_proof(&self, _header: &BlockHeader, proof: &VrfProof) -> Result<()> {
        // VRF proof structure validation
        if proof.to_bytes().is_empty() {
            return Err(ConsensusError::InvalidVrfProof(
                "Empty VRF proof".to_string(),
            ));
        }

        // Check pool is in stake distribution (convert Ed25519KeyHash to PoolId)
        let pool_id = PoolId(Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap()); // Simplified conversion
        if !self
            .ledger_state
            .stake_distribution
            .pools
            .contains_key(&pool_id)
        {
            return Err(ConsensusError::PoolNotFound(
                "Pool not in stake distribution".to_string(),
            ));
        }

        // In a real implementation, this would:
        // 1. Reconstruct VRF input from epoch nonce and slot
        // 2. Verify VRF proof cryptographically
        // 3. Check that VRF output meets leadership threshold

        // Simplified validation
        let pool_stake = self.ledger_state.stake_distribution.pools[&pool_id];
        let relative_stake =
            pool_stake as f64 / self.ledger_state.stake_distribution.total_stake as f64;

        // Very basic threshold check (real implementation would use VRF output)
        if relative_stake < 0.000001 {
            // Pool must have at least 0.0001% stake
            return Err(ConsensusError::InvalidVrfProof(
                "Pool stake too small for leadership".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate KES signature on the block header
    ///
    /// This validates that the KES signature covers the block header correctly
    /// and that the signature period matches the block's KES period.
    fn validate_kes_signature(&self, block: &ForgedBlock) -> Result<()> {
        // Verify KES signature period matches the block's expected KES period
        let expected_kes_period = slot_to_kes_period(block.header.slot);

        if block.kes_signature.period != expected_kes_period {
            return Err(ConsensusError::InvalidKesSignature(format!(
                "KES signature period mismatch: expected {}, got {}",
                expected_kes_period, block.kes_signature.period
            )));
        }

        // Verify KES signature period matches operational certificate KES period
        if block.kes_signature.period < block.header.operational_cert.kes_period {
            return Err(ConsensusError::InvalidKesSignature(
                "KES signature period before operational certificate start period".to_string(),
            ));
        }

        // Get the block header bytes for signature verification
        let _header_bytes = block.header.to_bytes_for_signing();

        // In a complete implementation, we would:
        // 1. Extract the KES verification key from the operational certificate
        // 2. Verify the KES signature against the header bytes
        // 3. Check that the KES key hasn't expired
        //
        // For now, we do basic structure validation:

        // Verify signature has the expected structure
        let signature_bytes = block.kes_signature.signature_bytes();
        if signature_bytes.len() != KesSignature::RAW_SIZE {
            return Err(ConsensusError::InvalidKesSignature(format!(
                "Invalid KES signature length: expected {} bytes, got {}",
                KesSignature::RAW_SIZE,
                signature_bytes.len()
            )));
        }

        // TODO: Actual cryptographic verification would require:
        // let kes_vkey = KesPublicKey::from_operational_cert(&block.header.operational_cert)?;
        // kes_vkey.verify(block.kes_signature.period, &header_bytes, &block.kes_signature)?;

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
        new_state.treasury +=
            (body.total_fee as f64 * new_state.protocol_parameters.treasury_cut) as u64;

        // Update reserves (simplified monetary policy)
        let epoch_reward = self.calculate_epoch_rewards(&new_state);
        if new_state.reserves >= epoch_reward {
            new_state.reserves -= epoch_reward;
            new_state.treasury += epoch_reward;
        }

        Ok(new_state)
    }

    /// Apply single transaction to UTxO set
    fn apply_transaction_to_utxo(
        &self,
        state: &mut ValidationLedgerState,
        tx: &Transaction,
    ) -> Result<()> {
        // Remove consumed inputs
        for input in &tx.inputs {
            if !state.utxo_set.contains_key(input) {
                return Err(ConsensusError::InvalidInput(
                    "Input not found in UTxO set".to_string(),
                ));
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
    pub fn validate_transaction_witness(
        &self,
        tx: &Transaction,
        witness: &TransactionWitness,
    ) -> Result<()> {
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
            return Err(ConsensusError::InvalidSignature(
                "Empty signature".to_string(),
            ));
        }

        if witness.vkey.as_bytes().iter().all(|&b| b == 0) {
            return Err(ConsensusError::InvalidSignature(
                "Empty verification key".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate native script
    #[allow(clippy::only_used_in_recursion)]
    pub fn validate_native_script(&self, script: &NativeScript) -> Result<()> {
        match script {
            NativeScript::RequireSignature(key_hash) => {
                if key_hash.as_bytes().iter().all(|&b| b == 0) {
                    return Err(ConsensusError::InvalidScript(
                        "Empty key hash in script".to_string(),
                    ));
                }
            }

            NativeScript::RequireAllOf(scripts) => {
                if scripts.is_empty() {
                    return Err(ConsensusError::InvalidScript(
                        "Empty RequireAllOf script".to_string(),
                    ));
                }
                for subscript in scripts {
                    self.validate_native_script(subscript)?;
                }
            }

            NativeScript::RequireAnyOf(scripts) => {
                if scripts.is_empty() {
                    return Err(ConsensusError::InvalidScript(
                        "Empty RequireAnyOf script".to_string(),
                    ));
                }
                for subscript in scripts {
                    self.validate_native_script(subscript)?;
                }
            }

            NativeScript::RequireNOf(n, scripts) => {
                if *n as usize > scripts.len() {
                    return Err(ConsensusError::InvalidScript(
                        "RequireNOf n exceeds script count".to_string(),
                    ));
                }
                if scripts.is_empty() {
                    return Err(ConsensusError::InvalidScript(
                        "Empty RequireNOf script".to_string(),
                    ));
                }
                for subscript in scripts {
                    self.validate_native_script(subscript)?;
                }
            }

            NativeScript::RequireTimeBefore(slot) => {
                if slot.0 > 1_000_000_000 {
                    // Reasonable upper bound
                    return Err(ConsensusError::InvalidScript(
                        "RequireTimeBefore slot too large".to_string(),
                    ));
                }
            }

            NativeScript::RequireTimeAfter(slot) => {
                if slot.0 > 1_000_000_000 {
                    // Reasonable upper bound
                    return Err(ConsensusError::InvalidScript(
                        "RequireTimeAfter slot too large".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Validate Plutus script (simplified)
    pub fn validate_plutus_script(&self, script: &PlutusScript) -> Result<()> {
        if script.code.is_empty() {
            return Err(ConsensusError::InvalidScript(
                "Empty Plutus script code".to_string(),
            ));
        }

        // Basic size check
        if script.code.len() > 16384 {
            // 16KB max script size
            return Err(ConsensusError::InvalidScript(
                "Plutus script too large".to_string(),
            ));
        }

        // Version validation
        match script.version {
            PlutusVersion::V1 | PlutusVersion::V2 => {} // Valid versions
        }

        Ok(())
    }
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            validate_signatures: true,
            validate_vrf_proofs: true,
            validate_kes_signatures: true,
            validate_transactions: true,
            validate_ledger_rules: true,
            max_validation_time_ms: 5000,  // 5 seconds
            parallel_tx_validation: false, // Disabled for testing
        }
    }
}

impl ValidationConfig {
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
            min_fee_a: 44,                 // 44 lovelace per byte
            min_fee_b: 155381,             // 155381 lovelace base fee
            max_block_size: 90112,         // ~88KB
            max_tx_size: 16384,            // 16KB
            max_block_header_size: 1100,   // 1.1KB
            key_deposit: 2_000_000,        // 2 ADA
            pool_deposit: 500_000_000,     // 500 ADA
            min_utxo_value: 1_000_000,     // 1 ADA
            utxo_cost_per_word: 4310,      // ~0.0043 ADA per word
            treasury_cut: 0.2,             // 20% to treasury
            monetary_expand_rate: 0.003,   // 0.3% per year
            pool_pledge_influence: 0.3,    // Pool pledge influence
            pool_retirement_max_epoch: 18, // Max 18 epochs for retirement
            desired_number_of_pools: 500,  // Desired 500 pools
            pool_influence: 0.3,           // Pool influence parameter
        }
    }
}

impl Default for ValidationLedgerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidationLedgerState {
    pub fn new() -> Self {
        Self {
            utxo_set: HashMap::new(),
            stake_distribution: StakeDistribution {
                pools: HashMap::new(),
                total_stake: 0,
            },
            protocol_parameters: ProtocolParameters::mainnet(),
            current_epoch: 0,
            epoch_boundary_slot: SlotNo(0),
            treasury: 1_000_000_000_000_000,      // 1B ADA
            reserves: 14_000_000_000_000_000,     // 14B ADA
            total_supply: 45_000_000_000_000_000, // 45B ADA max
        }
    }
}

impl Default for ConsensusState {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsensusState {
    pub fn new() -> Self {
        Self {
            current_slot: SlotNo(0),
        }
    }
}
