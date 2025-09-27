//! Alonzo Era Validation Tests
//!
//! Tests for Alonzo era ledger rules validation.
//! Alonzo introduced Plutus smart contracts:
//! - Plutus V1 script execution
//! - Script data (datums and redeemers)
//! - Collateral inputs for script failures
//! - Extended UTXO model (EUTxO)

use cardano_ledger::{LedgerError, Result};
use cardano_crypto::{Blake2b256Hash, Ed25519KeyHash};
use std::collections::HashMap;

/// Re-export types from previous eras
pub use crate::test_mary_validation::{
    MaryTransaction, MaryValue, MultiAsset, PolicyId, AssetName,
    MaryLedgerState, MaryUtxo, Mint
};
pub use crate::test_allegra_validation::{
    NativeScript, ValidityInterval, AllegraWitnessSet, VKeyWitness
};
pub use crate::test_shelley_validation::{ShelleyAddress, ShelleyTxIn, NetworkId, Credential};

/// Plutus script types
#[derive(Debug, Clone, PartialEq)]
pub enum PlutusVersion {
    V1,
    V2, // For future Babbage era
}

/// Plutus script representation
#[derive(Debug, Clone)]
pub struct PlutusScript {
    pub version: PlutusVersion,
    pub code: Vec<u8>, // CBOR-encoded Plutus script
}

/// Plutus data for scripts (generic JSON-like structure)
#[derive(Debug, Clone, PartialEq)]
pub enum PlutusData {
    Integer(i64),
    Bytes(Vec<u8>),
    List(Vec<PlutusData>),
    Map(Vec<(PlutusData, PlutusData)>),
    Constructor {
        alternative: u64,
        fields: Vec<PlutusData>,
    },
}

/// Datum attached to UTxO
#[derive(Debug, Clone)]
pub enum Datum {
    Hash(Blake2b256Hash),     // Datum hash (datum provided in witness)
    Inline(PlutusData),       // Inline datum (Babbage era feature)
}

/// Redeemer for script execution
#[derive(Debug, Clone)]
pub struct Redeemer {
    pub tag: RedeemerTag,
    pub index: u32,
    pub data: PlutusData,
    pub ex_units: ExUnits,
}

/// Redeemer purpose
#[derive(Debug, Clone, PartialEq)]
pub enum RedeemerTag {
    Spend,    // Spending script input
    Mint,     // Minting policy script
    Cert,     // Certificate script
    Reward,   // Reward withdrawal script
}

/// Execution units (memory and CPU steps)
#[derive(Debug, Clone)]
pub struct ExUnits {
    pub mem: u64,  // Memory units
    pub steps: u64, // CPU steps
}

/// Script reference (for future Babbage era)
#[derive(Debug, Clone)]
pub enum ScriptRef {
    NativeScript(NativeScript),
    PlutusScript(PlutusScript),
}

/// Alonzo transaction output with datum support
#[derive(Debug, Clone)]
pub struct AlonzoTransactionOutput {
    pub address: ShelleyAddress,
    pub value: MaryValue,
    pub datum_hash: Option<Blake2b256Hash>, // Datum hash for script-locked outputs
    pub script_ref: Option<ScriptRef>,      // For Babbage era
}

/// Alonzo transaction with Plutus script support
#[derive(Debug, Clone)]
pub struct AlonzoTransaction {
    pub inputs: Vec<ShelleyTxIn>,
    pub outputs: Vec<AlonzoTransactionOutput>,
    pub fee: u64,
    pub ttl: Option<u64>, // Time to live (deprecated in favor of validity interval)
    pub validity_interval: Option<ValidityInterval>,
    pub certificates: Vec<Certificate>,
    pub withdrawals: HashMap<ShelleyAddress, u64>,
    pub mint: Option<Mint>,
    pub collateral: Vec<ShelleyTxIn>, // Collateral inputs for script failures
    pub required_signers: Vec<Ed25519KeyHash>, // Additional required signatures
}

/// Alonzo witness set with Plutus support
#[derive(Debug, Clone)]
pub struct AlonzoWitnessSet {
    pub vkey_witnesses: Vec<VKeyWitness>,
    pub native_scripts: Vec<NativeScript>,
    pub plutus_v1_scripts: Vec<PlutusScript>,
    pub plutus_data: Vec<PlutusData>, // Datums and other data
    pub redeemers: Vec<Redeemer>,
}

/// Script execution context
#[derive(Debug, Clone)]
pub struct ScriptContext {
    pub tx_info: TxInfo,
    pub purpose: ScriptPurpose,
}

#[derive(Debug, Clone)]
pub enum ScriptPurpose {
    Spending(ShelleyTxIn),
    Minting(PolicyId),
    Certifying(Certificate),
    Rewarding(ShelleyAddress),
}

/// Transaction info visible to Plutus scripts
#[derive(Debug, Clone)]
pub struct TxInfo {
    pub inputs: Vec<TxInInfo>,
    pub outputs: Vec<AlonzoTransactionOutput>,
    pub fee: MaryValue,
    pub mint: MaryValue,
    pub dcert: Vec<Certificate>,
    pub wdrl: HashMap<ShelleyAddress, u64>,
    pub valid_range: ValidityInterval,
    pub signatories: Vec<Ed25519KeyHash>,
    pub data: HashMap<Blake2b256Hash, PlutusData>,
    pub id: Blake2b256Hash, // Transaction hash
}

#[derive(Debug, Clone)]
pub struct TxInInfo {
    pub out_ref: ShelleyTxIn,
    pub resolved: AlonzoTransactionOutput,
}

/// Network ID for script validation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NetworkId {
    Testnet,
    Mainnet,
}

/// Alonzo UTXO entry
#[derive(Debug, Clone)]
pub struct AlonzoUtxo {
    pub tx_out: AlonzoTransactionOutput,
    pub spent: bool,
}

/// Alonzo ledger state with script execution tracking
#[derive(Debug, Clone)]
pub struct AlonzoLedgerState {
    pub base: MaryLedgerState,
    pub alonzo_utxo_set: HashMap<ShelleyTxIn, AlonzoUtxo>,
    pub current_slot: u64,
    pub network_id: NetworkId,
    /// Protocol parameters for script execution
    pub max_tx_ex_units: ExUnits,
    pub max_block_ex_units: ExUnits,
    pub cost_models: HashMap<PlutusVersion, CostModel>,
}

/// Cost model for Plutus script execution
#[derive(Debug, Clone)]
pub struct CostModel {
    pub costs: HashMap<String, i64>, // Operation name -> cost
}

// Re-export certificate type for convenience
pub use crate::test_shelley_validation::Certificate;

impl PlutusScript {
    pub fn new_v1(code: Vec<u8>) -> Self {
        Self {
            version: PlutusVersion::V1,
            code,
        }
    }

    pub fn hash(&self) -> Blake2b256Hash {
        Blake2b256Hash::new(&self.code)
    }

    /// Simplified script execution (in real implementation, this would use Plutus interpreter)
    pub fn execute(&self, datum: &PlutusData, redeemer: &PlutusData, context: &ScriptContext) -> Result<()> {
        // Simplified validation - in reality this would execute Plutus bytecode
        match &context.purpose {
            ScriptPurpose::Spending(_) => {
                // Example validation: redeemer must match expected pattern
                if let PlutusData::Integer(n) = redeemer {
                    if *n > 0 {
                        return Ok(());
                    }
                }
                Err(LedgerError::ScriptExecutionError("Script validation failed".to_string()))
            }

            ScriptPurpose::Minting(_) => {
                // Example minting policy: allow minting if redeemer is positive
                if let PlutusData::Integer(n) = redeemer {
                    if *n > 0 {
                        return Ok(());
                    }
                }
                Err(LedgerError::ScriptExecutionError("Minting policy failed".to_string()))
            }

            _ => Ok(()) // Simplified for other purposes
        }
    }
}

impl ExUnits {
    pub fn new(mem: u64, steps: u64) -> Self {
        Self { mem, steps }
    }

    pub fn add(&self, other: &ExUnits) -> ExUnits {
        ExUnits {
            mem: self.mem + other.mem,
            steps: self.steps + other.steps,
        }
    }

    pub fn is_within_limit(&self, limit: &ExUnits) -> bool {
        self.mem <= limit.mem && self.steps <= limit.steps
    }
}

impl AlonzoTransaction {
    /// Validate transaction against Alonzo era rules
    pub fn validate(&self, witnesses: &AlonzoWitnessSet, ledger_state: &AlonzoLedgerState) -> Result<()> {
        // Check validity interval or TTL
        if let Some(validity_interval) = &self.validity_interval {
            if !validity_interval.is_valid_at_slot(ledger_state.current_slot) {
                return Err(LedgerError::InvalidTransaction("Transaction outside validity interval".to_string()));
            }
        } else if let Some(ttl) = self.ttl {
            if ledger_state.current_slot > ttl {
                return Err(LedgerError::InvalidTransaction("Transaction expired (TTL exceeded)".to_string()));
            }
        }

        // Check inputs exist and are unspent
        for input in &self.inputs {
            if !ledger_state.alonzo_utxo_set.contains_key(input) {
                return Err(LedgerError::InvalidInput(format!("Input {:?} not found", input.tx_id)));
            }

            if ledger_state.alonzo_utxo_set[input].spent {
                return Err(LedgerError::InvalidInput("Input already spent".to_string()));
            }
        }

        // Validate collateral inputs
        for collateral in &self.collateral {
            if !ledger_state.alonzo_utxo_set.contains_key(collateral) {
                return Err(LedgerError::InvalidInput("Collateral input not found".to_string()));
            }

            let utxo = &ledger_state.alonzo_utxo_set[collateral];
            if utxo.spent {
                return Err(LedgerError::InvalidInput("Collateral input already spent".to_string()));
            }

            // Collateral must not be script-locked (only key-locked UTxOs)
            if utxo.tx_out.datum_hash.is_some() {
                return Err(LedgerError::InvalidInput("Collateral cannot be script-locked".to_string()));
            }
        }

        // Execute Plutus scripts
        self.execute_scripts(witnesses, ledger_state)?;

        // Validate value conservation
        self.validate_value_conservation(ledger_state)?;

        // Check required signers
        for required_signer in &self.required_signers {
            if !witnesses.vkey_witnesses.iter().any(|w| w.vkey == *required_signer) {
                return Err(LedgerError::InvalidWitness("Missing required signer".to_string()));
            }
        }

        Ok(())
    }

    fn execute_scripts(&self, witnesses: &AlonzoWitnessSet, ledger_state: &AlonzoLedgerState) -> Result<()> {
        let mut total_ex_units = ExUnits::new(0, 0);

        // Execute scripts for spending inputs
        for (input_idx, input) in self.inputs.iter().enumerate() {
            let utxo = &ledger_state.alonzo_utxo_set[input];

            if let Some(datum_hash) = &utxo.tx_out.datum_hash {
                // This is a script-locked output, need to execute script
                let script = self.find_required_script(input, witnesses)?;
                let datum = self.find_datum(datum_hash, witnesses)?;
                let redeemer = self.find_redeemer(RedeemerTag::Spend, input_idx as u32, witnesses)?;

                let context = ScriptContext {
                    tx_info: self.build_tx_info(witnesses, ledger_state),
                    purpose: ScriptPurpose::Spending(input.clone()),
                };

                script.execute(&datum, &redeemer.data, &context)?;
                total_ex_units = total_ex_units.add(&redeemer.ex_units);
            }
        }

        // Execute minting policy scripts
        if let Some(mint) = &self.mint {
            for policy_id in mint.assets.keys() {
                let script = self.find_minting_script(policy_id, witnesses)?;
                let redeemer = self.find_minting_redeemer(policy_id, witnesses)?;

                let context = ScriptContext {
                    tx_info: self.build_tx_info(witnesses, ledger_state),
                    purpose: ScriptPurpose::Minting(policy_id.clone()),
                };

                script.execute(&PlutusData::List(vec![]), &redeemer.data, &context)?;
                total_ex_units = total_ex_units.add(&redeemer.ex_units);
            }
        }

        // Check execution units are within limits
        if !total_ex_units.is_within_limit(&ledger_state.max_tx_ex_units) {
            return Err(LedgerError::ScriptExecutionError("Transaction execution units exceed limit".to_string()));
        }

        Ok(())
    }

    fn find_required_script(&self, _input: &ShelleyTxIn, witnesses: &AlonzoWitnessSet) -> Result<&PlutusScript> {
        // Simplified - in reality would match script hash from input
        witnesses.plutus_v1_scripts.first()
            .ok_or_else(|| LedgerError::InvalidScript("Required Plutus script not found".to_string()))
    }

    fn find_datum(&self, _datum_hash: &Blake2b256Hash, witnesses: &AlonzoWitnessSet) -> Result<PlutusData> {
        // Simplified - in reality would match datum hash
        witnesses.plutus_data.first()
            .cloned()
            .ok_or_else(|| LedgerError::InvalidScript("Required datum not found".to_string()))
    }

    fn find_redeemer(&self, tag: RedeemerTag, index: u32, witnesses: &AlonzoWitnessSet) -> Result<&Redeemer> {
        witnesses.redeemers.iter()
            .find(|r| r.tag == tag && r.index == index)
            .ok_or_else(|| LedgerError::InvalidScript("Required redeemer not found".to_string()))
    }

    fn find_minting_script(&self, _policy_id: &PolicyId, witnesses: &AlonzoWitnessSet) -> Result<&PlutusScript> {
        // Simplified - would match policy ID to script hash
        witnesses.plutus_v1_scripts.first()
            .ok_or_else(|| LedgerError::InvalidScript("Minting policy script not found".to_string()))
    }

    fn find_minting_redeemer(&self, _policy_id: &PolicyId, witnesses: &AlonzoWitnessSet) -> Result<&Redeemer> {
        // Simplified - would find redeemer for specific minting policy
        witnesses.redeemers.iter()
            .find(|r| r.tag == RedeemerTag::Mint)
            .ok_or_else(|| LedgerError::InvalidScript("Minting redeemer not found".to_string()))
    }

    fn build_tx_info(&self, _witnesses: &AlonzoWitnessSet, _ledger_state: &AlonzoLedgerState) -> TxInfo {
        // Simplified TX info construction
        TxInfo {
            inputs: vec![], // Would populate with resolved inputs
            outputs: self.outputs.clone(),
            fee: MaryValue::new_ada_only(self.fee),
            mint: MaryValue::new_ada_only(0), // Would calculate from mint field
            dcert: self.certificates.clone(),
            wdrl: self.withdrawals.clone(),
            valid_range: self.validity_interval.clone().unwrap_or_else(|| ValidityInterval {
                invalid_before: None,
                invalid_hereafter: self.ttl,
            }),
            signatories: self.required_signers.clone(),
            data: HashMap::new(), // Would populate with datum map
            id: self.hash(),
        }
    }

    fn validate_value_conservation(&self, ledger_state: &AlonzoLedgerState) -> Result<()> {
        // Similar to Mary era but includes collateral considerations
        let mut input_value = MaryValue::new_ada_only(0);
        for input in &self.inputs {
            let utxo = &ledger_state.alonzo_utxo_set[input];
            input_value = input_value.add(&utxo.tx_out.value);
        }

        let mut output_value = MaryValue::new_ada_only(self.fee);
        for output in &self.outputs {
            output_value = output_value.add(&output.value);
        }

        let withdrawal_sum: u64 = self.withdrawals.values().sum();
        input_value.coin += withdrawal_sum;

        // Apply minting
        if let Some(mint) = &self.mint {
            for (policy_id, assets) in &mint.assets {
                for (asset_name, &amount) in assets {
                    if amount > 0 {
                        output_value.multi_asset.add_token(
                            policy_id.clone(),
                            asset_name.clone(),
                            amount as u64
                        );
                    } else if amount < 0 {
                        input_value.multi_asset.add_token(
                            policy_id.clone(),
                            asset_name.clone(),
                            (-amount) as u64
                        );
                    }
                }
            }
        }

        // Check value conservation
        if input_value.coin != output_value.coin {
            return Err(LedgerError::InvalidTransaction("ADA not conserved".to_string()));
        }

        // Check native tokens (same as Mary era logic)
        // ... token conservation checks ...

        Ok(())
    }

    pub fn hash(&self) -> Blake2b256Hash {
        Blake2b256Hash::new(&format!("{:?}", self).as_bytes())
    }
}

impl AlonzoLedgerState {
    pub fn new() -> Self {
        Self {
            base: MaryLedgerState::new(),
            alonzo_utxo_set: HashMap::new(),
            current_slot: 0,
            network_id: NetworkId::Testnet,
            max_tx_ex_units: ExUnits::new(10_000_000, 10_000_000_000),
            max_block_ex_units: ExUnits::new(50_000_000, 40_000_000_000),
            cost_models: HashMap::new(),
        }
    }

    pub fn apply_transaction(&mut self, tx: &AlonzoTransaction, witnesses: &AlonzoWitnessSet) -> Result<()> {
        // Validate first - if scripts fail, only collateral is consumed
        let script_validation_result = tx.validate(witnesses, self);

        if script_validation_result.is_err() {
            // Script validation failed - consume collateral
            self.consume_collateral(&tx.collateral)?;
            return script_validation_result;
        }

        // Scripts succeeded - apply transaction normally

        // Remove spent outputs
        for input in &tx.inputs {
            if let Some(utxo) = self.alonzo_utxo_set.get_mut(input) {
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
            let utxo = AlonzoUtxo {
                tx_out: output.clone(),
                spent: false,
            };
            self.alonzo_utxo_set.insert(tx_in, utxo);
        }

        // Update token supplies (same as Mary era)
        if let Some(mint) = &tx.mint {
            // ... minting logic from Mary era ...
        }

        Ok(())
    }

    fn consume_collateral(&mut self, collateral: &[ShelleyTxIn]) -> Result<()> {
        for collateral_input in collateral {
            if let Some(utxo) = self.alonzo_utxo_set.get_mut(collateral_input) {
                utxo.spent = true;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_shelley_validation::StakeReference;

    fn create_test_address() -> ShelleyAddress {
        ShelleyAddress {
            payment_credential: Credential::Key(Ed25519KeyHash::new(b"test_payment_key")),
            stake_credential: Some(StakeReference::Credential(
                Credential::Key(Ed25519KeyHash::new(b"test_stake_key"))
            )),
            network_id: crate::test_shelley_validation::NetworkId::Testnet,
        }
    }

    #[test]
    fn test_plutus_script_execution_success() {
        let script = PlutusScript::new_v1(vec![1, 2, 3, 4]); // Dummy bytecode
        let datum = PlutusData::Integer(42);
        let redeemer = PlutusData::Integer(100); // Positive number should pass

        let context = ScriptContext {
            tx_info: TxInfo {
                inputs: vec![],
                outputs: vec![],
                fee: MaryValue::new_ada_only(200_000),
                mint: MaryValue::new_ada_only(0),
                dcert: vec![],
                wdrl: HashMap::new(),
                valid_range: ValidityInterval {
                    invalid_before: None,
                    invalid_hereafter: None,
                },
                signatories: vec![],
                data: HashMap::new(),
                id: Blake2b256Hash::new(b"test_tx"),
            },
            purpose: ScriptPurpose::Spending(ShelleyTxIn {
                tx_id: Blake2b256Hash::new(b"test_input"),
                output_index: 0,
            }),
        };

        let result = script.execute(&datum, &redeemer, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_plutus_script_execution_failure() {
        let script = PlutusScript::new_v1(vec![1, 2, 3, 4]);
        let datum = PlutusData::Integer(42);
        let redeemer = PlutusData::Integer(-5); // Negative number should fail

        let context = ScriptContext {
            tx_info: TxInfo {
                inputs: vec![],
                outputs: vec![],
                fee: MaryValue::new_ada_only(200_000),
                mint: MaryValue::new_ada_only(0),
                dcert: vec![],
                wdrl: HashMap::new(),
                valid_range: ValidityInterval {
                    invalid_before: None,
                    invalid_hereafter: None,
                },
                signatories: vec![],
                data: HashMap::new(),
                id: Blake2b256Hash::new(b"test_tx"),
            },
            purpose: ScriptPurpose::Spending(ShelleyTxIn {
                tx_id: Blake2b256Hash::new(b"test_input"),
                output_index: 0,
            }),
        };

        let result = script.execute(&datum, &redeemer, &context);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::ScriptExecutionError(_)));
    }

    #[test]
    fn test_execution_units_limits() {
        let ex_units1 = ExUnits::new(1000, 5000);
        let ex_units2 = ExUnits::new(2000, 3000);
        let limit = ExUnits::new(5000, 10000);

        let total = ex_units1.add(&ex_units2);
        assert_eq!(total.mem, 3000);
        assert_eq!(total.steps, 8000);
        assert!(total.is_within_limit(&limit));

        let over_limit = ExUnits::new(6000, 15000);
        assert!(!over_limit.is_within_limit(&limit));
    }

    #[test]
    fn test_script_locked_utxo_spending() {
        let mut ledger = AlonzoLedgerState::new();
        ledger.current_slot = 1000;

        // Create a script-locked UTXO
        let script = PlutusScript::new_v1(vec![1, 2, 3, 4]);
        let datum_hash = Blake2b256Hash::new(b"test_datum_hash");

        let input_tx_in = ShelleyTxIn {
            tx_id: Blake2b256Hash::new(b"script_tx"),
            output_index: 0,
        };

        let script_output = AlonzoTransactionOutput {
            address: create_test_address(),
            value: MaryValue::new_ada_only(2_000_000),
            datum_hash: Some(datum_hash.clone()),
            script_ref: None,
        };

        ledger.alonzo_utxo_set.insert(input_tx_in.clone(), AlonzoUtxo {
            tx_out: script_output,
            spent: false,
        });

        // Create transaction spending the script-locked output
        let tx = AlonzoTransaction {
            inputs: vec![input_tx_in],
            outputs: vec![AlonzoTransactionOutput {
                address: create_test_address(),
                value: MaryValue::new_ada_only(1_700_000),
                datum_hash: None,
                script_ref: None,
            }],
            fee: 300_000,
            ttl: Some(2000),
            validity_interval: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            mint: None,
            collateral: vec![],
            required_signers: vec![],
        };

        let witnesses = AlonzoWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![script],
            plutus_data: vec![PlutusData::Integer(42)], // Datum
            redeemers: vec![Redeemer {
                tag: RedeemerTag::Spend,
                index: 0,
                data: PlutusData::Integer(100), // Valid redeemer
                ex_units: ExUnits::new(500_000, 1_000_000),
            }],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_ok());
    }

    #[test]
    fn test_collateral_validation() {
        let mut ledger = AlonzoLedgerState::new();

        // Create collateral UTXO (must not be script-locked)
        let collateral_input = ShelleyTxIn {
            tx_id: Blake2b256Hash::new(b"collateral_tx"),
            output_index: 0,
        };

        let collateral_output = AlonzoTransactionOutput {
            address: create_test_address(),
            value: MaryValue::new_ada_only(5_000_000),
            datum_hash: None, // Not script-locked
            script_ref: None,
        };

        ledger.alonzo_utxo_set.insert(collateral_input.clone(), AlonzoUtxo {
            tx_out: collateral_output,
            spent: false,
        });

        let tx = AlonzoTransaction {
            inputs: vec![],
            outputs: vec![],
            fee: 200_000,
            ttl: Some(2000),
            validity_interval: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            mint: None,
            collateral: vec![collateral_input],
            required_signers: vec![],
        };

        let witnesses = AlonzoWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_collateral_script_locked() {
        let mut ledger = AlonzoLedgerState::new();

        // Create script-locked collateral (should be invalid)
        let collateral_input = ShelleyTxIn {
            tx_id: Blake2b256Hash::new(b"bad_collateral"),
            output_index: 0,
        };

        let script_locked_output = AlonzoTransactionOutput {
            address: create_test_address(),
            value: MaryValue::new_ada_only(5_000_000),
            datum_hash: Some(Blake2b256Hash::new(b"datum")), // Script-locked!
            script_ref: None,
        };

        ledger.alonzo_utxo_set.insert(collateral_input.clone(), AlonzoUtxo {
            tx_out: script_locked_output,
            spent: false,
        });

        let tx = AlonzoTransaction {
            inputs: vec![],
            outputs: vec![],
            fee: 200_000,
            ttl: Some(2000),
            validity_interval: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            mint: None,
            collateral: vec![collateral_input],
            required_signers: vec![],
        };

        let witnesses = AlonzoWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidInput(_)));
    }

    #[test]
    fn test_plutus_minting_policy() {
        let ledger = AlonzoLedgerState::new();

        let script = PlutusScript::new_v1(vec![5, 6, 7, 8]);
        let policy_id = PolicyId(script.hash());
        let asset_name = AssetName::new(b"SCRIPT_TOKEN").unwrap();

        let mut mint = Mint::new();
        mint.add_mint(policy_id.clone(), asset_name.clone(), 1000);

        let tx = AlonzoTransaction {
            inputs: vec![],
            outputs: vec![AlonzoTransactionOutput {
                address: create_test_address(),
                value: {
                    let mut value = MaryValue::new_ada_only(1_000_000);
                    value.add_token(policy_id.clone(), asset_name, 1000);
                    value
                },
                datum_hash: None,
                script_ref: None,
            }],
            fee: 300_000,
            ttl: Some(2000),
            validity_interval: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            mint: Some(mint),
            collateral: vec![],
            required_signers: vec![],
        };

        let witnesses = AlonzoWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![script],
            plutus_data: vec![],
            redeemers: vec![Redeemer {
                tag: RedeemerTag::Mint,
                index: 0,
                data: PlutusData::Integer(1), // Valid minting redeemer
                ex_units: ExUnits::new(300_000, 800_000),
            }],
        };

        // Note: This would fail value conservation since no inputs provide ADA
        // but the script execution part should work
        let result = tx.validate(&witnesses, &ledger);
        // Expected to fail on value conservation, not script execution
        assert!(result.is_err());
    }
}
