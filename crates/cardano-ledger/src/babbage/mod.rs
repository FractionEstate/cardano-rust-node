//! Babbage Era Ledger Rules
//!
//! The Babbage era (September 2022) introduced Plutus V2 and several important
//! improvements to smart contract functionality. Building on Alonzo, it adds:
//! - Plutus V2 scripts with improved efficiency
//! - Reference inputs (read-only inputs)
//! - Inline datums (no longer need datum hash)
//! - Reference scripts (scripts attached to outputs)
//! - Collateral return outputs
//! - Improved script validation context

use crate::{LedgerError, Result};
use crate::mary::{MaryValue, MultiAsset, Coin, Slot, Address, RewardAddress, Certificate, ValidityInterval, Ed25519KeyHash};
use crate::alonzo::{
    PlutusScript, PlutusVersion, PlutusData, NativeScript,
    ExUnits, RedeemerTag, NetworkId, ScriptPurpose
};
use cardano_crypto::Blake2b256Hash;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Babbage Era Transaction with Plutus V2 and reference input support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BabbageTransaction {
    pub inputs: Vec<TransactionInput>,
    pub outputs: Vec<BabbageTransactionOutput>,
    pub fee: Coin,
    pub ttl: Option<Slot>,
    pub certificates: Vec<Certificate>,
    pub withdrawals: HashMap<RewardAddress, Coin>,
    pub auxiliary_data: Option<AuxiliaryData>,
    pub validity_interval: ValidityInterval,
    pub mint: Option<MultiAsset>,
    pub script_data_hash: Option<Blake2b256Hash>,
    pub collateral: Vec<TransactionInput>,
    pub required_signers: Vec<Ed25519KeyHash>,
    pub network_id: Option<NetworkId>,
    pub collateral_return: Option<BabbageTransactionOutput>, // New: collateral return
    pub total_collateral: Option<Coin>, // New: total collateral amount
    pub reference_inputs: Vec<TransactionInput>, // New: read-only inputs
    pub witness_set: BabbageWitnessSet,
}

/// Babbage Transaction Output with inline datum and script reference support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BabbageTransactionOutput {
    pub address: Address,
    pub value: MaryValue,
    pub datum: Option<OutputDatum>, // Enhanced datum support
    pub script_ref: Option<ScriptReference>, // Enhanced script reference
}

/// Enhanced datum that can be hash or inline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputDatum {
    /// Hash of the datum (traditional approach)
    DatumHash(Blake2b256Hash),
    /// Inline datum stored in the output (Babbage feature)
    InlineDatum(PlutusData),
}

/// Enhanced script reference with version support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScriptReference {
    NativeScript(NativeScript),
    PlutusV1Script(PlutusScript),
    PlutusV2Script(PlutusScript), // New: Plutus V2 support
}

/// Enhanced redeemer with pointer to reference scripts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BabbageRedeemer {
    pub tag: RedeemerTag,
    pub index: u32,
    pub data: PlutusData,
    pub ex_units: ExUnits,
}

/// Babbage witness set with Plutus V2 script support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BabbageWitnessSet {
    pub vkey_witnesses: Vec<VKeyWitness>,
    pub native_scripts: Vec<NativeScript>,
    pub bootstrap_witnesses: Vec<BootstrapWitness>,
    pub plutus_v1_scripts: Vec<PlutusScript>,
    pub plutus_v2_scripts: Vec<PlutusScript>, // New: Plutus V2 scripts
    pub plutus_data: Vec<PlutusData>,
    pub redeemers: Vec<BabbageRedeemer>,
}

// Re-exported types
pub use crate::mary::{TransactionInput, AuxiliaryData};
pub use crate::alonzo::{VKeyWitness, BootstrapWitness};

/// Enhanced script context with reference inputs and improved datum access
#[derive(Debug, Clone)]
pub struct BabbageScriptContext {
    pub tx_info: BabbageTxInfo,
    pub purpose: ScriptPurpose,
}

/// Enhanced transaction info with reference inputs
#[derive(Debug, Clone)]
pub struct BabbageTxInfo {
    pub inputs: Vec<BabbageTxInInfo>,
    pub reference_inputs: Vec<BabbageTxInInfo>, // New: read-only inputs
    pub outputs: Vec<BabbageTransactionOutput>,
    pub fee: MaryValue,
    pub mint: MaryValue,
    pub dcert: Vec<Certificate>,
    pub wdrl: Vec<(RewardAddress, Coin)>,
    pub valid_range: ValidityInterval,
    pub signatories: Vec<Ed25519KeyHash>,
    pub redeemers: HashMap<ScriptPurpose, BabbageRedeemer>,
    pub data: HashMap<Blake2b256Hash, PlutusData>, // Available datums
    pub id: Blake2b256Hash,
}

/// Enhanced transaction input info with inline datum support
#[derive(Debug, Clone)]
pub struct BabbageTxInInfo {
    pub out_ref: TransactionInput,
    pub resolved: BabbageTransactionOutput,
}

/// Cost model parameters for Plutus V2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostModel {
    pub parameters: Vec<i64>, // Cost model parameters
}

impl BabbageTransactionOutput {
    /// Create new output with ADA only
    pub fn new_ada_only(address: Address, amount: Coin) -> Self {
        Self {
            address,
            value: MaryValue::new_ada_only(amount),
            datum: None,
            script_ref: None,
        }
    }

    /// Create new output with multi-assets
    pub fn new_with_assets(address: Address, value: MaryValue) -> Self {
        Self {
            address,
            value,
            datum: None,
            script_ref: None,
        }
    }

    /// Add inline datum to output
    pub fn with_inline_datum(mut self, datum: PlutusData) -> Self {
        self.datum = Some(OutputDatum::InlineDatum(datum));
        self
    }

    /// Add datum hash to output
    pub fn with_datum_hash(mut self, hash: Blake2b256Hash) -> Self {
        self.datum = Some(OutputDatum::DatumHash(hash));
        self
    }

    /// Add script reference to output
    pub fn with_script_ref(mut self, script: ScriptReference) -> Self {
        self.script_ref = Some(script);
        self
    }

    /// Check if output contains a script
    pub fn has_script(&self) -> bool {
        self.script_ref.is_some()
    }

    /// Check if output has inline datum
    pub fn has_inline_datum(&self) -> bool {
        matches!(self.datum, Some(OutputDatum::InlineDatum(_)))
    }

    /// Get the datum if it's inline
    pub fn inline_datum(&self) -> Option<&PlutusData> {
        match &self.datum {
            Some(OutputDatum::InlineDatum(data)) => Some(data),
            _ => None,
        }
    }
}

impl OutputDatum {
    /// Get hash of this datum
    pub fn hash(&self) -> Blake2b256Hash {
        match self {
            Self::DatumHash(hash) => *hash,
            Self::InlineDatum(data) => data.hash(),
        }
    }

    /// Check if this is an inline datum
    pub fn is_inline(&self) -> bool {
        matches!(self, Self::InlineDatum(_))
    }
}

impl ScriptReference {
    /// Get the script hash for this reference
    pub fn hash(&self) -> Blake2b256Hash {
        match self {
            Self::NativeScript(script) => {
                // Native script hash calculation (simplified)
                Blake2b256Hash::hash(&format!("{:?}", script).as_bytes())
            }
            Self::PlutusV1Script(script) | Self::PlutusV2Script(script) => {
                script.hash()
            }
        }
    }

    /// Check if this is a Plutus script
    pub fn is_plutus_script(&self) -> bool {
        matches!(self, Self::PlutusV1Script(_) | Self::PlutusV2Script(_))
    }

    /// Get Plutus version if applicable
    pub fn plutus_version(&self) -> Option<PlutusVersion> {
        match self {
            Self::PlutusV1Script(_) => Some(PlutusVersion::V1),
            Self::PlutusV2Script(_) => Some(PlutusVersion::V2),
            Self::NativeScript(_) => None,
        }
    }
}

/// Babbage era ledger validation
pub struct BabbageLedger;

impl BabbageLedger {
    /// Validate a Babbage era transaction
    pub fn validate_transaction(tx: &BabbageTransaction) -> Result<()> {
        // 1. Validate basic structure
        Self::validate_structure(tx)?;

        // 2. Validate reference inputs
        Self::validate_reference_inputs(tx)?;

        // 3. Validate collateral return
        Self::validate_collateral_return(tx)?;

        // 4. Validate script references
        Self::validate_script_references(tx)?;

        // 5. Validate inline datums
        Self::validate_inline_datums(tx)?;

        // 6. Validate Plutus V2 scripts (if any)
        Self::validate_plutus_v2_scripts(tx)?;

        Ok(())
    }

    fn validate_structure(tx: &BabbageTransaction) -> Result<()> {
        if tx.inputs.is_empty() {
            return Err(LedgerError::InvalidTransaction(
                "Transaction must have at least one input".to_string()
            ));
        }

        if tx.outputs.is_empty() {
            return Err(LedgerError::InvalidTransaction(
                "Transaction must have at least one output".to_string()
            ));
        }

        // Reference inputs can be empty
        // Validate no overlap between regular inputs and reference inputs
        for ref_input in &tx.reference_inputs {
            if tx.inputs.contains(ref_input) {
                return Err(LedgerError::InvalidTransaction(
                    "Input cannot be both regular and reference input".to_string()
                ));
            }
        }

        Ok(())
    }

    fn validate_reference_inputs(tx: &BabbageTransaction) -> Result<()> {
        // Reference inputs must not be spent elsewhere in the transaction
        for ref_input in &tx.reference_inputs {
            if tx.inputs.contains(ref_input) {
                return Err(LedgerError::InvalidTransaction(
                    "Reference input cannot also be a regular input".to_string()
                ));
            }
        }

        // Check for duplicates in reference inputs
        let mut seen = std::collections::HashSet::new();
        for ref_input in &tx.reference_inputs {
            let key = (&ref_input.transaction_id, ref_input.index);
            if !seen.insert(key) {
                return Err(LedgerError::InvalidTransaction(
                    "Duplicate reference input".to_string()
                ));
            }
        }

        Ok(())
    }

    fn validate_collateral_return(tx: &BabbageTransaction) -> Result<()> {
        match (&tx.collateral_return, &tx.total_collateral) {
            (Some(_), None) => {
                return Err(LedgerError::InvalidTransaction(
                    "Collateral return specified but total collateral missing".to_string()
                ));
            }
            (None, Some(_)) => {
                return Err(LedgerError::InvalidTransaction(
                    "Total collateral specified but collateral return missing".to_string()
                ));
            }
            (Some(collateral_return), Some(total_collateral)) => {
                // Validate collateral return value is less than total collateral
                if collateral_return.value.coin >= *total_collateral {
                    return Err(LedgerError::InvalidTransaction(
                        "Collateral return must be less than total collateral".to_string()
                    ));
                }

                // Collateral return must not have multi-assets (ADA only)
                if collateral_return.value.multi_asset.is_some() {
                    return Err(LedgerError::InvalidTransaction(
                        "Collateral return cannot contain multi-assets".to_string()
                    ));
                }

                // Collateral return must not have datum or script reference
                if collateral_return.datum.is_some() || collateral_return.script_ref.is_some() {
                    return Err(LedgerError::InvalidTransaction(
                        "Collateral return cannot have datum or script reference".to_string()
                    ));
                }
            }
            (None, None) => {} // Both missing is valid
        }

        Ok(())
    }

    fn validate_script_references(tx: &BabbageTransaction) -> Result<()> {
        // Validate that script references are properly structured
        for output in &tx.outputs {
            if let Some(script_ref) = &output.script_ref {
                match script_ref {
                    ScriptReference::PlutusV1Script(script) => {
                        if !matches!(script.version, PlutusVersion::V1) {
                            return Err(LedgerError::ScriptError(
                                "PlutusV1Script must have V1 version".to_string()
                            ));
                        }
                    }
                    ScriptReference::PlutusV2Script(script) => {
                        if !matches!(script.version, PlutusVersion::V2) {
                            return Err(LedgerError::ScriptError(
                                "PlutusV2Script must have V2 version".to_string()
                            ));
                        }
                    }
                    ScriptReference::NativeScript(_) => {} // Always valid
                }
            }
        }

        Ok(())
    }

    fn validate_inline_datums(_tx: &BabbageTransaction) -> Result<()> {
        // Inline datums are always valid structurally
        // Semantic validation would happen during script execution
        Ok(())
    }

    fn validate_plutus_v2_scripts(_tx: &BabbageTransaction) -> Result<()> {
        // In a full implementation, this would:
        // 1. Execute Plutus V2 scripts with enhanced context
        // 2. Validate new Plutus V2 built-ins work correctly
        // 3. Check cost model parameters are valid
        // 4. Ensure reference scripts are accessible

        // TODO: Implement Plutus V2 interpreter integration
        Ok(())
    }

    /// Build enhanced script context for Babbage era
    pub fn build_script_context(
        tx: &BabbageTransaction,
        purpose: ScriptPurpose,
        _utxo_set: &HashMap<TransactionInput, BabbageTransactionOutput>, // Would use for resolving
    ) -> BabbageScriptContext {
        let tx_info = BabbageTxInfo {
            inputs: vec![], // Would resolve from UTxO set
            reference_inputs: vec![], // Would resolve from UTxO set
            outputs: tx.outputs.clone(),
            fee: MaryValue::new_ada_only(tx.fee),
            mint: tx.mint.as_ref()
                .map(|ma| MaryValue::new_with_assets(0, ma.clone()))
                .unwrap_or_else(|| MaryValue::new_ada_only(0)),
            dcert: tx.certificates.clone(),
            wdrl: tx.withdrawals.iter().map(|(addr, coin)| (addr.clone(), *coin)).collect(),
            valid_range: tx.validity_interval.clone(),
            signatories: tx.required_signers.clone(),
            redeemers: HashMap::new(), // Would be populated from witness set
            data: HashMap::new(), // Would extract from witness set and inline datums
            id: Blake2b256Hash::hash(&format!("{:?}", tx).as_bytes()), // Simplified
        };

        BabbageScriptContext {
            tx_info,
            purpose,
        }
    }

    /// Calculate minimum ADA for Babbage output (includes all features)
    pub fn min_ada_for_output(output: &BabbageTransactionOutput) -> Coin {
        let mut min_ada = 1_000_000; // Base minimum (1 ADA)

        // Add cost for multi-assets
        if let Some(multi_asset) = &output.value.multi_asset {
            min_ada += multi_asset.asset_count() as u64 * 150_000;
        }

        // Add cost for datum
        match &output.datum {
            Some(OutputDatum::DatumHash(_)) => {
                min_ada += 100_000; // Small cost for hash
            }
            Some(OutputDatum::InlineDatum(data)) => {
                // Estimate cost based on serialized size
                let estimated_size = Self::estimate_plutus_data_size(data);
                min_ada += estimated_size * 44; // ~44 lovelace per byte
            }
            None => {} // No additional cost
        }

        // Add cost for script reference
        if let Some(script_ref) = &output.script_ref {
            let script_cost = match script_ref {
                ScriptReference::NativeScript(_) => 200_000, // ~0.2 ADA for native scripts
                ScriptReference::PlutusV1Script(script) => {
                    // Cost based on script size
                    500_000 + (script.code.len() as u64 * 44)
                }
                ScriptReference::PlutusV2Script(script) => {
                    // Cost based on script size
                    500_000 + (script.code.len() as u64 * 44)
                }
            };
            min_ada += script_cost;
        }

        min_ada
    }

    /// Estimate serialized size of PlutusData (simplified)
    fn estimate_plutus_data_size(data: &PlutusData) -> u64 {
        match data {
            PlutusData::Integer(_) => 8, // Rough estimate
            PlutusData::Bytes(bytes) => bytes.len() as u64 + 4,
            PlutusData::List(items) => {
                4 + items.iter().map(|item| Self::estimate_plutus_data_size(item)).sum::<u64>()
            }
            PlutusData::Map(pairs) => {
                4 + pairs.iter()
                    .map(|(k, v)| Self::estimate_plutus_data_size(k) + Self::estimate_plutus_data_size(v))
                    .sum::<u64>()
            }
            PlutusData::Constr(_, fields) => {
                8 + fields.iter().map(|field| Self::estimate_plutus_data_size(field)).sum::<u64>()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_babbage_output_creation() {
        let addr = Address { bytes: vec![1, 2, 3] };
        let output = BabbageTransactionOutput::new_ada_only(addr, 2_000_000);

        assert_eq!(output.value.coin, 2_000_000);
        assert!(output.datum.is_none());
        assert!(output.script_ref.is_none());
    }

    #[test]
    fn test_inline_datum_support() {
        let addr = Address { bytes: vec![1, 2, 3] };
        let datum = PlutusData::integer(42);

        let output = BabbageTransactionOutput::new_ada_only(addr, 2_000_000)
            .with_inline_datum(datum);

        assert!(output.has_inline_datum());
        assert_eq!(output.inline_datum(), Some(&PlutusData::integer(42)));
    }

    #[test]
    fn test_script_reference_support() {
        let addr = Address { bytes: vec![1, 2, 3] };
        let script = PlutusScript::v2(vec![1, 2, 3, 4, 5]);
        let script_ref = ScriptReference::PlutusV2Script(script);

        let output = BabbageTransactionOutput::new_ada_only(addr, 2_000_000)
            .with_script_ref(script_ref);

        assert!(output.has_script());

        if let Some(ScriptReference::PlutusV2Script(script)) = &output.script_ref {
            assert!(matches!(script.version, PlutusVersion::V2));
        } else {
            panic!("Expected Plutus V2 script reference");
        }
    }

    #[test]
    fn test_output_datum_types() {
        let hash = Blake2b256Hash::hash(b"test");
        let hash_datum = OutputDatum::DatumHash(hash);
        let inline_datum = OutputDatum::InlineDatum(PlutusData::integer(100));

        assert!(!hash_datum.is_inline());
        assert!(inline_datum.is_inline());
        assert_eq!(hash_datum.hash(), hash);
    }

    #[test]
    fn test_script_reference_properties() {
        let v2_script = PlutusScript::v2(vec![1, 2, 3]);
        let script_ref = ScriptReference::PlutusV2Script(v2_script);

        assert!(script_ref.is_plutus_script());
        assert_eq!(script_ref.plutus_version(), Some(PlutusVersion::V2));

        let native_script = NativeScript::ScriptPubkey(Blake2b256Hash::hash(b"key"));
        let native_ref = ScriptReference::NativeScript(native_script);

        assert!(!native_ref.is_plutus_script());
        assert_eq!(native_ref.plutus_version(), None);
    }

    #[test]
    fn test_min_ada_calculation_babbage() {
        let addr = Address { bytes: vec![1, 2, 3] };

        // Simple ADA-only output
        let simple_output = BabbageTransactionOutput::new_ada_only(addr.clone(), 1_000_000);
        let min_simple = BabbageLedger::min_ada_for_output(&simple_output);
        assert_eq!(min_simple, 1_000_000);

        // Output with inline datum
        let inline_output = BabbageTransactionOutput::new_ada_only(addr.clone(), 1_000_000)
            .with_inline_datum(PlutusData::integer(42));
        let min_inline = BabbageLedger::min_ada_for_output(&inline_output);
        assert!(min_inline > min_simple);

        // Output with script reference
        let script = PlutusScript::v2(vec![0; 100]); // 100-byte script
        let script_output = BabbageTransactionOutput::new_ada_only(addr, 1_000_000)
            .with_script_ref(ScriptReference::PlutusV2Script(script));
        let min_script = BabbageLedger::min_ada_for_output(&script_output);
        assert!(min_script > min_inline);
    }

    #[test]
    fn test_reference_input_validation() {
        let input1 = TransactionInput {
            transaction_id: Blake2b256Hash::hash(b"tx1"),
            index: 0,
        };
        let input2 = TransactionInput {
            transaction_id: Blake2b256Hash::hash(b"tx2"),
            index: 0,
        };

        let mut tx = BabbageTransaction {
            inputs: vec![input1.clone()],
            outputs: vec![BabbageTransactionOutput::new_ada_only(
                Address { bytes: vec![1, 2, 3] },
                1_000_000
            )],
            fee: 200_000,
            ttl: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            auxiliary_data: None,
            validity_interval: ValidityInterval {
                invalid_before: None,
                invalid_hereafter: None,
            },
            mint: None,
            script_data_hash: None,
            collateral: vec![],
            required_signers: vec![],
            network_id: None,
            collateral_return: None,
            total_collateral: None,
            reference_inputs: vec![input2],
            witness_set: BabbageWitnessSet {
                vkey_witnesses: vec![],
                native_scripts: vec![],
                bootstrap_witnesses: vec![],
                plutus_v1_scripts: vec![],
                plutus_v2_scripts: vec![],
                plutus_data: vec![],
                redeemers: vec![],
            },
        };

        // Valid transaction - no overlap between inputs and reference inputs
        assert!(BabbageLedger::validate_transaction(&tx).is_ok());

        // Invalid - same input used as both regular and reference input
        tx.reference_inputs = vec![input1];
        assert!(BabbageLedger::validate_transaction(&tx).is_err());
    }
}
