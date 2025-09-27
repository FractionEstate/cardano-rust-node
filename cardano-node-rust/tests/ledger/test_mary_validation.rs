//! Mary Era Validation Tests
//!
//! Tests for Mary era ledger rules validation.
//! Mary introduced native tokens (multi-assets):
//! - Native token creation and destruction (minting/burning)
//! - Multi-asset transaction outputs
//! - Token bundle management and validation
//! - Policy-based token control

use cardano_ledger::{LedgerError, Result};
use cardano_crypto::{Blake2b256Hash, Ed25519KeyHash};
use std::collections::HashMap;

/// Re-export types from previous eras
pub use crate::test_allegra_validation::{
    AllegraTransaction, AllegraTransactionOutput, ValidityInterval,
    NativeScript, AllegraWitnessSet, VKeyWitness, ShelleyTxIn
};
pub use crate::test_shelley_validation::{ShelleyAddress, NetworkId, Credential};

/// Policy ID for native tokens (hash of minting policy script)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PolicyId(pub Blake2b256Hash);

/// Asset name within a policy
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetName(pub Vec<u8>);

/// Represents a quantity of a native token
pub type TokenAmount = i64; // Positive = mint, Negative = burn, Zero = no change

/// Multi-asset value containing ADA and native tokens
#[derive(Debug, Clone)]
pub struct MaryValue {
    pub coin: u64, // ADA in lovelace
    pub multi_asset: MultiAsset,
}

/// Collection of native tokens organized by policy
#[derive(Debug, Clone)]
pub struct MultiAsset {
    pub assets: HashMap<PolicyId, HashMap<AssetName, u64>>,
}

/// Minting/burning specification in a transaction
#[derive(Debug, Clone)]
pub struct Mint {
    pub assets: HashMap<PolicyId, HashMap<AssetName, TokenAmount>>,
}

/// Mary transaction output with multi-asset support
#[derive(Debug, Clone)]
pub struct MaryTransactionOutput {
    pub address: ShelleyAddress,
    pub value: MaryValue,
    pub script: Option<NativeScript>,
}

/// Mary transaction with multi-asset support
#[derive(Debug, Clone)]
pub struct MaryTransaction {
    pub inputs: Vec<ShelleyTxIn>,
    pub outputs: Vec<MaryTransactionOutput>,
    pub fee: u64,
    pub validity_interval: ValidityInterval,
    pub mint: Option<Mint>, // Minting/burning operations
    pub native_scripts: Vec<NativeScript>, // Minting policies and other scripts
}

/// Mary UTXO entry
#[derive(Debug, Clone)]
pub struct MaryUtxo {
    pub tx_out: MaryTransactionOutput,
    pub spent: bool,
}

/// Mary ledger state with multi-asset tracking
#[derive(Debug, Clone)]
pub struct MaryLedgerState {
    pub utxo_set: HashMap<ShelleyTxIn, MaryUtxo>,
    pub current_slot: u64,
    /// Track total supply of each native token
    pub token_supply: HashMap<PolicyId, HashMap<AssetName, u64>>,
}

impl PolicyId {
    pub fn from_script(script: &NativeScript) -> Self {
        PolicyId(script.hash())
    }

    /// ADA policy ID (empty hash)
    pub fn ada() -> Self {
        PolicyId(Blake2b256Hash::new(b""))
    }
}

impl AssetName {
    pub fn new(name: &[u8]) -> Result<Self> {
        if name.len() > 32 {
            return Err(LedgerError::InvalidAsset("Asset name too long".to_string()));
        }
        Ok(AssetName(name.to_vec()))
    }

    pub fn empty() -> Self {
        AssetName(vec![])
    }
}

impl MaryValue {
    pub fn new_ada_only(lovelace: u64) -> Self {
        Self {
            coin: lovelace,
            multi_asset: MultiAsset::new(),
        }
    }

    pub fn add_token(&mut self, policy_id: PolicyId, asset_name: AssetName, amount: u64) {
        self.multi_asset.add_token(policy_id, asset_name, amount);
    }

    /// Add two values together
    pub fn add(&self, other: &MaryValue) -> MaryValue {
        MaryValue {
            coin: self.coin + other.coin,
            multi_asset: self.multi_asset.add(&other.multi_asset),
        }
    }

    /// Subtract other from self, returning error if insufficient funds
    pub fn subtract(&self, other: &MaryValue) -> Result<MaryValue> {
        if self.coin < other.coin {
            return Err(LedgerError::InsufficientFunds("Insufficient ADA".to_string()));
        }

        let result_multi_asset = self.multi_asset.subtract(&other.multi_asset)?;

        Ok(MaryValue {
            coin: self.coin - other.coin,
            multi_asset: result_multi_asset,
        })
    }
}

impl MultiAsset {
    pub fn new() -> Self {
        Self {
            assets: HashMap::new(),
        }
    }

    pub fn add_token(&mut self, policy_id: PolicyId, asset_name: AssetName, amount: u64) {
        let policy_assets = self.assets.entry(policy_id).or_insert_with(HashMap::new);
        *policy_assets.entry(asset_name).or_insert(0) += amount;
    }

    pub fn get_token_amount(&self, policy_id: &PolicyId, asset_name: &AssetName) -> u64 {
        self.assets
            .get(policy_id)
            .and_then(|assets| assets.get(asset_name))
            .copied()
            .unwrap_or(0)
    }

    pub fn add(&self, other: &MultiAsset) -> MultiAsset {
        let mut result = self.clone();

        for (policy_id, assets) in &other.assets {
            for (asset_name, amount) in assets {
                result.add_token(policy_id.clone(), asset_name.clone(), *amount);
            }
        }

        result
    }

    pub fn subtract(&self, other: &MultiAsset) -> Result<MultiAsset> {
        let mut result = self.clone();

        for (policy_id, assets) in &other.assets {
            for (asset_name, amount) in assets {
                let current_amount = result.get_token_amount(policy_id, asset_name);
                if current_amount < *amount {
                    return Err(LedgerError::InsufficientFunds(
                        format!("Insufficient {} tokens", hex::encode(&asset_name.0))
                    ));
                }

                let policy_assets = result.assets.entry(policy_id.clone()).or_insert_with(HashMap::new);
                let entry = policy_assets.entry(asset_name.clone()).or_insert(0);
                *entry = entry.saturating_sub(*amount);

                // Remove zero entries to keep map clean
                if *entry == 0 {
                    policy_assets.remove(asset_name);
                    if policy_assets.is_empty() {
                        result.assets.remove(policy_id);
                    }
                }
            }
        }

        Ok(result)
    }
}

impl Mint {
    pub fn new() -> Self {
        Self {
            assets: HashMap::new(),
        }
    }

    pub fn add_mint(&mut self, policy_id: PolicyId, asset_name: AssetName, amount: TokenAmount) {
        let policy_assets = self.assets.entry(policy_id).or_insert_with(HashMap::new);
        *policy_assets.entry(asset_name).or_insert(0) += amount;
    }
}

impl MaryTransaction {
    /// Validate transaction against Mary era rules
    pub fn validate(&self, witnesses: &AllegraWitnessSet, ledger_state: &MaryLedgerState) -> Result<()> {
        // Check validity interval
        if !self.validity_interval.is_valid_at_slot(ledger_state.current_slot) {
            return Err(LedgerError::InvalidTransaction("Transaction outside validity interval".to_string()));
        }

        // Check inputs exist and are unspent
        for input in &self.inputs {
            if !ledger_state.utxo_set.contains_key(input) {
                return Err(LedgerError::InvalidInput(format!("Input {:?} not found", input.tx_id)));
            }

            if ledger_state.utxo_set[input].spent {
                return Err(LedgerError::InvalidInput("Input already spent".to_string()));
            }
        }

        // Validate native scripts for inputs and minting policies
        self.validate_scripts(witnesses, ledger_state)?;

        // Validate value conservation including native tokens
        self.validate_value_conservation(ledger_state)?;

        // Validate minting operations
        if let Some(mint) = &self.mint {
            self.validate_minting(mint, witnesses, ledger_state)?;
        }

        Ok(())
    }

    fn validate_scripts(&self, witnesses: &AllegraWitnessSet, ledger_state: &MaryLedgerState) -> Result<()> {
        // Validate scripts for spending inputs
        for input in &self.inputs {
            let utxo = &ledger_state.utxo_set[input];
            if let Some(script) = &utxo.tx_out.script {
                if !script.evaluate(witnesses, ledger_state.current_slot) {
                    return Err(LedgerError::InvalidScript("Input script evaluation failed".to_string()));
                }
            }
        }

        // Validate minting policy scripts
        if let Some(mint) = &self.mint {
            for policy_id in mint.assets.keys() {
                // Find the corresponding native script for this policy
                let policy_script = self.native_scripts.iter()
                    .find(|script| PolicyId::from_script(script) == *policy_id)
                    .ok_or_else(|| LedgerError::InvalidScript("Missing minting policy script".to_string()))?;

                if !policy_script.evaluate(witnesses, ledger_state.current_slot) {
                    return Err(LedgerError::InvalidScript("Minting policy script evaluation failed".to_string()));
                }
            }
        }

        Ok(())
    }

    fn validate_value_conservation(&self, ledger_state: &MaryLedgerState) -> Result<()> {
        // Calculate input value
        let mut input_value = MaryValue::new_ada_only(0);
        for input in &self.inputs {
            let utxo = &ledger_state.utxo_set[input];
            input_value = input_value.add(&utxo.tx_out.value);
        }

        // Calculate output value
        let mut output_value = MaryValue::new_ada_only(self.fee);
        for output in &self.outputs {
            output_value = output_value.add(&output.value);
        }

        // Apply minting (positive = create, negative = destroy)
        if let Some(mint) = &self.mint {
            for (policy_id, assets) in &mint.assets {
                for (asset_name, &amount) in assets {
                    if amount > 0 {
                        // Minting - adds to output side
                        output_value.multi_asset.add_token(
                            policy_id.clone(),
                            asset_name.clone(),
                            amount as u64
                        );
                    } else if amount < 0 {
                        // Burning - subtracts from input side
                        input_value.multi_asset.add_token(
                            policy_id.clone(),
                            asset_name.clone(),
                            (-amount) as u64
                        );
                    }
                }
            }
        }

        // Check ADA conservation
        if input_value.coin != output_value.coin {
            return Err(LedgerError::InvalidTransaction("ADA not conserved".to_string()));
        }

        // Check native token conservation
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

        // Check for extra tokens in output
        for (policy_id, assets) in &output_value.multi_asset.assets {
            for (asset_name, &output_amount) in assets {
                let input_amount = input_value.multi_asset.get_token_amount(policy_id, asset_name);
                if input_amount != output_amount {
                    return Err(LedgerError::InvalidTransaction(
                        format!("Token {} not conserved", hex::encode(&asset_name.0))
                    ));
                }
            }
        }

        Ok(())
    }

    fn validate_minting(&self, mint: &Mint, _witnesses: &AllegraWitnessSet, ledger_state: &MaryLedgerState) -> Result<()> {
        // Check that burning doesn't exceed current supply
        for (policy_id, assets) in &mint.assets {
            for (asset_name, &amount) in assets {
                if amount < 0 {
                    let current_supply = ledger_state.token_supply
                        .get(policy_id)
                        .and_then(|assets| assets.get(asset_name))
                        .copied()
                        .unwrap_or(0);

                    if (-amount) as u64 > current_supply {
                        return Err(LedgerError::InvalidTransaction(
                            "Cannot burn more tokens than exist".to_string()
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn hash(&self) -> Blake2b256Hash {
        Blake2b256Hash::new(&format!("{:?}", self).as_bytes())
    }
}

impl MaryLedgerState {
    pub fn new() -> Self {
        Self {
            utxo_set: HashMap::new(),
            current_slot: 0,
            token_supply: HashMap::new(),
        }
    }

    pub fn apply_transaction(&mut self, tx: &MaryTransaction, witnesses: &AllegraWitnessSet) -> Result<()> {
        tx.validate(witnesses, self)?;

        // Update token supplies from minting
        if let Some(mint) = &tx.mint {
            for (policy_id, assets) in &mint.assets {
                for (asset_name, &amount) in assets {
                    let policy_supplies = self.token_supply.entry(policy_id.clone()).or_insert_with(HashMap::new);
                    let current_supply = policy_supplies.entry(asset_name.clone()).or_insert(0);

                    if amount >= 0 {
                        *current_supply += amount as u64;
                    } else {
                        *current_supply = current_supply.saturating_sub((-amount) as u64);
                        if *current_supply == 0 {
                            policy_supplies.remove(asset_name);
                            if policy_supplies.is_empty() {
                                self.token_supply.remove(policy_id);
                            }
                        }
                    }
                }
            }
        }

        // Remove spent outputs
        for input in &tx.inputs {
            if let Some(utxo) = self.utxo_set.get_mut(input) {
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
            let utxo = MaryUtxo {
                tx_out: output.clone(),
                spent: false,
            };
            self.utxo_set.insert(tx_in, utxo);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_shelley_validation::{StakeReference};

    fn create_test_address() -> ShelleyAddress {
        ShelleyAddress {
            payment_credential: Credential::Key(Ed25519KeyHash::new(b"test_payment_key")),
            stake_credential: Some(StakeReference::Credential(
                Credential::Key(Ed25519KeyHash::new(b"test_stake_key"))
            )),
            network_id: NetworkId::Testnet,
        }
    }

    #[test]
    fn test_mary_value_operations() {
        let mut value1 = MaryValue::new_ada_only(1_000_000);
        let policy_id = PolicyId(Blake2b256Hash::new(b"test_policy"));
        let asset_name = AssetName::new(b"TEST_TOKEN").unwrap();

        value1.add_token(policy_id.clone(), asset_name.clone(), 100);

        let mut value2 = MaryValue::new_ada_only(500_000);
        value2.add_token(policy_id.clone(), asset_name.clone(), 50);

        // Test addition
        let sum = value1.add(&value2);
        assert_eq!(sum.coin, 1_500_000);
        assert_eq!(sum.multi_asset.get_token_amount(&policy_id, &asset_name), 150);

        // Test subtraction
        let diff = sum.subtract(&value2).unwrap();
        assert_eq!(diff.coin, 1_000_000);
        assert_eq!(diff.multi_asset.get_token_amount(&policy_id, &asset_name), 100);
    }

    #[test]
    fn test_insufficient_funds_native_tokens() {
        let mut value1 = MaryValue::new_ada_only(1_000_000);
        let policy_id = PolicyId(Blake2b256Hash::new(b"test_policy"));
        let asset_name = AssetName::new(b"TEST_TOKEN").unwrap();

        value1.add_token(policy_id.clone(), asset_name.clone(), 50);

        let mut value2 = MaryValue::new_ada_only(500_000);
        value2.add_token(policy_id.clone(), asset_name.clone(), 100); // More than available

        let result = value1.subtract(&value2);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InsufficientFunds(_)));
    }

    #[test]
    fn test_native_token_minting() {
        let mut ledger = MaryLedgerState::new();
        ledger.current_slot = 1000;

        let key_hash = Ed25519KeyHash::new(b"minting_key");
        let minting_script = NativeScript::RequireSignature(key_hash.clone());
        let policy_id = PolicyId::from_script(&minting_script);
        let asset_name = AssetName::new(b"NEW_TOKEN").unwrap();

        let mut mint = Mint::new();
        mint.add_mint(policy_id.clone(), asset_name.clone(), 1000); // Mint 1000 tokens

        let tx = MaryTransaction {
            inputs: vec![],
            outputs: vec![MaryTransactionOutput {
                address: create_test_address(),
                value: {
                    let mut value = MaryValue::new_ada_only(1_000_000);
                    value.add_token(policy_id.clone(), asset_name.clone(), 1000);
                    value
                },
                script: None,
            }],
            fee: 200_000,
            validity_interval: ValidityInterval {
                invalid_before: None,
                invalid_hereafter: None,
            },
            mint: Some(mint),
            native_scripts: vec![minting_script],
        };

        let witnesses = AllegraWitnessSet {
            vkey_witnesses: vec![VKeyWitness {
                vkey: key_hash,
                signature: cardano_crypto::Ed25519Signature::new(&[1; 64]),
            }],
            native_scripts: vec![],
        };

        // Should fail due to value conservation (no ADA input for output + fee)
        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_err());
    }

    #[test]
    fn test_token_burning() {
        let mut ledger = MaryLedgerState::new();
        ledger.current_slot = 1000;

        let key_hash = Ed25519KeyHash::new(b"burning_key");
        let burning_script = NativeScript::RequireSignature(key_hash.clone());
        let policy_id = PolicyId::from_script(&burning_script);
        let asset_name = AssetName::new(b"BURN_TOKEN").unwrap();

        // Set up existing token supply
        let mut policy_supplies = HashMap::new();
        policy_supplies.insert(asset_name.clone(), 2000);
        ledger.token_supply.insert(policy_id.clone(), policy_supplies);

        // Create UTXO with tokens to burn
        let input_tx_in = ShelleyTxIn {
            tx_id: Blake2b256Hash::new(b"input_tx"),
            output_index: 0,
        };

        let mut input_value = MaryValue::new_ada_only(2_000_000);
        input_value.add_token(policy_id.clone(), asset_name.clone(), 500);

        ledger.utxo_set.insert(input_tx_in.clone(), MaryUtxo {
            tx_out: MaryTransactionOutput {
                address: create_test_address(),
                value: input_value,
                script: None,
            },
            spent: false,
        });

        // Create transaction that burns 300 tokens
        let mut mint = Mint::new();
        mint.add_mint(policy_id.clone(), asset_name.clone(), -300); // Burn 300 tokens

        let mut output_value = MaryValue::new_ada_only(1_700_000); // 2M - 300k fee
        output_value.add_token(policy_id.clone(), asset_name.clone(), 200); // 500 - 300 burned

        let tx = MaryTransaction {
            inputs: vec![input_tx_in],
            outputs: vec![MaryTransactionOutput {
                address: create_test_address(),
                value: output_value,
                script: None,
            }],
            fee: 300_000,
            validity_interval: ValidityInterval {
                invalid_before: None,
                invalid_hereafter: None,
            },
            mint: Some(mint),
            native_scripts: vec![burning_script],
        };

        let witnesses = AllegraWitnessSet {
            vkey_witnesses: vec![VKeyWitness {
                vkey: key_hash,
                signature: cardano_crypto::Ed25519Signature::new(&[1; 64]),
            }],
            native_scripts: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_ok());

        // Apply transaction and check supply updated
        ledger.apply_transaction(&tx, &witnesses).unwrap();
        let new_supply = ledger.token_supply.get(&policy_id).unwrap().get(&asset_name).unwrap();
        assert_eq!(*new_supply, 1700); // 2000 - 300 burned
    }

    #[test]
    fn test_excessive_token_burning() {
        let mut ledger = MaryLedgerState::new();

        let policy_id = PolicyId(Blake2b256Hash::new(b"test_policy"));
        let asset_name = AssetName::new(b"LIMITED_TOKEN").unwrap();

        // Set up token supply of only 100
        let mut policy_supplies = HashMap::new();
        policy_supplies.insert(asset_name.clone(), 100);
        ledger.token_supply.insert(policy_id.clone(), policy_supplies);

        // Try to burn 200 tokens (more than exist)
        let mut mint = Mint::new();
        mint.add_mint(policy_id.clone(), asset_name.clone(), -200);

        let tx = MaryTransaction {
            inputs: vec![],
            outputs: vec![],
            fee: 0,
            validity_interval: ValidityInterval {
                invalid_before: None,
                invalid_hereafter: None,
            },
            mint: Some(mint),
            native_scripts: vec![],
        };

        let witnesses = AllegraWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidTransaction(_)));
    }

    #[test]
    fn test_asset_name_length_validation() {
        // Valid asset name (32 bytes or less)
        let valid_name = AssetName::new(&[0u8; 32]);
        assert!(valid_name.is_ok());

        // Invalid asset name (too long)
        let invalid_name = AssetName::new(&[0u8; 33]);
        assert!(invalid_name.is_err());
        assert!(matches!(invalid_name.unwrap_err(), LedgerError::InvalidAsset(_)));
    }
}
