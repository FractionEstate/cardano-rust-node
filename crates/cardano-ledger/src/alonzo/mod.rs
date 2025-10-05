//! Alonzo Era Ledger Rules
//!
//! The Alonzo era (September 2021) introduced Plutus smart contracts to Cardano,
//! enabling programmable validation logic beyond native scripts. This era builds
//! on Mary's multi-asset support while adding:
//! - Plutus V1 smart contracts
//! - Script data (redeemers and datums)
//! - Collateral inputs for script execution
//! - Extended UTXO model (EUTxO)
//! - Script execution cost accounting

use crate::mary::{
    Address, Certificate, Coin, Ed25519KeyHash, MaryValue, MultiAsset, PolicyId,
    RewardAddress, Slot, ValidityInterval,
};
use crate::{LedgerError, Result};
use cardano_crypto::Blake2b256Hash;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Alonzo Era Transaction with Plutus script support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlonzoTransaction {
    pub inputs: Vec<TransactionInput>,
    pub outputs: Vec<AlonzoTransactionOutput>,
    pub fee: Coin,
    pub ttl: Option<Slot>,
    pub certificates: Vec<Certificate>,
    pub withdrawals: HashMap<RewardAddress, Coin>,
    pub auxiliary_data: Option<AuxiliaryData>,
    pub validity_interval: ValidityInterval,
    pub mint: Option<MultiAsset>,
    pub script_data_hash: Option<Blake2b256Hash>, // New: hash of redeemers and datums
    pub collateral: Vec<TransactionInput>,        // New: collateral for script failures
    pub required_signers: Vec<Ed25519KeyHash>,    // New: required signatures for scripts
    pub network_id: Option<NetworkId>,            // New: network identification
    pub witness_set: AlonzoWitnessSet,
}

/// Alonzo Transaction Output with datum support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlonzoTransactionOutput {
    pub address: Address,
    pub value: MaryValue,
    pub datum: Option<Datum>,          // New: full datum (not just hash)
    pub script_ref: Option<ScriptRef>, // Added later in Babbage but defined here
}

/// Script data that can be attached to outputs or used in script execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Datum {
    /// Hash of the actual datum (for privacy)
    DatumHash(Blake2b256Hash),
    /// Inline datum data (Babbage era feature, but defined here for compatibility)
    InlineDatum(PlutusData),
}

/// Reference to a script for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScriptRef {
    NativeScript(NativeScript),
    PlutusScript(PlutusScript),
}

/// Plutus smart contract script
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlutusScript {
    pub version: PlutusVersion,
    pub code: Vec<u8>, // CBOR-encoded Plutus Core code
}

/// Plutus script version
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlutusVersion {
    V1, // Alonzo era
    V2, // Babbage era (defined here for forward compatibility)
}

/// Plutus data structure (simplified representation)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlutusData {
    Constr(u64, Vec<PlutusData>),       // Constructor with tag and fields
    Map(Vec<(PlutusData, PlutusData)>), // Key-value map
    List(Vec<PlutusData>),              // List of data
    Integer(i64),                       // Big integer (simplified as i64)
    Bytes(Vec<u8>),                     // Byte string
}

/// Script redeemer providing context for script execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Redeemer {
    pub tag: RedeemerTag,
    pub index: u32, // Index into the corresponding input/mint/cert/withdrawal
    pub data: PlutusData,
    pub ex_units: ExUnits, // Execution cost budget
}

/// Redeemer usage context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RedeemerTag {
    Spend, // For spending script inputs
    Mint,  // For minting/burning tokens
    Cert,  // For certificates
    Wdrl,  // For withdrawals
}

/// Execution units for script cost accounting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExUnits {
    pub mem: u64,   // Memory usage
    pub steps: u64, // CPU steps
}

/// Network identifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkId {
    Testnet,
    Mainnet,
}

/// Alonzo witness set with Plutus script support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlonzoWitnessSet {
    pub vkey_witnesses: Vec<VKeyWitness>,
    pub native_scripts: Vec<NativeScript>,
    pub bootstrap_witnesses: Vec<BootstrapWitness>,
    pub plutus_v1_scripts: Vec<PlutusScript>, // New: Plutus V1 scripts
    pub plutus_data: Vec<PlutusData>,         // New: datum and redeemer data
    pub redeemers: Vec<Redeemer>,             // New: script redeemers
}

// Re-exported types from Mary era
pub use crate::mary::{
    AuxiliaryData, BootstrapWitness, NativeScript, TransactionInput, VKeyWitness,
};

/// Script purpose for validation context
#[derive(Debug, Clone)]
pub enum ScriptPurpose {
    Spending(TransactionInput),
    Minting(PolicyId),
    Certifying(Certificate),
    Rewarding(RewardAddress),
}

/// Script execution context
#[derive(Debug, Clone)]
pub struct ScriptContext {
    pub tx_info: TxInfo,
    pub purpose: ScriptPurpose,
}

/// Transaction information available to scripts
#[derive(Debug, Clone)]
pub struct TxInfo {
    pub inputs: Vec<TxInInfo>,
    pub outputs: Vec<AlonzoTransactionOutput>,
    pub fee: MaryValue,
    pub mint: MaryValue,
    pub dcert: Vec<Certificate>,
    pub wdrl: Vec<(RewardAddress, Coin)>,
    pub valid_range: ValidityInterval,
    pub signatories: Vec<Ed25519KeyHash>,
    pub data: HashMap<Blake2b256Hash, PlutusData>,
    pub id: Blake2b256Hash, // Transaction hash
}

/// Transaction input info for scripts
#[derive(Debug, Clone)]
pub struct TxInInfo {
    pub out_ref: TransactionInput,
    pub resolved: AlonzoTransactionOutput,
}

impl PlutusData {
    /// Create integer PlutusData
    pub fn integer(value: i64) -> Self {
        Self::Integer(value)
    }

    /// Create bytes PlutusData
    pub fn bytes(data: Vec<u8>) -> Self {
        Self::Bytes(data)
    }

    /// Create constructor PlutusData
    pub fn constr(tag: u64, fields: Vec<PlutusData>) -> Self {
        Self::Constr(tag, fields)
    }

    /// Create list PlutusData
    pub fn list(items: Vec<PlutusData>) -> Self {
        Self::List(items)
    }

    /// Create map PlutusData
    pub fn map(pairs: Vec<(PlutusData, PlutusData)>) -> Self {
        Self::Map(pairs)
    }

    /// Calculate hash of this PlutusData
    pub fn hash(&self) -> Blake2b256Hash {
        // In practice, this would serialize to CBOR and hash
        let serialized = format!("{:?}", self);
        Blake2b256Hash::hash(serialized.as_bytes())
    }
}

impl PlutusScript {
    /// Create a new Plutus V1 script
    pub fn v1(code: Vec<u8>) -> Self {
        Self {
            version: PlutusVersion::V1,
            code,
        }
    }

    /// Create a new Plutus V2 script
    pub fn v2(code: Vec<u8>) -> Self {
        Self {
            version: PlutusVersion::V2,
            code,
        }
    }

    /// Calculate script hash for policy ID
    pub fn hash(&self) -> Blake2b256Hash {
        // In practice, this would use the proper Plutus script hashing algorithm
        Blake2b256Hash::hash(&self.code)
    }
}

impl ExUnits {
    /// Create new ExUnits
    pub fn new(mem: u64, steps: u64) -> Self {
        Self { mem, steps }
    }

    /// Add another ExUnits to this one
    pub fn add(&mut self, other: &ExUnits) -> Result<()> {
        self.mem = self
            .mem
            .checked_add(other.mem)
            .ok_or_else(|| LedgerError::ScriptError("Memory unit overflow".to_string()))?;
        self.steps = self
            .steps
            .checked_add(other.steps)
            .ok_or_else(|| LedgerError::ScriptError("Step unit overflow".to_string()))?;
        Ok(())
    }

    /// Check if this ExUnits is within budget
    pub fn within_budget(&self, budget: &ExUnits) -> bool {
        self.mem <= budget.mem && self.steps <= budget.steps
    }
}

/// Alonzo era ledger validation
pub struct AlonzoLedger;

impl AlonzoLedger {
    /// Validate an Alonzo era transaction
    pub fn validate_transaction(tx: &AlonzoTransaction) -> Result<()> {
        // 1. Validate basic transaction structure
        Self::validate_structure(tx)?;

        // 2. Validate script data hash if present
        Self::validate_script_data_hash(tx)?;

        // 3. Validate collateral inputs
        Self::validate_collateral(tx)?;

        // 4. Validate execution units
        Self::validate_execution_units(tx)?;

        // 5. Validate Plutus scripts (simplified)
        Self::validate_plutus_scripts(tx)?;

        Ok(())
    }

    fn validate_structure(tx: &AlonzoTransaction) -> Result<()> {
        if tx.inputs.is_empty() {
            return Err(LedgerError::InvalidTransaction(
                "Transaction must have at least one input".to_string(),
            ));
        }

        // Collateral can be empty if no scripts are used
        if !tx.collateral.is_empty()
            && tx.witness_set.plutus_v1_scripts.is_empty()
            && tx.witness_set.redeemers.is_empty()
        {
            return Err(LedgerError::InvalidTransaction(
                "Collateral provided but no scripts present".to_string(),
            ));
        }

        Ok(())
    }

    fn validate_script_data_hash(tx: &AlonzoTransaction) -> Result<()> {
        let has_script_data =
            !tx.witness_set.plutus_data.is_empty() || !tx.witness_set.redeemers.is_empty();

        match (has_script_data, &tx.script_data_hash) {
            (true, None) => {
                return Err(LedgerError::InvalidTransaction(
                    "Script data present but hash missing".to_string(),
                ));
            }
            (false, Some(_)) => {
                return Err(LedgerError::InvalidTransaction(
                    "Script data hash present but no script data".to_string(),
                ));
            }
            _ => {} // Valid combinations
        }

        Ok(())
    }

    fn validate_collateral(tx: &AlonzoTransaction) -> Result<()> {
        if tx.collateral.is_empty() {
            return Ok(()); // No collateral is valid for non-script transactions
        }

        // Collateral inputs must be simple (no scripts)
        // This would be validated against the UTxO set in practice
        if tx.collateral.len() > 3 {
            return Err(LedgerError::InvalidTransaction(
                "Too many collateral inputs (max 3)".to_string(),
            ));
        }

        Ok(())
    }

    fn validate_execution_units(tx: &AlonzoTransaction) -> Result<()> {
        let mut total_ex_units = ExUnits::new(0, 0);

        for redeemer in &tx.witness_set.redeemers {
            total_ex_units.add(&redeemer.ex_units)?;
        }

        // Check against protocol limits (simplified)
        let max_tx_ex_units = ExUnits::new(14_000_000, 10_000_000_000); // Example limits
        if !total_ex_units.within_budget(&max_tx_ex_units) {
            return Err(LedgerError::ScriptError(
                "Transaction exceeds execution unit limits".to_string(),
            ));
        }

        Ok(())
    }

    fn validate_plutus_scripts(_tx: &AlonzoTransaction) -> Result<()> {
        // In a full implementation, this would:
        // 1. Execute each Plutus script with its context
        // 2. Validate redeemer data matches script expectations
        // 3. Ensure scripts validate successfully within ex_units budget
        // 4. Check all required datums are present

        // For now, just validate script structure
        // TODO: Implement actual Plutus interpreter integration
        Ok(())
    }

    /// Build script context for a specific purpose
    pub fn build_script_context(tx: &AlonzoTransaction, purpose: ScriptPurpose) -> ScriptContext {
        let tx_info = TxInfo {
            inputs: vec![], // Would resolve from UTxO set
            outputs: tx.outputs.clone(),
            fee: MaryValue::new_ada_only(tx.fee),
            mint: tx
                .mint
                .as_ref()
                .map(|ma| MaryValue::new_with_assets(0, ma.clone()))
                .unwrap_or_else(|| MaryValue::new_ada_only(0)),
            dcert: tx.certificates.clone(),
            wdrl: tx
                .withdrawals
                .iter()
                .map(|(addr, coin)| (addr.clone(), *coin))
                .collect(),
            valid_range: tx.validity_interval.clone(),
            signatories: tx.required_signers.clone(),
            data: HashMap::new(), // Would be populated from witness set
            id: Blake2b256Hash::hash(&format!("{:?}", tx).as_bytes()), // Simplified
        };

        ScriptContext { tx_info, purpose }
    }

    /// Calculate minimum ADA for Alonzo output (includes datum cost)
    pub fn min_ada_for_output(output: &AlonzoTransactionOutput) -> Coin {
        let mut min_ada = 1_000_000; // Base minimum

        // Add cost for multi-assets
        if let Some(multi_asset) = &output.value.multi_asset {
            min_ada += multi_asset.asset_count() as u64 * 150_000;
        }

        // Add cost for datum
        match &output.datum {
            Some(Datum::DatumHash(_)) => min_ada += 100_000, // Small cost for hash
            Some(Datum::InlineDatum(data)) => {
                // Estimate cost based on data size (simplified)
                let estimated_size = format!("{:?}", data).len() as u64;
                min_ada += estimated_size * 44; // ~44 lovelace per byte
            }
            None => {} // No additional cost
        }

        min_ada
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plutus_data_creation() {
        let data = PlutusData::constr(
            0,
            vec![PlutusData::integer(42), PlutusData::bytes(vec![1, 2, 3, 4])],
        );

        match data {
            PlutusData::Constr(tag, fields) => {
                assert_eq!(tag, 0);
                assert_eq!(fields.len(), 2);
            }
            _ => panic!("Expected constructor"),
        }
    }

    #[test]
    fn test_ex_units_operations() {
        let mut units1 = ExUnits::new(1000, 5000);
        let units2 = ExUnits::new(500, 2000);

        units1.add(&units2).unwrap();
        assert_eq!(units1.mem, 1500);
        assert_eq!(units1.steps, 7000);

        let budget = ExUnits::new(2000, 10000);
        assert!(units1.within_budget(&budget));

        let small_budget = ExUnits::new(1000, 5000);
        assert!(!units1.within_budget(&small_budget));
    }

    #[test]
    fn test_plutus_script_creation() {
        let script = PlutusScript::v1(vec![1, 2, 3, 4, 5]);

        assert!(matches!(script.version, PlutusVersion::V1));
        assert_eq!(script.code, vec![1, 2, 3, 4, 5]);

        let hash = script.hash();
        assert_eq!(hash.as_bytes().len(), 32); // Blake2b-256 hash
    }

    #[test]
    fn test_datum_handling() {
        let data = PlutusData::integer(100);
        let datum = Datum::InlineDatum(data);

        match datum {
            Datum::InlineDatum(PlutusData::Integer(value)) => {
                assert_eq!(value, 100);
            }
            _ => panic!("Expected inline datum with integer"),
        }
    }

    #[test]
    fn test_min_ada_calculation_with_datum() {
        // Output without datum
        let simple_output = AlonzoTransactionOutput {
            address: Address {
                bytes: vec![1, 2, 3],
            },
            value: MaryValue::new_ada_only(1_000_000),
            datum: None,
            script_ref: None,
        };

        let min_ada_simple = AlonzoLedger::min_ada_for_output(&simple_output);
        assert_eq!(min_ada_simple, 1_000_000);

        // Output with datum hash
        let datum_hash_output = AlonzoTransactionOutput {
            address: Address {
                bytes: vec![1, 2, 3],
            },
            value: MaryValue::new_ada_only(1_000_000),
            datum: Some(Datum::DatumHash(Blake2b256Hash::hash(b"test"))),
            script_ref: None,
        };

        let min_ada_hash = AlonzoLedger::min_ada_for_output(&datum_hash_output);
        assert!(min_ada_hash > min_ada_simple);
        assert_eq!(min_ada_hash, 1_100_000); // Base + 100k for hash

        // Output with small inline datum (should cost less than hash due to small size)
        let small_inline = AlonzoTransactionOutput {
            address: Address {
                bytes: vec![1, 2, 3],
            },
            value: MaryValue::new_ada_only(1_000_000),
            datum: Some(Datum::InlineDatum(PlutusData::integer(42))),
            script_ref: None,
        };

        let min_ada_small_inline = AlonzoLedger::min_ada_for_output(&small_inline);
        // Small inline datum costs less than fixed hash cost
        assert!(min_ada_small_inline > min_ada_simple);
        assert!(min_ada_small_inline < min_ada_hash);
    }

    #[test]
    fn test_redeemer_validation() {
        let redeemer = Redeemer {
            tag: RedeemerTag::Spend,
            index: 0,
            data: PlutusData::integer(42),
            ex_units: ExUnits::new(1000, 5000),
        };

        assert!(matches!(redeemer.tag, RedeemerTag::Spend));
        assert_eq!(redeemer.index, 0);
        assert_eq!(redeemer.ex_units.mem, 1000);
    }
}
