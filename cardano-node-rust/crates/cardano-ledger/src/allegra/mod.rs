//! Allegra Era Ledger Implementation
//!
//! The Allegra era extends Shelley with native scripts support and validity intervals.
//! This era introduces time-based transaction validation and multi-signature capabilities.

use crate::{LedgerError, Result, Coin, Epoch, Slot};
use crate::shelley::{
    ShelleyTransaction, ShelleyTxIn, ShelleyTxOut, ShelleyAddress, Certificate,
    RewardAddress, ShelleyWitnessSet, VKeyWitness, BootstrapWitness, NativeScript,
    ShelleyLedgerState, ShelleyProtocolParameters, NetworkId, StakeCredential,
};
use cardano_crypto::{Blake2b256Hash, Ed25519KeyHash, Ed25519Signature};
use std::collections::{BTreeMap, BTreeSet};

/// Allegra era transaction
#[derive(Debug, Clone)]
pub struct AllegraTransaction {
    pub body: AllegraTransactionBody,
    pub witness_set: AllegraWitnessSet,
    pub auxiliary_data: Option<AuxiliaryData>,
}

/// Allegra transaction body with validity intervals
#[derive(Debug, Clone)]
pub struct AllegraTransactionBody {
    pub inputs: BTreeSet<ShelleyTxIn>,
    pub outputs: Vec<ShelleyTxOut>,
    pub fee: Coin,
    pub ttl: Option<Slot>,
    pub certificates: Vec<Certificate>,
    pub withdrawals: BTreeMap<RewardAddress, Coin>,
    pub validity_interval_start: Option<Slot>, // Allegra addition
    pub auxiliary_data_hash: Option<Blake2b256Hash>,
}

/// Allegra witness set with native scripts
#[derive(Debug, Clone)]
pub struct AllegraWitnessSet {
    pub vkey_witnesses: BTreeSet<VKeyWitness>,
    pub native_scripts: BTreeMap<Blake2b256Hash, NativeScript>,
    pub bootstrap_witnesses: BTreeSet<BootstrapWitness>,
}

/// Auxiliary data for metadata and scripts
#[derive(Debug, Clone)]
pub struct AuxiliaryData {
    pub metadata: Option<TransactionMetadata>,
    pub native_scripts: Vec<NativeScript>,
}

/// Transaction metadata (CBOR map)
#[derive(Debug, Clone)]
pub struct TransactionMetadata {
    pub map: BTreeMap<u64, MetadataValue>,
}

/// Metadata value types
#[derive(Debug, Clone)]
pub enum MetadataValue {
    Integer(i64),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<MetadataValue>),
    Map(BTreeMap<MetadataValue, MetadataValue>),
}

/// Native script with time constraints
#[derive(Debug, Clone)]
pub enum AllegraScript {
    /// Require a specific key signature
    ScriptPubkey(Ed25519KeyHash),
    /// Require all of the nested scripts
    ScriptAll(Vec<AllegraScript>),
    /// Require any one of the nested scripts
    ScriptAny(Vec<AllegraScript>),
    /// Require N of the nested scripts
    ScriptNOfK {
        n: u32,
        scripts: Vec<AllegraScript>,
    },
    /// Require transaction validity before slot
    ScriptInvalidBefore(Slot),
    /// Require transaction validity after slot
    ScriptInvalidHereafter(Slot),
}

/// Script evaluation context
#[derive(Debug, Clone)]
pub struct ScriptContext {
    pub current_slot: Slot,
    pub transaction: AllegraTransactionBody,
    pub signatures: BTreeSet<Ed25519KeyHash>,
}

/// Script validation result
#[derive(Debug, Clone)]
pub enum ScriptValidation {
    Valid,
    Invalid(String),
    RequiresSlotCheck(Slot),
}

/// Allegra ledger state extends Shelley
#[derive(Debug, Clone)]
pub struct AllegraLedgerState {
    pub shelley_state: ShelleyLedgerState,
    pub native_scripts: BTreeMap<Blake2b256Hash, NativeScript>,
}

/// Allegra protocol parameters extend Shelley
#[derive(Debug, Clone)]
pub struct AllegraProtocolParameters {
    pub shelley_params: ShelleyProtocolParameters,
    pub utxo_cost_per_word: Coin,         // Cost per word of UTxO storage
}

impl AllegraTransaction {
    /// Calculate transaction ID
    pub fn tx_id(&self) -> Blake2b256Hash {
        let body_data = format!("{:?}", self.body);
        Blake2b256Hash::hash(body_data.as_bytes())
    }

    /// Validate transaction structure and scripts
    pub fn validate(&self, context: &ScriptContext, params: &AllegraProtocolParameters) -> Result<()> {
        // First validate basic structure
        self.body.validate(&params.shelley_params)?;

        // Validate validity interval
        self.validate_validity_interval(context.current_slot)?;

        // Validate native scripts
        self.validate_native_scripts(context)?;

        // Validate auxiliary data
        if let Some(ref aux_data) = self.auxiliary_data {
            aux_data.validate(&self.body)?;
        }

        Ok(())
    }

    /// Validate transaction validity interval
    fn validate_validity_interval(&self, current_slot: Slot) -> Result<()> {
        // Check invalid_before constraint
        if let Some(invalid_before) = self.body.validity_interval_start {
            if current_slot < invalid_before {
                return Err(LedgerError::ValidationError(
                    format!("Transaction not yet valid (current slot: {}, invalid before: {})",
                           current_slot, invalid_before)
                ));
            }
        }

        // Check ttl constraint (invalid_hereafter)
        if let Some(ttl) = self.body.ttl {
            if current_slot >= ttl {
                return Err(LedgerError::ValidationError(
                    format!("Transaction expired (current slot: {}, ttl: {})",
                           current_slot, ttl)
                ));
            }
        }

        Ok(())
    }

    /// Validate native scripts in transaction
    fn validate_native_scripts(&self, context: &ScriptContext) -> Result<()> {
        for (script_hash, script) in &self.witness_set.native_scripts {
            // Verify script hash matches
            let computed_hash = script.hash();
            if computed_hash != *script_hash {
                return Err(LedgerError::ValidationError(
                    "Script hash mismatch".to_string()
                ));
            }

            // Evaluate script
            match script.evaluate(context)? {
                ScriptValidation::Valid => continue,
                ScriptValidation::Invalid(reason) => {
                    return Err(LedgerError::ValidationError(
                        format!("Script validation failed: {}", reason)
                    ));
                }
                ScriptValidation::RequiresSlotCheck(_) => {
                    return Err(LedgerError::ValidationError(
                        "Script requires slot check outside validity interval".to_string()
                    ));
                }
            }
        }

        Ok(())
    }

    /// Get all required signers for this transaction
    pub fn required_signers(&self) -> BTreeSet<Ed25519KeyHash> {
        let mut signers = BTreeSet::new();

        // Add signers required by inputs (UTxO addresses)
        for input in &self.body.inputs {
            // In practice, this would look up the UTxO and extract the required signers
            // For now, we'll assume it's handled elsewhere
        }

        // Add signers required by certificates
        for cert in &self.body.certificates {
            signers.extend(cert.required_signers());
        }

        // Add signers required by withdrawals
        for reward_addr in self.body.withdrawals.keys() {
            if let StakeCredential::Key(key_hash) = &reward_addr.stake_credential {
                signers.insert(*key_hash);
            }
        }

        // Add signers required by native scripts
        for script in self.witness_set.native_scripts.values() {
            signers.extend(script.required_signers());
        }

        signers
    }
}

impl AllegraTransactionBody {
    /// Validate transaction body structure
    pub fn validate(&self, protocol_params: &ShelleyProtocolParameters) -> Result<()> {
        // Check inputs not empty
        if self.inputs.is_empty() {
            return Err(LedgerError::InvalidTransaction("Transaction has no inputs".to_string()));
        }

        // Check outputs not empty
        if self.outputs.is_empty() {
            return Err(LedgerError::InvalidTransaction("Transaction has no outputs".to_string()));
        }

        // Check fee is non-zero
        if self.fee == 0 {
            return Err(LedgerError::InvalidTransaction("Transaction fee is zero".to_string()));
        }

        // Check outputs meet minimum UTxO requirement
        for output in &self.outputs {
            if output.amount < protocol_params.min_utxo {
                return Err(LedgerError::InvalidTransaction("Output below minimum UTxO".to_string()));
            }
        }

        // Validate certificates
        for cert in &self.certificates {
            cert.validate()?;
        }

        // Validate validity interval ordering
        if let (Some(start), Some(end)) = (self.validity_interval_start, self.ttl) {
            if start >= end {
                return Err(LedgerError::InvalidTransaction(
                    "Invalid validity interval: start >= end".to_string()
                ));
            }
        }

        Ok(())
    }

    /// Calculate transaction hash for signing
    pub fn hash(&self) -> Blake2b256Hash {
        let body_data = format!("{:?}", self);
        Blake2b256Hash::hash(body_data.as_bytes())
    }

    /// Calculate total output value including withdrawals
    pub fn total_output_value(&self) -> Result<Coin> {
        let mut total = 0u64;

        for output in &self.outputs {
            total = total.checked_add(output.amount)
                .ok_or_else(|| LedgerError::ValueOverflow("Output sum overflow".to_string()))?;
        }

        // Add withdrawals
        for &withdrawal in self.withdrawals.values() {
            total = total.checked_add(withdrawal)
                .ok_or_else(|| LedgerError::ValueOverflow("Withdrawal sum overflow".to_string()))?;
        }

        Ok(total)
    }

    /// Estimate transaction size in bytes
    pub fn estimate_size(&self) -> u32 {
        let base_size = 100; // Base transaction overhead
        let input_size = self.inputs.len() as u32 * 40;
        let output_size = self.outputs.len() as u32 * 50;
        let cert_size = self.certificates.len() as u32 * 80;
        let withdrawal_size = self.withdrawals.len() as u32 * 40;
        let validity_size = if self.validity_interval_start.is_some() { 10 } else { 0 };

        base_size + input_size + output_size + cert_size + withdrawal_size + validity_size
    }
}

impl AllegraScript {
    /// Evaluate script in given context
    pub fn evaluate(&self, context: &ScriptContext) -> Result<ScriptValidation> {
        match self {
            AllegraScript::ScriptPubkey(key_hash) => {
                if context.signatures.contains(key_hash) {
                    Ok(ScriptValidation::Valid)
                } else {
                    Ok(ScriptValidation::Invalid(
                        format!("Missing signature for key: {:?}", key_hash)
                    ))
                }
            }
            AllegraScript::ScriptAll(scripts) => {
                for script in scripts {
                    match script.evaluate(context)? {
                        ScriptValidation::Valid => continue,
                        invalid => return Ok(invalid),
                    }
                }
                Ok(ScriptValidation::Valid)
            }
            AllegraScript::ScriptAny(scripts) => {
                if scripts.is_empty() {
                    return Ok(ScriptValidation::Invalid("Empty ScriptAny".to_string()));
                }

                for script in scripts {
                    if let ScriptValidation::Valid = script.evaluate(context)? {
                        return Ok(ScriptValidation::Valid);
                    }
                }
                Ok(ScriptValidation::Invalid("No script in ScriptAny satisfied".to_string()))
            }
            AllegraScript::ScriptNOfK { n, scripts } => {
                if *n as usize > scripts.len() {
                    return Ok(ScriptValidation::Invalid(
                        "N greater than number of scripts".to_string()
                    ));
                }

                let mut satisfied = 0u32;
                for script in scripts {
                    if let ScriptValidation::Valid = script.evaluate(context)? {
                        satisfied += 1;
                        if satisfied >= *n {
                            return Ok(ScriptValidation::Valid);
                        }
                    }
                }
                Ok(ScriptValidation::Invalid(
                    format!("Only {} of {} required scripts satisfied", satisfied, n)
                ))
            }
            AllegraScript::ScriptInvalidBefore(slot) => {
                if context.current_slot >= *slot {
                    Ok(ScriptValidation::Valid)
                } else {
                    Ok(ScriptValidation::RequiresSlotCheck(*slot))
                }
            }
            AllegraScript::ScriptInvalidHereafter(slot) => {
                if context.current_slot < *slot {
                    Ok(ScriptValidation::Valid)
                } else {
                    Ok(ScriptValidation::Invalid(
                        format!("Current slot {} >= invalid_hereafter {}",
                               context.current_slot, slot)
                    ))
                }
            }
        }
    }

    /// Get all required signers for this script
    pub fn required_signers(&self) -> BTreeSet<Ed25519KeyHash> {
        match self {
            AllegraScript::ScriptPubkey(key_hash) => {
                vec![*key_hash].into_iter().collect()
            }
            AllegraScript::ScriptAll(scripts) => {
                scripts.iter().flat_map(|s| s.required_signers()).collect()
            }
            AllegraScript::ScriptAny(scripts) => {
                // For ScriptAny, we return all possible signers since we don't know which will be used
                scripts.iter().flat_map(|s| s.required_signers()).collect()
            }
            AllegraScript::ScriptNOfK { scripts, .. } => {
                // For ScriptNOfK, return all possible signers
                scripts.iter().flat_map(|s| s.required_signers()).collect()
            }
            AllegraScript::ScriptInvalidBefore(_) | AllegraScript::ScriptInvalidHereafter(_) => {
                BTreeSet::new() // Time-based scripts don't require signatures
            }
        }
    }

    /// Calculate script hash
    pub fn hash(&self) -> Blake2b256Hash {
        let script_data = format!("{:?}", self);
        Blake2b256Hash::hash(script_data.as_bytes())
    }
}

impl NativeScript {
    /// Evaluate Shelley-era native script
    pub fn evaluate(&self, context: &ScriptContext) -> Result<ScriptValidation> {
        match self {
            NativeScript::ScriptPubkey(key_hash) => {
                if context.signatures.contains(key_hash) {
                    Ok(ScriptValidation::Valid)
                } else {
                    Ok(ScriptValidation::Invalid(
                        format!("Missing signature for key: {:?}", key_hash)
                    ))
                }
            }
            NativeScript::ScriptAll(scripts) => {
                for script in scripts {
                    match script.evaluate(context)? {
                        ScriptValidation::Valid => continue,
                        invalid => return Ok(invalid),
                    }
                }
                Ok(ScriptValidation::Valid)
            }
            NativeScript::ScriptAny(scripts) => {
                if scripts.is_empty() {
                    return Ok(ScriptValidation::Invalid("Empty ScriptAny".to_string()));
                }

                for script in scripts {
                    if let ScriptValidation::Valid = script.evaluate(context)? {
                        return Ok(ScriptValidation::Valid);
                    }
                }
                Ok(ScriptValidation::Invalid("No script in ScriptAny satisfied".to_string()))
            }
            NativeScript::ScriptNOfK { n, scripts } => {
                if *n as usize > scripts.len() {
                    return Ok(ScriptValidation::Invalid(
                        "N greater than number of scripts".to_string()
                    ));
                }

                let mut satisfied = 0u32;
                for script in scripts {
                    if let ScriptValidation::Valid = script.evaluate(context)? {
                        satisfied += 1;
                        if satisfied >= *n {
                            return Ok(ScriptValidation::Valid);
                        }
                    }
                }
                Ok(ScriptValidation::Invalid(
                    format!("Only {} of {} required scripts satisfied", satisfied, n)
                ))
            }
        }
    }

    /// Get required signers for Shelley native script
    pub fn required_signers(&self) -> BTreeSet<Ed25519KeyHash> {
        match self {
            NativeScript::ScriptPubkey(key_hash) => {
                vec![*key_hash].into_iter().collect()
            }
            NativeScript::ScriptAll(scripts) => {
                scripts.iter().flat_map(|s| s.required_signers()).collect()
            }
            NativeScript::ScriptAny(scripts) => {
                scripts.iter().flat_map(|s| s.required_signers()).collect()
            }
            NativeScript::ScriptNOfK { scripts, .. } => {
                scripts.iter().flat_map(|s| s.required_signers()).collect()
            }
        }
    }

    /// Calculate script hash
    pub fn hash(&self) -> Blake2b256Hash {
        let script_data = format!("{:?}", self);
        Blake2b256Hash::hash(script_data.as_bytes())
    }
}

impl Certificate {
    /// Get required signers for certificate
    pub fn required_signers(&self) -> BTreeSet<Ed25519KeyHash> {
        match self {
            Certificate::StakeRegistration(cred) | Certificate::StakeDeregistration(cred) => {
                if let StakeCredential::Key(key_hash) = cred {
                    vec![*key_hash].into_iter().collect()
                } else {
                    BTreeSet::new()
                }
            }
            Certificate::StakeDelegation { stake_credential, .. } => {
                if let StakeCredential::Key(key_hash) = stake_credential {
                    vec![*key_hash].into_iter().collect()
                } else {
                    BTreeSet::new()
                }
            }
            Certificate::PoolRegistration(pool_reg) => {
                pool_reg.pool_owners.clone()
            }
            Certificate::PoolRetirement { .. } => {
                // Pool retirement requires pool owner signatures
                // In practice, this would look up the pool registration
                BTreeSet::new()
            }
        }
    }
}

impl AuxiliaryData {
    /// Validate auxiliary data
    pub fn validate(&self, tx_body: &AllegraTransactionBody) -> Result<()> {
        // Check that auxiliary data hash matches if present
        if let Some(expected_hash) = tx_body.auxiliary_data_hash {
            let computed_hash = self.hash();
            if computed_hash != expected_hash {
                return Err(LedgerError::ValidationError(
                    "Auxiliary data hash mismatch".to_string()
                ));
            }
        }

        // Validate metadata if present
        if let Some(ref metadata) = self.metadata {
            metadata.validate()?;
        }

        // Validate native scripts
        for script in &self.native_scripts {
            // Scripts in auxiliary data are just stored, validation happens during execution
        }

        Ok(())
    }

    /// Calculate auxiliary data hash
    pub fn hash(&self) -> Blake2b256Hash {
        let aux_data = format!("{:?}", self);
        Blake2b256Hash::hash(aux_data.as_bytes())
    }
}

impl TransactionMetadata {
    /// Validate transaction metadata
    pub fn validate(&self) -> Result<()> {
        // Check metadata size constraints
        let serialized_size = self.estimate_size();
        if serialized_size > 16384 { // 16KB limit
            return Err(LedgerError::ValidationError(
                "Metadata too large".to_string()
            ));
        }

        // Validate all values
        for (key, value) in &self.map {
            value.validate()?;
        }

        Ok(())
    }

    /// Estimate metadata size
    fn estimate_size(&self) -> usize {
        self.map.len() * 100 // Rough estimate
    }
}

impl MetadataValue {
    /// Validate metadata value
    pub fn validate(&self) -> Result<()> {
        match self {
            MetadataValue::Integer(_) => Ok(()),
            MetadataValue::Bytes(bytes) => {
                if bytes.len() > 64 {
                    Err(LedgerError::ValidationError("Metadata bytes too long".to_string()))
                } else {
                    Ok(())
                }
            }
            MetadataValue::Text(text) => {
                if text.len() > 64 {
                    Err(LedgerError::ValidationError("Metadata text too long".to_string()))
                } else {
                    Ok(())
                }
            }
            MetadataValue::Array(values) => {
                for value in values {
                    value.validate()?;
                }
                Ok(())
            }
            MetadataValue::Map(map) => {
                for (key, value) in map {
                    key.validate()?;
                    value.validate()?;
                }
                Ok(())
            }
        }
    }
}

impl AllegraLedgerState {
    /// Create new Allegra ledger state
    pub fn new() -> Self {
        Self {
            shelley_state: ShelleyLedgerState::new(),
            native_scripts: BTreeMap::new(),
        }
    }

    /// Apply Allegra transaction to ledger state
    pub fn apply_transaction(
        &mut self,
        tx: &AllegraTransaction,
        context: &ScriptContext,
        params: &AllegraProtocolParameters,
    ) -> Result<()> {
        // Validate transaction
        tx.validate(context, params)?;

        // Convert to Shelley transaction for basic processing
        let shelley_tx = ShelleyTransaction {
            inputs: tx.body.inputs.clone(),
            outputs: tx.body.outputs.clone(),
            fee: tx.body.fee,
            ttl: tx.body.ttl,
            certificates: tx.body.certificates.clone(),
            withdrawals: tx.body.withdrawals.clone(),
            auxiliary_data_hash: tx.body.auxiliary_data_hash,
        };

        // Apply using Shelley rules
        self.shelley_state.apply_transaction(&shelley_tx, &params.shelley_params)?;

        // Store native scripts
        for (hash, script) in &tx.witness_set.native_scripts {
            self.native_scripts.insert(*hash, script.clone());
        }

        Ok(())
    }

    /// Get stored native script
    pub fn get_native_script(&self, hash: &Blake2b256Hash) -> Option<&NativeScript> {
        self.native_scripts.get(hash)
    }
}

impl AllegraProtocolParameters {
    /// Create mainnet Allegra protocol parameters
    pub fn mainnet() -> Self {
        Self {
            shelley_params: ShelleyProtocolParameters::mainnet(),
            utxo_cost_per_word: 34482, // Cost per word in lovelace
        }
    }

    /// Create testnet Allegra protocol parameters
    pub fn testnet() -> Self {
        Self {
            shelley_params: ShelleyProtocolParameters::testnet(),
            utxo_cost_per_word: 34482,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cardano_crypto::Ed25519KeyHash;

    #[test]
    fn test_allegra_script_pubkey() {
        let key_hash = Ed25519KeyHash::from_test_data(b"test_key");
        let script = AllegraScript::ScriptPubkey(key_hash);

        // Create context with signature
        let mut signatures = BTreeSet::new();
        signatures.insert(key_hash);

        let context = ScriptContext {
            current_slot: 1000,
            transaction: AllegraTransactionBody {
                inputs: BTreeSet::new(),
                outputs: vec![],
                fee: 0,
                ttl: None,
                certificates: vec![],
                withdrawals: BTreeMap::new(),
                validity_interval_start: None,
                auxiliary_data_hash: None,
            },
            signatures,
        };

        assert!(matches!(script.evaluate(&context).unwrap(), ScriptValidation::Valid));
    }

    #[test]
    fn test_allegra_script_time_constraints() {
        let current_slot = 1000;

        // Test InvalidBefore
        let script_before = AllegraScript::ScriptInvalidBefore(500);
        let context = create_test_context(current_slot);
        assert!(matches!(script_before.evaluate(&context).unwrap(), ScriptValidation::Valid));

        let script_before_fail = AllegraScript::ScriptInvalidBefore(1500);
        assert!(matches!(script_before_fail.evaluate(&context).unwrap(), ScriptValidation::RequiresSlotCheck(_)));

        // Test InvalidHereafter
        let script_after = AllegraScript::ScriptInvalidHereafter(1500);
        assert!(matches!(script_after.evaluate(&context).unwrap(), ScriptValidation::Valid));

        let script_after_fail = AllegraScript::ScriptInvalidHereafter(500);
        assert!(matches!(script_after_fail.evaluate(&context).unwrap(), ScriptValidation::Invalid(_)));
    }

    #[test]
    fn test_allegra_script_all() {
        let key1 = Ed25519KeyHash::from_test_data(b"key1");
        let key2 = Ed25519KeyHash::from_test_data(b"key2");

        let script = AllegraScript::ScriptAll(vec![
            AllegraScript::ScriptPubkey(key1),
            AllegraScript::ScriptPubkey(key2),
        ]);

        // Context with both signatures
        let mut signatures = BTreeSet::new();
        signatures.insert(key1);
        signatures.insert(key2);
        let context = ScriptContext {
            current_slot: 1000,
            transaction: create_test_tx_body(),
            signatures,
        };

        assert!(matches!(script.evaluate(&context).unwrap(), ScriptValidation::Valid));

        // Context with only one signature
        let mut signatures = BTreeSet::new();
        signatures.insert(key1);
        let context_partial = ScriptContext {
            current_slot: 1000,
            transaction: create_test_tx_body(),
            signatures,
        };

        assert!(matches!(script.evaluate(&context_partial).unwrap(), ScriptValidation::Invalid(_)));
    }

    #[test]
    fn test_allegra_script_any() {
        let key1 = Ed25519KeyHash::from_test_data(b"key1");
        let key2 = Ed25519KeyHash::from_test_data(b"key2");

        let script = AllegraScript::ScriptAny(vec![
            AllegraScript::ScriptPubkey(key1),
            AllegraScript::ScriptPubkey(key2),
        ]);

        // Context with one signature
        let mut signatures = BTreeSet::new();
        signatures.insert(key1);
        let context = ScriptContext {
            current_slot: 1000,
            transaction: create_test_tx_body(),
            signatures,
        };

        assert!(matches!(script.evaluate(&context).unwrap(), ScriptValidation::Valid));

        // Context with no signatures
        let context_empty = ScriptContext {
            current_slot: 1000,
            transaction: create_test_tx_body(),
            signatures: BTreeSet::new(),
        };

        assert!(matches!(script.evaluate(&context_empty).unwrap(), ScriptValidation::Invalid(_)));
    }

    #[test]
    fn test_allegra_script_n_of_k() {
        let key1 = Ed25519KeyHash::from_test_data(b"key1");
        let key2 = Ed25519KeyHash::from_test_data(b"key2");
        let key3 = Ed25519KeyHash::from_test_data(b"key3");

        let script = AllegraScript::ScriptNOfK {
            n: 2,
            scripts: vec![
                AllegraScript::ScriptPubkey(key1),
                AllegraScript::ScriptPubkey(key2),
                AllegraScript::ScriptPubkey(key3),
            ],
        };

        // Context with 2 signatures (should pass)
        let mut signatures = BTreeSet::new();
        signatures.insert(key1);
        signatures.insert(key2);
        let context = ScriptContext {
            current_slot: 1000,
            transaction: create_test_tx_body(),
            signatures,
        };

        assert!(matches!(script.evaluate(&context).unwrap(), ScriptValidation::Valid));

        // Context with 1 signature (should fail)
        let mut signatures = BTreeSet::new();
        signatures.insert(key1);
        let context_insufficient = ScriptContext {
            current_slot: 1000,
            transaction: create_test_tx_body(),
            signatures,
        };

        assert!(matches!(script.evaluate(&context_insufficient).unwrap(), ScriptValidation::Invalid(_)));
    }

    #[test]
    fn test_allegra_validity_interval() {
        let mut tx = create_test_allegra_tx();
        tx.body.validity_interval_start = Some(500);
        tx.body.ttl = Some(1500);

        // Valid slot
        let context = create_test_context(1000);
        assert!(tx.validate_validity_interval(1000).is_ok());

        // Too early
        assert!(tx.validate_validity_interval(400).is_err());

        // Too late
        assert!(tx.validate_validity_interval(1600).is_err());
    }

    #[test]
    fn test_metadata_validation() {
        let mut metadata = TransactionMetadata {
            map: BTreeMap::new(),
        };

        // Valid metadata
        metadata.map.insert(1, MetadataValue::Text("Hello".to_string()));
        metadata.map.insert(2, MetadataValue::Integer(42));
        assert!(metadata.validate().is_ok());

        // Invalid metadata (text too long)
        metadata.map.insert(3, MetadataValue::Text("a".repeat(100)));
        assert!(metadata.validate().is_err());
    }

    #[test]
    fn test_allegra_transaction_validation() {
        let tx = create_test_allegra_tx();
        let context = create_test_context(1000);
        let params = AllegraProtocolParameters::testnet();

        assert!(tx.body.validate(&params.shelley_params).is_ok());
    }

    fn create_test_context(current_slot: Slot) -> ScriptContext {
        ScriptContext {
            current_slot,
            transaction: create_test_tx_body(),
            signatures: BTreeSet::new(),
        }
    }

    fn create_test_tx_body() -> AllegraTransactionBody {
        AllegraTransactionBody {
            inputs: BTreeSet::new(),
            outputs: vec![],
            fee: 0,
            ttl: None,
            certificates: vec![],
            withdrawals: BTreeMap::new(),
            validity_interval_start: None,
            auxiliary_data_hash: None,
        }
    }

    fn create_test_allegra_tx() -> AllegraTransaction {
        AllegraTransaction {
            body: AllegraTransactionBody {
                inputs: vec![ShelleyTxIn {
                    transaction_id: Blake2b256Hash::hash(b"test"),
                    output_index: 0,
                }].into_iter().collect(),
                outputs: vec![ShelleyTxOut {
                    address: crate::shelley::ShelleyAddress::new_enterprise(
                        NetworkId::Testnet,
                        StakeCredential::Key(Ed25519KeyHash::from_test_data(b"output"))
                    ),
                    amount: 2_000_000,
                }],
                fee: 200_000,
                ttl: Some(2000),
                certificates: vec![],
                withdrawals: BTreeMap::new(),
                validity_interval_start: Some(1000),
                auxiliary_data_hash: None,
            },
            witness_set: AllegraWitnessSet {
                vkey_witnesses: BTreeSet::new(),
                native_scripts: BTreeMap::new(),
                bootstrap_witnesses: BTreeSet::new(),
            },
            auxiliary_data: None,
        }
    }
}
