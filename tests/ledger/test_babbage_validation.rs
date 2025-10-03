//! Babbage Era Validation Tests
//!
//! Tests for Babbage era ledger rules validation.
//! Babbage introduced Plutus V2 and several important improvements:
//! - Plutus V2 scripts with improved efficiency
//! - Reference inputs (read-only inputs)
//! - Inline datums (no longer need datum hash)
//! - Reference scripts (scripts attached to outputs)
//! - Collateral return outputs

use cardano_ledger::{LedgerError, Result};
use cardano_crypto::{Blake2b256Hash, Ed25519KeyHash};
use std::collections::HashMap;

/// Re-export types from previous eras
pub use crate::test_alonzo_validation::{
    AlonzoTransaction, AlonzoTransactionOutput, AlonzoWitnessSet,
    PlutusScript, PlutusVersion, PlutusData, ScriptContext, ScriptPurpose,
    TxInfo, RedeemerTag, Redeemer, ExUnits, NetworkId, AlonzoLedgerState
};
pub use crate::test_mary_validation::{MaryValue, MultiAsset, PolicyId, AssetName, Mint};
pub use crate::test_allegra_validation::{NativeScript, ValidityInterval, VKeyWitness};
pub use crate::test_shelley_validation::{ShelleyAddress, ShelleyTxIn, Credential};

/// Enhanced datum support with inline datums
#[derive(Debug, Clone)]
pub enum OutputDatum {
    DatumHash(Blake2b256Hash),  // Traditional datum hash
    InlineDatum(PlutusData),    // New: datum embedded in output
}

/// Reference script attached to outputs
#[derive(Debug, Clone)]
pub enum ScriptReference {
    NativeScript(NativeScript),
    PlutusV1Script(PlutusScript),
    PlutusV2Script(PlutusScript), // New in Babbage
}

/// Babbage transaction output with enhanced features
#[derive(Debug, Clone)]
pub struct BabbageTransactionOutput {
    pub address: ShelleyAddress,
    pub value: MaryValue,
    pub datum: Option<OutputDatum>,        // Enhanced datum support
    pub script_ref: Option<ScriptReference>, // Reference script
}

/// Babbage transaction with Plutus V2 and reference features
#[derive(Debug, Clone)]
pub struct BabbageTransaction {
    pub inputs: Vec<ShelleyTxIn>,
    pub reference_inputs: Vec<ShelleyTxIn>, // New: read-only reference inputs
    pub outputs: Vec<BabbageTransactionOutput>,
    pub collateral: Vec<ShelleyTxIn>,
    pub collateral_return: Option<BabbageTransactionOutput>, // New: return change from collateral
    pub total_collateral: Option<u64>,     // New: total collateral amount
    pub fee: u64,
    pub validity_interval: Option<ValidityInterval>,
    pub certificates: Vec<Certificate>,
    pub withdrawals: HashMap<ShelleyAddress, u64>,
    pub mint: Option<Mint>,
    pub required_signers: Vec<Ed25519KeyHash>,
    pub network_id: Option<NetworkId>,      // New: explicit network ID
}

/// Enhanced witness set for Babbage
#[derive(Debug, Clone)]
pub struct BabbageWitnessSet {
    pub vkey_witnesses: Vec<VKeyWitness>,
    pub native_scripts: Vec<NativeScript>,
    pub plutus_v1_scripts: Vec<PlutusScript>,
    pub plutus_v2_scripts: Vec<PlutusScript>, // New: Plutus V2 scripts
    pub plutus_data: Vec<PlutusData>,
    pub redeemers: Vec<BabbageRedeemer>, // Enhanced redeemers
}

/// Enhanced redeemer with ex units budget
#[derive(Debug, Clone)]
pub struct BabbageRedeemer {
    pub tag: RedeemerTag,
    pub index: u32,
    pub data: PlutusData,
    pub ex_units: ExUnits,
}

/// Babbage UTXO entry
#[derive(Debug, Clone)]
pub struct BabbageUtxo {
    pub tx_out: BabbageTransactionOutput,
    pub spent: bool,
}

/// Enhanced ledger state for Babbage era
#[derive(Debug, Clone)]
pub struct BabbageLedgerState {
    pub base: AlonzoLedgerState,
    pub babbage_utxo_set: HashMap<ShelleyTxIn, BabbageUtxo>,
    pub current_slot: u64,
    pub network_id: NetworkId,
    /// Protocol parameters for Plutus V2
    pub plutus_v2_cost_model: HashMap<String, i64>,
    pub max_collateral_inputs: u32,
}

// Re-export certificate type
pub use crate::test_shelley_validation::Certificate;

impl PlutusScript {
    pub fn new_v2(code: Vec<u8>) -> Self {
        Self {
            version: PlutusVersion::V2,
            code,
        }
    }

    /// Enhanced script execution for Plutus V2 with improved context
    pub fn execute_v2(&self, datum: Option<&PlutusData>, redeemer: &PlutusData, context: &ScriptContext) -> Result<()> {
        match self.version {
            PlutusVersion::V1 => {
                // V1 requires datum
                let datum = datum.ok_or_else(||
                    LedgerError::ScriptExecutionError("Plutus V1 requires datum".to_string()))?;
                self.execute(datum, redeemer, context)
            }

            PlutusVersion::V2 => {
                // V2 can work without datum and has enhanced validation
                match &context.purpose {
                    ScriptPurpose::Spending(_) => {
                        // Enhanced validation logic for V2
                        if let PlutusData::Constructor { alternative, .. } = redeemer {
                            if *alternative < 10 {  // Example validation
                                return Ok(());
                            }
                        }
                        Err(LedgerError::ScriptExecutionError("Plutus V2 validation failed".to_string()))
                    }

                    ScriptPurpose::Minting(_) => {
                        // V2 minting policies can see reference inputs
                        if let PlutusData::List(ref items) = redeemer {
                            if !items.is_empty() {
                                return Ok(());
                            }
                        }
                        Err(LedgerError::ScriptExecutionError("V2 minting policy failed".to_string()))
                    }

                    _ => Ok(())
                }
            }
        }
    }
}

impl OutputDatum {
    /// Get the datum data if inline, or error if hash-based
    pub fn get_inline_datum(&self) -> Option<&PlutusData> {
        match self {
            OutputDatum::InlineDatum(data) => Some(data),
            OutputDatum::DatumHash(_) => None,
        }
    }

    /// Calculate hash for datum (for both inline and hash-based)
    pub fn hash(&self) -> Blake2b256Hash {
        match self {
            OutputDatum::DatumHash(hash) => hash.clone(),
            OutputDatum::InlineDatum(data) => {
                Blake2b256Hash::new(&format!("{:?}", data).as_bytes())
            }
        }
    }
}

impl ScriptReference {
    pub fn hash(&self) -> Blake2b256Hash {
        match self {
            ScriptReference::NativeScript(script) => script.hash(),
            ScriptReference::PlutusV1Script(script) => script.hash(),
            ScriptReference::PlutusV2Script(script) => script.hash(),
        }
    }
}

impl BabbageTransaction {
    /// Validate transaction against Babbage era rules
    pub fn validate(&self, witnesses: &BabbageWitnessSet, ledger_state: &BabbageLedgerState) -> Result<()> {
        // Check validity interval
        if let Some(validity_interval) = &self.validity_interval {
            if !validity_interval.is_valid_at_slot(ledger_state.current_slot) {
                return Err(LedgerError::InvalidTransaction("Transaction outside validity interval".to_string()));
            }
        }

        // Validate network ID if specified
        if let Some(tx_network_id) = &self.network_id {
            if *tx_network_id != ledger_state.network_id {
                return Err(LedgerError::InvalidTransaction("Network ID mismatch".to_string()));
            }
        }

        // Check regular inputs exist and are unspent
        for input in &self.inputs {
            self.validate_input_exists(input, ledger_state)?;
        }

        // Check reference inputs exist (don't need to be unspent, they're read-only)
        for ref_input in &self.reference_inputs {
            if !ledger_state.babbage_utxo_set.contains_key(ref_input) {
                return Err(LedgerError::InvalidInput("Reference input not found".to_string()));
            }
        }

        // Validate collateral
        self.validate_collateral(ledger_state)?;

        // Execute Plutus scripts (V1 and V2)
        self.execute_scripts(witnesses, ledger_state)?;

        // Validate value conservation
        self.validate_value_conservation(ledger_state)?;

        Ok(())
    }

    fn validate_input_exists(&self, input: &ShelleyTxIn, ledger_state: &BabbageLedgerState) -> Result<()> {
        if !ledger_state.babbage_utxo_set.contains_key(input) {
            return Err(LedgerError::InvalidInput(format!("Input {:?} not found", input.tx_id)));
        }

        if ledger_state.babbage_utxo_set[input].spent {
            return Err(LedgerError::InvalidInput("Input already spent".to_string()));
        }

        Ok(())
    }

    fn validate_collateral(&self, ledger_state: &BabbageLedgerState) -> Result<()> {
        if self.collateral.len() > ledger_state.max_collateral_inputs as usize {
            return Err(LedgerError::InvalidInput("Too many collateral inputs".to_string()));
        }

        for collateral in &self.collateral {
            if !ledger_state.babbage_utxo_set.contains_key(collateral) {
                return Err(LedgerError::InvalidInput("Collateral input not found".to_string()));
            }

            let utxo = &ledger_state.babbage_utxo_set[collateral];
            if utxo.spent {
                return Err(LedgerError::InvalidInput("Collateral input already spent".to_string()));
            }

            // Collateral must not have datum or reference script
            if utxo.tx_out.datum.is_some() || utxo.tx_out.script_ref.is_some() {
                return Err(LedgerError::InvalidInput("Collateral cannot have datum or script reference".to_string()));
            }
        }

        // Validate collateral return if present
        if let Some(collateral_return) = &self.collateral_return {
            if self.total_collateral.is_none() {
                return Err(LedgerError::InvalidTransaction("Total collateral must be specified with collateral return".to_string()));
            }

            // Collateral return should not have datum or script reference
            if collateral_return.datum.is_some() || collateral_return.script_ref.is_some() {
                return Err(LedgerError::InvalidTransaction("Collateral return cannot have datum or script reference".to_string()));
            }
        }

        Ok(())
    }

    fn execute_scripts(&self, witnesses: &BabbageWitnessSet, ledger_state: &BabbageLedgerState) -> Result<()> {
        let mut total_ex_units = ExUnits::new(0, 0);

        // Execute scripts for spending inputs
        for (input_idx, input) in self.inputs.iter().enumerate() {
            let utxo = &ledger_state.babbage_utxo_set[input];

            if let Some(datum) = &utxo.tx_out.datum {
                let script = self.find_script_for_input(input, witnesses, ledger_state)?;
                let datum_data = self.resolve_datum(datum, witnesses)?;
                let redeemer = self.find_redeemer(RedeemerTag::Spend, input_idx as u32, witnesses)?;

                let context = self.build_script_context(witnesses, ledger_state);

                script.execute_v2(datum_data.as_ref(), &redeemer.data, &context)?;
                total_ex_units = total_ex_units.add(&redeemer.ex_units);
            }
        }

        // Execute minting policy scripts
        if let Some(mint) = &self.mint {
            for (mint_idx, policy_id) in mint.assets.keys().enumerate() {
                let script = self.find_minting_script(policy_id, witnesses, ledger_state)?;
                let redeemer = self.find_redeemer(RedeemerTag::Mint, mint_idx as u32, witnesses)?;

                let context = self.build_script_context(witnesses, ledger_state);

                script.execute_v2(None, &redeemer.data, &context)?;
                total_ex_units = total_ex_units.add(&redeemer.ex_units);
            }
        }

        // Check execution units are within limits
        if !total_ex_units.is_within_limit(&ledger_state.base.max_tx_ex_units) {
            return Err(LedgerError::ScriptExecutionError("Transaction execution units exceed limit".to_string()));
        }

        Ok(())
    }

    fn find_script_for_input(&self, input: &ShelleyTxIn, witnesses: &BabbageWitnessSet, ledger_state: &BabbageLedgerState) -> Result<PlutusScript> {
        let utxo = &ledger_state.babbage_utxo_set[input];

        // Check if script is referenced in the output
        if let Some(script_ref) = &utxo.tx_out.script_ref {
            match script_ref {
                ScriptReference::PlutusV1Script(script) => return Ok(script.clone()),
                ScriptReference::PlutusV2Script(script) => return Ok(script.clone()),
                ScriptReference::NativeScript(_) => {
                    return Err(LedgerError::InvalidScript("Expected Plutus script, found native script".to_string()));
                }
            }
        }

        // Look for script in reference inputs
        for ref_input in &self.reference_inputs {
            let ref_utxo = &ledger_state.babbage_utxo_set[ref_input];
            if let Some(script_ref) = &ref_utxo.tx_out.script_ref {
                match script_ref {
                    ScriptReference::PlutusV1Script(script) => return Ok(script.clone()),
                    ScriptReference::PlutusV2Script(script) => return Ok(script.clone()),
                    _ => continue,
                }
            }
        }

        // Fall back to witness set
        witnesses.plutus_v2_scripts.first()
            .or_else(|| witnesses.plutus_v1_scripts.first())
            .cloned()
            .ok_or_else(|| LedgerError::InvalidScript("Required script not found".to_string()))
    }

    fn find_minting_script(&self, _policy_id: &PolicyId, witnesses: &BabbageWitnessSet, _ledger_state: &BabbageLedgerState) -> Result<PlutusScript> {
        // Simplified - would match policy ID to script hash
        witnesses.plutus_v2_scripts.first()
            .or_else(|| witnesses.plutus_v1_scripts.first())
            .cloned()
            .ok_or_else(|| LedgerError::InvalidScript("Minting policy script not found".to_string()))
    }

    fn resolve_datum(&self, output_datum: &OutputDatum, witnesses: &BabbageWitnessSet) -> Result<Option<PlutusData>> {
        match output_datum {
            OutputDatum::InlineDatum(data) => Ok(Some(data.clone())),
            OutputDatum::DatumHash(_hash) => {
                // Would look up datum in witnesses by hash
                Ok(witnesses.plutus_data.first().cloned())
            }
        }
    }

    fn find_redeemer(&self, tag: RedeemerTag, index: u32, witnesses: &BabbageWitnessSet) -> Result<&BabbageRedeemer> {
        witnesses.redeemers.iter()
            .find(|r| r.tag == tag && r.index == index)
            .ok_or_else(|| LedgerError::InvalidScript("Required redeemer not found".to_string()))
    }

    fn build_script_context(&self, _witnesses: &BabbageWitnessSet, _ledger_state: &BabbageLedgerState) -> ScriptContext {
        // Enhanced context with reference inputs
        ScriptContext {
            tx_info: TxInfo {
                inputs: vec![], // Would include reference inputs in V2
                outputs: self.outputs.iter().map(|o| AlonzoTransactionOutput {
                    address: o.address.clone(),
                    value: o.value.clone(),
                    datum_hash: o.datum.as_ref().map(|d| d.hash()),
                    script_ref: None, // Simplified
                }).collect(),
                fee: MaryValue::new_ada_only(self.fee),
                mint: MaryValue::new_ada_only(0),
                dcert: self.certificates.clone(),
                wdrl: self.withdrawals.clone(),
                valid_range: self.validity_interval.clone().unwrap_or_else(|| ValidityInterval {
                    invalid_before: None,
                    invalid_hereafter: None,
                }),
                signatories: self.required_signers.clone(),
                data: HashMap::new(),
                id: self.hash(),
            },
            purpose: ScriptPurpose::Spending(ShelleyTxIn {
                tx_id: Blake2b256Hash::new(b"dummy"),
                output_index: 0,
            }),
        }
    }

    fn validate_value_conservation(&self, ledger_state: &BabbageLedgerState) -> Result<()> {
        // Calculate input value (excluding reference inputs)
        let mut input_value = MaryValue::new_ada_only(0);
        for input in &self.inputs {
            let utxo = &ledger_state.babbage_utxo_set[input];
            input_value = input_value.add(&utxo.tx_out.value);
        }

        // Calculate output value including fee
        let mut output_value = MaryValue::new_ada_only(self.fee);
        for output in &self.outputs {
            output_value = output_value.add(&output.value);
        }

        // Add collateral return if present
        if let Some(collateral_return) = &self.collateral_return {
            output_value = output_value.add(&collateral_return.value);
        }

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

        // Add withdrawals to input
        let withdrawal_sum: u64 = self.withdrawals.values().sum();
        input_value.coin += withdrawal_sum;

        // Check conservation
        if input_value.coin != output_value.coin {
            return Err(LedgerError::InvalidTransaction("ADA not conserved".to_string()));
        }

        // Check native tokens conservation (same as previous eras)
        for (policy_id, assets) in &input_value.multi_asset.assets {
            for (asset_name, &input_amount) in assets {
                let output_amount = output_value.multi_asset.get_token_amount(policy_id, asset_name);
                if input_amount != output_amount {
                    return Err(LedgerError::InvalidTransaction(
                        format!("Token {} not conserved", hex::encode(&asset_name.0))
                    ));
                }
            }
        }

        Ok(())
    }

    pub fn hash(&self) -> Blake2b256Hash {
        Blake2b256Hash::new(&format!("{:?}", self).as_bytes())
    }
}

impl BabbageLedgerState {
    pub fn new() -> Self {
        Self {
            base: AlonzoLedgerState::new(),
            babbage_utxo_set: HashMap::new(),
            current_slot: 0,
            network_id: NetworkId::Testnet,
            plutus_v2_cost_model: HashMap::new(),
            max_collateral_inputs: 3,
        }
    }

    pub fn apply_transaction(&mut self, tx: &BabbageTransaction, witnesses: &BabbageWitnessSet) -> Result<()> {
        let script_validation_result = tx.validate(witnesses, self);

        if script_validation_result.is_err() {
            // Script validation failed - consume collateral and optionally return change
            self.consume_collateral_with_return(tx)?;
            return script_validation_result;
        }

        // Scripts succeeded - apply transaction normally

        // Remove spent outputs (not reference inputs)
        for input in &tx.inputs {
            if let Some(utxo) = self.babbage_utxo_set.get_mut(input) {
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
            let utxo = BabbageUtxo {
                tx_out: output.clone(),
                spent: false,
            };
            self.babbage_utxo_set.insert(tx_in, utxo);
        }

        Ok(())
    }

    fn consume_collateral_with_return(&mut self, tx: &BabbageTransaction) -> Result<()> {
        // Mark collateral inputs as spent
        for collateral_input in &tx.collateral {
            if let Some(utxo) = self.babbage_utxo_set.get_mut(collateral_input) {
                utxo.spent = true;
            }
        }

        // Add collateral return output if present
        if let Some(collateral_return) = &tx.collateral_return {
            let tx_hash = tx.hash();
            let return_tx_in = ShelleyTxIn {
                tx_id: tx_hash,
                output_index: u32::MAX, // Special index for collateral return
            };

            self.babbage_utxo_set.insert(return_tx_in, BabbageUtxo {
                tx_out: collateral_return.clone(),
                spent: false,
            });
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
    fn test_inline_datum_resolution() {
        let inline_datum = OutputDatum::InlineDatum(PlutusData::Integer(42));
        let hash_datum = OutputDatum::DatumHash(Blake2b256Hash::new(b"test_hash"));

        assert!(inline_datum.get_inline_datum().is_some());
        assert_eq!(*inline_datum.get_inline_datum().unwrap(), PlutusData::Integer(42));

        assert!(hash_datum.get_inline_datum().is_none());
    }

    #[test]
    fn test_reference_inputs_validation() {
        let mut ledger = BabbageLedgerState::new();
        ledger.current_slot = 1000;

        // Create reference input UTXO with script
        let ref_input = ShelleyTxIn {
            tx_id: Blake2b256Hash::new(b"ref_tx"),
            output_index: 0,
        };

        let script_v2 = PlutusScript::new_v2(vec![1, 2, 3, 4]);
        let ref_output = BabbageTransactionOutput {
            address: create_test_address(),
            value: MaryValue::new_ada_only(1_000_000),
            datum: None,
            script_ref: Some(ScriptReference::PlutusV2Script(script_v2)),
        };

        ledger.babbage_utxo_set.insert(ref_input.clone(), BabbageUtxo {
            tx_out: ref_output,
            spent: false,
        });

        // Create transaction that uses reference input
        let tx = BabbageTransaction {
            inputs: vec![],
            reference_inputs: vec![ref_input],
            outputs: vec![],
            collateral: vec![],
            collateral_return: None,
            total_collateral: None,
            fee: 200_000,
            validity_interval: Some(ValidityInterval {
                invalid_before: None,
                invalid_hereafter: Some(2000),
            }),
            certificates: vec![],
            withdrawals: HashMap::new(),
            mint: None,
            required_signers: vec![],
            network_id: Some(NetworkId::Testnet),
        };

        let witnesses = BabbageWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_v2_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_ok());
    }

    #[test]
    fn test_collateral_return() {
        let mut ledger = BabbageLedgerState::new();

        // Create collateral input
        let collateral_input = ShelleyTxIn {
            tx_id: Blake2b256Hash::new(b"collateral_tx"),
            output_index: 0,
        };

        ledger.babbage_utxo_set.insert(collateral_input.clone(), BabbageUtxo {
            tx_out: BabbageTransactionOutput {
                address: create_test_address(),
                value: MaryValue::new_ada_only(10_000_000), // 10 ADA collateral
                datum: None,
                script_ref: None,
            },
            spent: false,
        });

        // Create transaction with collateral return
        let tx = BabbageTransaction {
            inputs: vec![],
            reference_inputs: vec![],
            outputs: vec![],
            collateral: vec![collateral_input],
            collateral_return: Some(BabbageTransactionOutput {
                address: create_test_address(),
                value: MaryValue::new_ada_only(7_000_000), // Return 7 ADA
                datum: None,
                script_ref: None,
            }),
            total_collateral: Some(3_000_000), // Consume 3 ADA
            fee: 200_000,
            validity_interval: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            mint: None,
            required_signers: vec![],
            network_id: None,
        };

        let witnesses = BabbageWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_v2_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_ok());
    }

    #[test]
    fn test_plutus_v2_script_execution() {
        let mut ledger = BabbageLedgerState::new();
        ledger.current_slot = 1000;

        // Create script-locked input with inline datum
        let script_v2 = PlutusScript::new_v2(vec![5, 6, 7, 8]);
        let input_tx_in = ShelleyTxIn {
            tx_id: Blake2b256Hash::new(b"script_input"),
            output_index: 0,
        };

        let script_output = BabbageTransactionOutput {
            address: create_test_address(),
            value: MaryValue::new_ada_only(2_000_000),
            datum: Some(OutputDatum::InlineDatum(PlutusData::Integer(100))), // Inline datum
            script_ref: Some(ScriptReference::PlutusV2Script(script_v2)),
        };

        ledger.babbage_utxo_set.insert(input_tx_in.clone(), BabbageUtxo {
            tx_out: script_output,
            spent: false,
        });

        let tx = BabbageTransaction {
            inputs: vec![input_tx_in],
            reference_inputs: vec![],
            outputs: vec![BabbageTransactionOutput {
                address: create_test_address(),
                value: MaryValue::new_ada_only(1_700_000),
                datum: None,
                script_ref: None,
            }],
            collateral: vec![],
            collateral_return: None,
            total_collateral: None,
            fee: 300_000,
            validity_interval: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            mint: None,
            required_signers: vec![],
            network_id: None,
        };

        let witnesses = BabbageWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_v2_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![BabbageRedeemer {
                tag: RedeemerTag::Spend,
                index: 0,
                data: PlutusData::Constructor {
                    alternative: 5, // Valid alternative < 10
                    fields: vec![],
                },
                ex_units: ExUnits::new(500_000, 1_000_000),
            }],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_collateral_with_datum() {
        let mut ledger = BabbageLedgerState::new();

        // Create collateral input with datum (invalid)
        let collateral_input = ShelleyTxIn {
            tx_id: Blake2b256Hash::new(b"bad_collateral"),
            output_index: 0,
        };

        ledger.babbage_utxo_set.insert(collateral_input.clone(), BabbageUtxo {
            tx_out: BabbageTransactionOutput {
                address: create_test_address(),
                value: MaryValue::new_ada_only(5_000_000),
                datum: Some(OutputDatum::InlineDatum(PlutusData::Integer(42))), // Invalid!
                script_ref: None,
            },
            spent: false,
        });

        let tx = BabbageTransaction {
            inputs: vec![],
            reference_inputs: vec![],
            outputs: vec![],
            collateral: vec![collateral_input],
            collateral_return: None,
            total_collateral: None,
            fee: 200_000,
            validity_interval: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            mint: None,
            required_signers: vec![],
            network_id: None,
        };

        let witnesses = BabbageWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_v2_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidInput(_)));
    }

    #[test]
    fn test_network_id_validation() {
        let mut ledger = BabbageLedgerState::new();
        ledger.network_id = NetworkId::Mainnet;

        let tx = BabbageTransaction {
            inputs: vec![],
            reference_inputs: vec![],
            outputs: vec![],
            collateral: vec![],
            collateral_return: None,
            total_collateral: None,
            fee: 200_000,
            validity_interval: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            mint: None,
            required_signers: vec![],
            network_id: Some(NetworkId::Testnet), // Mismatch!
        };

        let witnesses = BabbageWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_v2_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidTransaction(_)));
    }

    #[test]
    fn test_too_many_collateral_inputs() {
        let mut ledger = BabbageLedgerState::new();
        ledger.max_collateral_inputs = 2; // Limit to 2

        // Create 3 collateral inputs
        let collateral_inputs: Vec<_> = (0..3).map(|i| {
            let input = ShelleyTxIn {
                tx_id: Blake2b256Hash::new(&format!("collateral_{}", i).as_bytes()),
                output_index: 0,
            };

            ledger.babbage_utxo_set.insert(input.clone(), BabbageUtxo {
                tx_out: BabbageTransactionOutput {
                    address: create_test_address(),
                    value: MaryValue::new_ada_only(5_000_000),
                    datum: None,
                    script_ref: None,
                },
                spent: false,
            });

            input
        }).collect();

        let tx = BabbageTransaction {
            inputs: vec![],
            reference_inputs: vec![],
            outputs: vec![],
            collateral: collateral_inputs, // 3 inputs > limit of 2
            collateral_return: None,
            total_collateral: None,
            fee: 200_000,
            validity_interval: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            mint: None,
            required_signers: vec![],
            network_id: None,
        };

        let witnesses = BabbageWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_v2_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidInput(_)));
    }
}
