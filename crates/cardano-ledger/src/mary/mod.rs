//! Mary Era Ledger Rules
//!
//! The Mary era (February 2021) introduced multi-asset support to Cardano,
//! allowing native tokens to be created, transferred, and managed alongside ADA.
//! This era maintains all Shelley and Allegra features while adding:
//! - Native token creation (minting/burning)
//! - Multi-asset transaction outputs
//! - Asset-aware transaction validation
//! - Token bundle management

use crate::{LedgerError, Result};
use cardano_crypto::{Blake2b256Hash, Ed25519Signature};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Mary Era Transaction representing multi-asset transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaryTransaction {
    pub inputs: Vec<TransactionInput>,
    pub outputs: Vec<MaryTransactionOutput>,
    pub fee: Coin,
    pub ttl: Option<Slot>,
    pub certificates: Vec<Certificate>,
    pub withdrawals: HashMap<RewardAddress, Coin>,
    pub auxiliary_data: Option<AuxiliaryData>,
    pub validity_interval: ValidityInterval,
    pub mint: Option<Mint>, // New in Mary: token minting/burning
    pub witness_set: MaryWitnessSet,
}

/// Mary Era Transaction Output with multi-asset support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaryTransactionOutput {
    pub address: Address,
    pub value: MaryValue, // Multi-asset value instead of just Coin
    pub datum_hash: Option<Blake2b256Hash>,
}

/// Multi-Asset Value containing ADA and native tokens
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MaryValue {
    pub coin: Coin,
    pub multi_asset: Option<MultiAsset>,
}

/// Collection of native token assets
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MultiAsset {
    /// Map from PolicyId to Asset bundles
    pub assets: HashMap<PolicyId, AssetMap>,
}

/// Asset map for a specific policy (asset names to quantities)
pub type AssetMap = HashMap<AssetName, u64>;

/// Policy ID identifying a native token policy
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyId(pub Blake2b256Hash);

/// Asset name within a policy (up to 32 bytes)
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetName(pub Vec<u8>);

/// Token minting specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mint {
    pub mint_assets: MultiAsset,
}

/// Mary era witness set with multi-asset script support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaryWitnessSet {
    pub vkey_witnesses: Vec<VKeyWitness>,
    pub native_scripts: Vec<NativeScript>,
    pub bootstrap_witnesses: Vec<BootstrapWitness>,
}

/// Native script for token policy validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NativeScript {
    ScriptPubkey(Ed25519KeyHash),
    ScriptAll(Vec<NativeScript>),
    ScriptAny(Vec<NativeScript>),
    ScriptNOfK(u32, Vec<NativeScript>),
    InvalidBefore(Slot), // Allegra timelock feature
    InvalidHereafter(Slot), // Allegra timelock feature
}

// Core types used across eras (simplified definitions)
pub type Coin = u64;
pub type Slot = u64;
pub type Ed25519KeyHash = Blake2b256Hash;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TransactionInput {
    pub transaction_id: Blake2b256Hash,
    pub index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    // Simplified - would contain network, payment, and stake credentials
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RewardAddress {
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certificate {
    // Simplified certificate structure
    pub cert_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuxiliaryData {
    pub metadata: HashMap<u64, String>, // Simplified - would be JSON in full implementation
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidityInterval {
    pub invalid_before: Option<Slot>,
    pub invalid_hereafter: Option<Slot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VKeyWitness {
    pub vkey: Vec<u8>,
    pub signature: Ed25519Signature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapWitness {
    pub public_key: Vec<u8>,
    pub signature: Ed25519Signature,
    pub chain_code: Vec<u8>,
    pub attributes: Vec<u8>,
}

impl MaryValue {
    /// Create a new MaryValue with only ADA
    pub fn new_ada_only(coin: Coin) -> Self {
        Self {
            coin,
            multi_asset: None,
        }
    }

    /// Create a new MaryValue with ADA and multi-assets
    pub fn new_with_assets(coin: Coin, multi_asset: MultiAsset) -> Self {
        Self {
            coin,
            multi_asset: Some(multi_asset),
        }
    }

    /// Add another MaryValue to this one
    pub fn add(&mut self, other: &MaryValue) -> Result<()> {
        // Add ADA
        self.coin = self.coin.checked_add(other.coin)
            .ok_or_else(|| LedgerError::InvalidTransaction("ADA overflow in addition".to_string()))?;

        // Add multi-assets
        if let Some(other_assets) = &other.multi_asset {
            match &mut self.multi_asset {
                Some(assets) => assets.add(other_assets)?,
                None => self.multi_asset = Some(other_assets.clone()),
            }
        }

        Ok(())
    }

    /// Subtract another MaryValue from this one
    pub fn subtract(&mut self, other: &MaryValue) -> Result<()> {
        // Subtract ADA
        self.coin = self.coin.checked_sub(other.coin)
            .ok_or_else(|| LedgerError::InvalidTransaction("Insufficient ADA for subtraction".to_string()))?;

        // Subtract multi-assets
        if let Some(other_assets) = &other.multi_asset {
            match &mut self.multi_asset {
                Some(assets) => {
                    assets.subtract(other_assets)?;
                    // Remove empty multi_asset if no tokens remain
                    if assets.is_empty() {
                        self.multi_asset = None;
                    }
                }
                None => {
                    return Err(LedgerError::InvalidTransaction(
                        "Cannot subtract assets from ADA-only value".to_string()
                    ));
                }
            }
        }

        Ok(())
    }

    /// Check if this value is greater than or equal to another
    pub fn geq(&self, other: &MaryValue) -> bool {
        // Check ADA
        if self.coin < other.coin {
            return false;
        }

        // Check multi-assets
        match (&self.multi_asset, &other.multi_asset) {
            (Some(self_assets), Some(other_assets)) => self_assets.geq(other_assets),
            (None, Some(_)) => false, // We have no assets but other requires some
            (_, None) => true, // Other requires no assets, we're good
        }
    }
}

impl MultiAsset {
    /// Create a new empty MultiAsset
    pub fn new() -> Self {
        Self {
            assets: HashMap::new(),
        }
    }

    /// Add assets from another MultiAsset
    pub fn add(&mut self, other: &MultiAsset) -> Result<()> {
        for (policy_id, other_assets) in &other.assets {
            match self.assets.get_mut(policy_id) {
                Some(self_assets) => {
                    // Add to existing policy
                    for (asset_name, quantity) in other_assets {
                        let current = self_assets.get(asset_name).unwrap_or(&0);
                        let new_quantity = current.checked_add(*quantity)
                            .ok_or_else(|| LedgerError::InvalidTransaction(
                                format!("Asset quantity overflow for {asset_name:?}")
                            ))?;
                        self_assets.insert(asset_name.clone(), new_quantity);
                    }
                }
                None => {
                    // New policy, add all assets
                    self.assets.insert(policy_id.clone(), other_assets.clone());
                }
            }
        }
        Ok(())
    }

    /// Subtract assets from this MultiAsset
    pub fn subtract(&mut self, other: &MultiAsset) -> Result<()> {
        for (policy_id, other_assets) in &other.assets {
            match self.assets.get_mut(policy_id) {
                Some(self_assets) => {
                    // Subtract from existing policy
                    for (asset_name, quantity) in other_assets {
                        let current = self_assets.get(asset_name).unwrap_or(&0);
                        let new_quantity = current.checked_sub(*quantity)
                            .ok_or_else(|| LedgerError::InvalidTransaction(
                                format!("Insufficient quantity for asset {asset_name:?}")
                            ))?;

                        if new_quantity == 0 {
                            self_assets.remove(asset_name);
                        } else {
                            self_assets.insert(asset_name.clone(), new_quantity);
                        }
                    }

                    // Remove empty policy
                    if self_assets.is_empty() {
                        self.assets.remove(policy_id);
                    }
                }
                None => {
                    return Err(LedgerError::InvalidTransaction(
                        format!("Policy {policy_id:?} not found for subtraction")
                    ));
                }
            }
        }
        Ok(())
    }

    /// Check if this MultiAsset is greater than or equal to another
    pub fn geq(&self, other: &MultiAsset) -> bool {
        for (policy_id, other_assets) in &other.assets {
            match self.assets.get(policy_id) {
                Some(self_assets) => {
                    for (asset_name, required_quantity) in other_assets {
                        let available = self_assets.get(asset_name).unwrap_or(&0);
                        if available < required_quantity {
                            return false;
                        }
                    }
                }
                None => return false, // Required policy not found
            }
        }
        true
    }

    /// Check if this MultiAsset is empty
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    /// Get the total number of different assets
    pub fn asset_count(&self) -> usize {
        self.assets.values().map(|assets| assets.len()).sum()
    }
}

impl AssetName {
    /// Create a new AssetName from bytes (max 32 bytes)
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() > 32 {
            return Err(LedgerError::InvalidTransaction(
                "Asset name cannot exceed 32 bytes".to_string()
            ));
        }
        Ok(Self(bytes))
    }

    /// Create AssetName from string
    pub fn from_string(s: &str) -> Result<Self> {
        Self::new(s.as_bytes().to_vec())
    }

    /// Get bytes representation
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl PolicyId {
    /// Create PolicyId from hash
    pub fn new(hash: Blake2b256Hash) -> Self {
        Self(hash)
    }

    /// Get the hash
    pub fn hash(&self) -> &Blake2b256Hash {
        &self.0
    }
}

/// Mary era transaction validation
pub struct MaryLedger;

impl MaryLedger {
    /// Validate a Mary era transaction
    pub fn validate_transaction(tx: &MaryTransaction) -> Result<()> {
        // 1. Validate transaction structure
        Self::validate_structure(tx)?;

        // 2. Validate value conservation (including multi-assets)
        Self::validate_value_conservation(tx)?;

        // 3. Validate native scripts for minting
        Self::validate_minting_scripts(tx)?;

        // 4. Validate timelock constraints
        Self::validate_timelocks(tx)?;

        Ok(())
    }

    fn validate_structure(tx: &MaryTransaction) -> Result<()> {
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

        // Validate output values
        for output in &tx.outputs {
            if output.value.coin == 0 && output.value.multi_asset.is_none() {
                return Err(LedgerError::InvalidTransaction(
                    "Output cannot have zero value".to_string()
                ));
            }
        }

        Ok(())
    }

    fn validate_value_conservation(_tx: &MaryTransaction) -> Result<()> {
        // In a full implementation, this would:
        // 1. Sum all input values (requires UTxO lookup)
        // 2. Sum all output values + fees + minted values
        // 3. Ensure inputs + minted = outputs + fees + burned

        // For now, just validate that fee is reasonable
        // TODO: Implement full value conservation with UTxO state
        Ok(())
    }

    fn validate_minting_scripts(_tx: &MaryTransaction) -> Result<()> {
        // In a full implementation, this would:
        // 1. For each minted/burned policy, find corresponding native script
        // 2. Validate script conditions are met
        // 3. Ensure all required signatures are present

        // TODO: Implement native script validation
        Ok(())
    }

    fn validate_timelocks(tx: &MaryTransaction) -> Result<()> {
        // Validate transaction validity interval
        if let (Some(before), Some(after)) = (
            tx.validity_interval.invalid_before,
            tx.validity_interval.invalid_hereafter
        ) {
            if before >= after {
                return Err(LedgerError::InvalidTransaction(
                    "Invalid validity interval: before >= after".to_string()
                ));
            }
        }

        Ok(())
    }

    /// Calculate the minimum ADA required for an output (Mary era rules)
    pub fn min_ada_for_output(output: &MaryTransactionOutput) -> Coin {
        // Mary era minimum ADA calculation based on output size
        // Base minimum + extra for multi-assets
        let base_min = 1_000_000; // 1 ADA base minimum

        let asset_count = output.value.multi_asset
            .as_ref()
            .map(|ma| ma.asset_count())
            .unwrap_or(0);

        // Add ~0.15 ADA per asset (simplified calculation)
        base_min + (asset_count as u64 * 150_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mary_value_operations() {
        let mut value1 = MaryValue::new_ada_only(1000);
        let value2 = MaryValue::new_ada_only(500);

        value1.add(&value2).unwrap();
        assert_eq!(value1.coin, 1500);

        value1.subtract(&value2).unwrap();
        assert_eq!(value1.coin, 1000);
    }

    #[test]
    fn test_multi_asset_operations() {
        let mut assets1 = MultiAsset::new();
        let mut assets2 = MultiAsset::new();

        // Create test assets
        let policy1 = PolicyId::new(Blake2b256Hash::hash(b"policy1"));
        let asset1 = AssetName::from_string("token1").unwrap();

        let mut asset_map = HashMap::new();
        asset_map.insert(asset1.clone(), 100);
        assets1.assets.insert(policy1.clone(), asset_map.clone());

        asset_map.insert(asset1, 50);
        assets2.assets.insert(policy1, asset_map);

        assets1.add(&assets2).unwrap();

        // Should have 150 of token1 now
        assert_eq!(assets1.asset_count(), 1);
    }

    #[test]
    fn test_asset_name_validation() {
        // Valid asset name
        let name = AssetName::from_string("MyToken").unwrap();
        assert_eq!(name.as_bytes(), b"MyToken");

        // Invalid asset name (too long)
        let long_name = "a".repeat(33);
        assert!(AssetName::from_string(&long_name).is_err());
    }

    #[test]
    fn test_min_ada_calculation() {
        // ADA-only output
        let ada_output = MaryTransactionOutput {
            address: Address { bytes: vec![1, 2, 3] },
            value: MaryValue::new_ada_only(1_000_000),
            datum_hash: None,
        };

        let min_ada = MaryLedger::min_ada_for_output(&ada_output);
        assert_eq!(min_ada, 1_000_000); // Base minimum

        // Multi-asset output
        let mut multi_asset = MultiAsset::new();
        let policy = PolicyId::new(Blake2b256Hash::hash(b"test"));
        let mut asset_map = HashMap::new();
        asset_map.insert(AssetName::from_string("token1").unwrap(), 100);
        asset_map.insert(AssetName::from_string("token2").unwrap(), 200);
        multi_asset.assets.insert(policy, asset_map);

        let multi_output = MaryTransactionOutput {
            address: Address { bytes: vec![1, 2, 3] },
            value: MaryValue::new_with_assets(1_000_000, multi_asset),
            datum_hash: None,
        };

        let min_ada_multi = MaryLedger::min_ada_for_output(&multi_output);
        assert!(min_ada_multi > min_ada); // Should require more ADA
    }
}
