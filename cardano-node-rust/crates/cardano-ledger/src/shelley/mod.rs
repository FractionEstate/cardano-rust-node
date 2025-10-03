//! Shelley Era Ledger Implementation
//!
//! The Shelley era introduces stake pools, delegation, rewards distribution,
//! and enhanced address formats. This era forms the foundation for Cardano's
//! proof-of-stake consensus mechanism with decentralized stake pools.

use crate::{Coin, Epoch, LedgerError, Result, Slot};
use cardano_crypto::{Blake2b256Hash, Ed25519KeyHash, Ed25519Signature};
use std::collections::{BTreeMap, BTreeSet};

/// Shelley era transaction
#[derive(Debug, Clone)]
pub struct ShelleyTransaction {
    pub inputs: BTreeSet<ShelleyTxIn>,
    pub outputs: Vec<ShelleyTxOut>,
    pub fee: Coin,
    pub ttl: Option<Slot>,
    pub certificates: Vec<Certificate>,
    pub withdrawals: BTreeMap<RewardAddress, Coin>,
    pub auxiliary_data_hash: Option<Blake2b256Hash>,
}

/// Shelley transaction input
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShelleyTxIn {
    pub transaction_id: Blake2b256Hash,
    pub output_index: u32,
}

/// Shelley transaction output
#[derive(Debug, Clone)]
pub struct ShelleyTxOut {
    pub address: ShelleyAddress,
    pub amount: Coin,
}

/// Shelley address format
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShelleyAddress {
    pub network: NetworkId,
    pub payment_credential: StakeCredential,
    pub stake_credential: Option<StakeCredential>,
}

/// Network identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NetworkId {
    Testnet = 0,
    Mainnet = 1,
}

/// Stake credential for addresses and certificates
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StakeCredential {
    Key(Ed25519KeyHash),    // Key-based credential
    Script(Blake2b256Hash), // Script-based credential
}

/// Reward address for staking rewards
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RewardAddress {
    pub network: NetworkId,
    pub stake_credential: StakeCredential,
}

/// Certificates for staking operations
#[derive(Debug, Clone)]
pub enum Certificate {
    /// Register a stake key for delegation
    StakeRegistration(StakeCredential),
    /// Deregister a stake key (reclaim deposit)
    StakeDeregistration(StakeCredential),
    /// Delegate stake to a pool
    StakeDelegation {
        stake_credential: StakeCredential,
        pool_id: PoolId,
    },
    /// Register a new stake pool
    PoolRegistration(PoolRegistration),
    /// Retire a stake pool
    PoolRetirement {
        pool_id: PoolId,
        retirement_epoch: Epoch,
    },
}

/// Pool identifier (hash of pool operator's verification key)
pub type PoolId = Ed25519KeyHash;

/// Stake pool registration certificate
#[derive(Debug, Clone)]
pub struct PoolRegistration {
    pub pool_id: PoolId,
    pub vrf_key: VrfKey,
    pub pledge: Coin,
    pub cost: Coin,
    pub margin: UnitInterval,
    pub reward_account: RewardAddress,
    pub pool_owners: BTreeSet<Ed25519KeyHash>,
    pub relays: Vec<Relay>,
    pub pool_metadata: Option<PoolMetadata>,
}

/// VRF key for pool leadership
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VrfKey([u8; 32]);

/// Unit interval for representing percentages (0.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitInterval {
    numerator: u64,
    denominator: u64,
}

/// Pool relay information
#[derive(Debug, Clone)]
pub enum Relay {
    SingleHostAddr {
        port: Option<u16>,
        ipv4: Option<[u8; 4]>,
        ipv6: Option<[u8; 16]>,
    },
    SingleHostName {
        port: Option<u16>,
        dns_name: String,
    },
    MultiHostName {
        dns_name: String,
    },
}

/// Pool metadata reference
#[derive(Debug, Clone)]
pub struct PoolMetadata {
    pub url: String, // Max 64 characters
    pub metadata_hash: Blake2b256Hash,
}

/// Transaction witness set
#[derive(Debug, Clone)]
pub struct ShelleyWitnessSet {
    pub vkey_witnesses: BTreeSet<VKeyWitness>,
    pub native_scripts: BTreeMap<Blake2b256Hash, NativeScript>,
    pub bootstrap_witnesses: BTreeSet<BootstrapWitness>,
}

/// Verification key witness
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct VKeyWitness {
    pub vkey: Ed25519KeyHash,
    pub signature: Ed25519Signature,
}

/// Bootstrap witness for Byron-era addresses
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BootstrapWitness {
    pub public_key: Ed25519KeyHash,
    pub signature: Ed25519Signature,
    pub chain_code: [u8; 32],
    pub attributes: Vec<u8>,
}

/// Native script for multi-signature transactions
#[derive(Debug, Clone)]
pub enum NativeScript {
    /// Require a specific key signature
    ScriptPubkey(Ed25519KeyHash),
    /// Require all of the nested scripts
    ScriptAll(Vec<NativeScript>),
    /// Require any one of the nested scripts
    ScriptAny(Vec<NativeScript>),
    /// Require N of the nested scripts
    ScriptNOfK { n: u32, scripts: Vec<NativeScript> },
}

/// Shelley ledger state
#[derive(Debug, Clone)]
pub struct ShelleyLedgerState {
    pub utxo: ShelleyUtxo,
    pub deposited: Coin,
    pub fees: Coin,
    pub account_state: AccountState,
    pub pool_state: PoolState,
    pub delegation_state: DelegationState,
}

/// UTXO set for Shelley era
#[derive(Debug, Clone)]
pub struct ShelleyUtxo {
    pub utxo_map: BTreeMap<ShelleyTxIn, ShelleyTxOut>,
}

/// Account state tracking deposits and rewards
#[derive(Debug, Clone)]
pub struct AccountState {
    pub treasury: Coin,
    pub reserves: Coin,
    pub reward_accounts: BTreeMap<RewardAddress, Coin>,
    pub stake_credentials: BTreeMap<StakeCredential, Coin>, // Deposits
}

/// Pool state tracking registered pools
#[derive(Debug, Clone)]
pub struct PoolState {
    pub pool_params: BTreeMap<PoolId, PoolRegistration>,
    pub pool_deposits: BTreeMap<PoolId, Coin>,
    pub retiring_pools: BTreeMap<PoolId, Epoch>,
}

/// Delegation state tracking stake delegation
#[derive(Debug, Clone)]
pub struct DelegationState {
    pub delegations: BTreeMap<StakeCredential, PoolId>,
    pub ptr_map: BTreeMap<Pointer, StakeCredential>,
}

/// Pointer to a certificate within a transaction
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pointer {
    pub slot: Slot,
    pub tx_ix: u32,
    pub cert_ix: u32,
}

/// Shelley protocol parameters
#[derive(Debug, Clone)]
pub struct ShelleyProtocolParameters {
    pub min_fee_a: Coin,            // Linear fee coefficient
    pub min_fee_b: Coin,            // Linear fee constant
    pub max_block_body_size: u32,   // Maximum block body size
    pub max_tx_size: u32,           // Maximum transaction size
    pub max_block_header_size: u32, // Maximum block header size
    pub key_deposit: Coin,          // Deposit for stake key registration
    pub pool_deposit: Coin,         // Deposit for pool registration
    pub min_utxo: Coin,             // Minimum UTXO value
    pub min_pool_cost: Coin,        // Minimum pool cost
    pub price_mem: UnitInterval,    // Memory price for Plutus scripts
    pub price_step: UnitInterval,   // Step price for Plutus scripts
    pub max_tx_ex_mem: u64,         // Maximum transaction execution memory
    pub max_tx_ex_steps: u64,       // Maximum transaction execution steps
    pub max_block_ex_mem: u64,      // Maximum block execution memory
    pub max_block_ex_steps: u64,    // Maximum block execution steps
    pub max_val_size: u32,          // Maximum value size
    pub collateral_percentage: u32, // Collateral percentage
    pub max_collateral_inputs: u32, // Maximum collateral inputs
}

impl UnitInterval {
    /// Create a new unit interval (fraction between 0 and 1)
    pub fn new(numerator: u64, denominator: u64) -> Result<Self> {
        if denominator == 0 {
            return Err(LedgerError::InvalidTransaction(
                "Zero denominator in unit interval".to_string(),
            ));
        }
        if numerator > denominator {
            return Err(LedgerError::InvalidTransaction(
                "Unit interval numerator exceeds denominator".to_string(),
            ));
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    /// Get the fraction as a floating point number
    pub fn as_f64(&self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }

    /// Get numerator
    pub fn numerator(&self) -> u64 {
        self.numerator
    }

    /// Get denominator
    pub fn denominator(&self) -> u64 {
        self.denominator
    }
}

impl VrfKey {
    /// Create VRF key from bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get key as bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl ShelleyAddress {
    /// Create a payment address (with optional staking component)
    pub fn new_payment(
        network: NetworkId,
        payment_credential: StakeCredential,
        stake_credential: Option<StakeCredential>,
    ) -> Self {
        Self {
            network,
            payment_credential,
            stake_credential,
        }
    }

    /// Create an enterprise address (no staking component)
    pub fn new_enterprise(network: NetworkId, payment_credential: StakeCredential) -> Self {
        Self {
            network,
            payment_credential,
            stake_credential: None,
        }
    }

    /// Check if address has staking component
    pub fn has_staking(&self) -> bool {
        self.stake_credential.is_some()
    }

    /// Get the staking credential if present
    pub fn staking_credential(&self) -> Option<&StakeCredential> {
        self.stake_credential.as_ref()
    }
}

impl RewardAddress {
    /// Create a new reward address
    pub fn new(network: NetworkId, stake_credential: StakeCredential) -> Self {
        Self {
            network,
            stake_credential,
        }
    }
}

impl ShelleyTransaction {
    /// Calculate transaction ID (hash)
    pub fn tx_id(&self) -> Blake2b256Hash {
        let tx_data = format!("{:?}", self);
        Blake2b256Hash::hash(tx_data.as_bytes())
    }

    /// Validate transaction structure
    pub fn validate(&self, protocol_params: &ShelleyProtocolParameters) -> Result<()> {
        // Check inputs not empty
        if self.inputs.is_empty() {
            return Err(LedgerError::InvalidTransaction(
                "Transaction has no inputs".to_string(),
            ));
        }

        // Check outputs not empty
        if self.outputs.is_empty() {
            return Err(LedgerError::InvalidTransaction(
                "Transaction has no outputs".to_string(),
            ));
        }

        // Check fee is non-zero
        if self.fee == 0 {
            return Err(LedgerError::InvalidTransaction(
                "Transaction fee is zero".to_string(),
            ));
        }

        // Check outputs meet minimum UTXO requirement
        for output in &self.outputs {
            if output.amount < protocol_params.min_utxo {
                return Err(LedgerError::InvalidTransaction(
                    "Output below minimum UTXO".to_string(),
                ));
            }
        }

        // Validate certificates
        for cert in &self.certificates {
            cert.validate()?;
        }

        Ok(())
    }

    /// Calculate total output value
    pub fn total_output_value(&self) -> Result<Coin> {
        let mut total = 0u64;

        for output in &self.outputs {
            total = total
                .checked_add(output.amount)
                .ok_or_else(|| LedgerError::ValueOverflow("Output sum overflow".to_string()))?;
        }

        // Add withdrawals
        for &withdrawal in self.withdrawals.values() {
            total = total
                .checked_add(withdrawal)
                .ok_or_else(|| LedgerError::ValueOverflow("Withdrawal sum overflow".to_string()))?;
        }

        Ok(total)
    }

    /// Calculate total input value (requires UTxO context)
    pub fn total_input_value(&self, utxo: &ShelleyUtxo) -> Result<Coin> {
        let mut total = 0u64;

        for input in &self.inputs {
            let output = utxo.utxo_map.get(input).ok_or_else(|| {
                LedgerError::InvalidInput("Input not found in UTxO set".to_string())
            })?;
            total = total
                .checked_add(output.amount)
                .ok_or_else(|| LedgerError::ValueOverflow("Input sum overflow".to_string()))?;
        }

        Ok(total)
    }

    /// Calculate transaction fee
    pub fn calculate_min_fee(&self, protocol_params: &ShelleyProtocolParameters) -> Coin {
        let tx_size = self.estimate_size();
        protocol_params
            .min_fee_a
            .saturating_mul(tx_size as u64)
            .saturating_add(protocol_params.min_fee_b)
    }

    /// Estimate transaction size in bytes
    fn estimate_size(&self) -> u32 {
        // Simplified size estimation
        let base_size = 100; // Base transaction overhead
        let input_size = self.inputs.len() as u32 * 40; // ~40 bytes per input
        let output_size = self.outputs.len() as u32 * 50; // ~50 bytes per output
        let cert_size = self.certificates.len() as u32 * 80; // ~80 bytes per certificate
        let withdrawal_size = self.withdrawals.len() as u32 * 40; // ~40 bytes per withdrawal

        base_size + input_size + output_size + cert_size + withdrawal_size
    }
}

impl Certificate {
    /// Validate certificate structure
    pub fn validate(&self) -> Result<()> {
        match self {
            Certificate::StakeRegistration(_) => {
                // Stake registration is always valid structurally
                Ok(())
            }
            Certificate::StakeDeregistration(_) => {
                // Stake deregistration is always valid structurally
                Ok(())
            }
            Certificate::StakeDelegation { .. } => {
                // Delegation is always valid structurally
                Ok(())
            }
            Certificate::PoolRegistration(pool_reg) => pool_reg.validate(),
            Certificate::PoolRetirement { .. } => {
                // Pool retirement is always valid structurally
                Ok(())
            }
        }
    }

    /// Get the deposit required for this certificate
    pub fn deposit_amount(&self, protocol_params: &ShelleyProtocolParameters) -> Coin {
        match self {
            Certificate::StakeRegistration(_) => protocol_params.key_deposit,
            Certificate::PoolRegistration(_) => protocol_params.pool_deposit,
            _ => 0,
        }
    }

    /// Get the refund provided by this certificate
    pub fn refund_amount(&self, protocol_params: &ShelleyProtocolParameters) -> Coin {
        match self {
            Certificate::StakeDeregistration(_) => protocol_params.key_deposit,
            Certificate::PoolRetirement { .. } => protocol_params.pool_deposit,
            _ => 0,
        }
    }
}

impl PoolRegistration {
    /// Validate pool registration parameters
    pub fn validate(&self) -> Result<()> {
        // Check margin is valid unit interval
        if self.margin.numerator() > self.margin.denominator() {
            return Err(LedgerError::InvalidCertificate(
                "Invalid pool margin".to_string(),
            ));
        }

        // Check pool has at least one owner
        if self.pool_owners.is_empty() {
            return Err(LedgerError::InvalidCertificate(
                "Pool must have at least one owner".to_string(),
            ));
        }

        // Check metadata URL length
        if let Some(ref metadata) = self.pool_metadata {
            if metadata.url.len() > 64 {
                return Err(LedgerError::InvalidCertificate(
                    "Metadata URL too long".to_string(),
                ));
            }
        }

        Ok(())
    }
}

impl Default for ShelleyUtxo {
    fn default() -> Self {
        Self::new()
    }
}

impl ShelleyUtxo {
    /// Create new empty UTxO set
    pub fn new() -> Self {
        Self {
            utxo_map: BTreeMap::new(),
        }
    }

    /// Add output to UTxO set
    pub fn add_output(&mut self, tx_in: ShelleyTxIn, tx_out: ShelleyTxOut) {
        self.utxo_map.insert(tx_in, tx_out);
    }

    /// Remove output from UTxO set
    pub fn remove_output(&mut self, tx_in: &ShelleyTxIn) -> Option<ShelleyTxOut> {
        self.utxo_map.remove(tx_in)
    }

    /// Check if output exists
    pub fn contains(&self, tx_in: &ShelleyTxIn) -> bool {
        self.utxo_map.contains_key(tx_in)
    }

    /// Get output
    pub fn get(&self, tx_in: &ShelleyTxIn) -> Option<&ShelleyTxOut> {
        self.utxo_map.get(tx_in)
    }

    /// Apply transaction to UTxO set
    pub fn apply_transaction(&mut self, tx: &ShelleyTransaction) -> Result<()> {
        // Remove consumed inputs
        for input in &tx.inputs {
            if !self.contains(input) {
                return Err(LedgerError::InvalidInput(format!(
                    "Input {:?} not found in UTxO set",
                    input.transaction_id
                )));
            }
            self.remove_output(input);
        }

        // Add new outputs
        let tx_id = tx.tx_id();
        for (index, output) in tx.outputs.iter().enumerate() {
            let new_input = ShelleyTxIn {
                transaction_id: tx_id,
                output_index: index as u32,
            };
            self.add_output(new_input, output.clone());
        }

        Ok(())
    }

    /// Calculate total value in UTxO set
    pub fn total_value(&self) -> Result<Coin> {
        let mut total = 0u64;

        for output in self.utxo_map.values() {
            total = total
                .checked_add(output.amount)
                .ok_or_else(|| LedgerError::ValueOverflow("UTxO total overflow".to_string()))?;
        }

        Ok(total)
    }
}

impl Default for ShelleyLedgerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ShelleyLedgerState {
    /// Create new Shelley ledger state
    pub fn new() -> Self {
        Self {
            utxo: ShelleyUtxo::new(),
            deposited: 0,
            fees: 0,
            account_state: AccountState::new(),
            pool_state: PoolState::new(),
            delegation_state: DelegationState::new(),
        }
    }

    /// Apply transaction to ledger state
    pub fn apply_transaction(
        &mut self,
        tx: &ShelleyTransaction,
        protocol_params: &ShelleyProtocolParameters,
    ) -> Result<()> {
        // Validate transaction
        tx.validate(protocol_params)?;

        // Check balance
        let input_value = tx.total_input_value(&self.utxo)?;
        let output_value = tx.total_output_value()?;
        let total_output_with_fee = output_value
            .checked_add(tx.fee)
            .ok_or_else(|| LedgerError::ValueOverflow("Output + fee overflow".to_string()))?;

        if input_value < total_output_with_fee {
            return Err(LedgerError::InsufficientFunds(
                "Transaction outputs + fee exceed inputs".to_string(),
            ));
        }

        // Apply certificates
        for cert in &tx.certificates {
            self.apply_certificate(cert, protocol_params)?;
        }

        // Process withdrawals
        for (reward_addr, amount) in &tx.withdrawals {
            self.process_withdrawal(reward_addr, *amount)?;
        }

        // Update UTxO set
        self.utxo.apply_transaction(tx)?;

        // Add fee to collected fees
        self.fees = self
            .fees
            .checked_add(tx.fee)
            .ok_or_else(|| LedgerError::ValueOverflow("Fee accumulation overflow".to_string()))?;

        Ok(())
    }

    /// Apply a certificate to the ledger state
    fn apply_certificate(
        &mut self,
        cert: &Certificate,
        protocol_params: &ShelleyProtocolParameters,
    ) -> Result<()> {
        match cert {
            Certificate::StakeRegistration(stake_cred) => {
                let deposit = protocol_params.key_deposit;
                self.account_state
                    .stake_credentials
                    .insert(stake_cred.clone(), deposit);
                self.deposited = self
                    .deposited
                    .checked_add(deposit)
                    .ok_or_else(|| LedgerError::ValueOverflow("Deposit overflow".to_string()))?;
            }
            Certificate::StakeDeregistration(stake_cred) => {
                if let Some(deposit) = self.account_state.stake_credentials.remove(stake_cred) {
                    self.deposited = self.deposited.checked_sub(deposit).ok_or_else(|| {
                        LedgerError::ValueUnderflow("Deposit underflow".to_string())
                    })?;
                    // Refund goes to treasury (simplified)
                    self.account_state.treasury = self
                        .account_state
                        .treasury
                        .checked_add(deposit)
                        .ok_or_else(|| {
                            LedgerError::ValueOverflow("Treasury overflow".to_string())
                        })?;
                }
                // Remove delegation
                self.delegation_state.delegations.remove(stake_cred);
            }
            Certificate::StakeDelegation {
                stake_credential,
                pool_id,
            } => {
                // Check if stake credential is registered
                if !self
                    .account_state
                    .stake_credentials
                    .contains_key(stake_credential)
                {
                    return Err(LedgerError::InvalidCertificate(
                        "Stake credential not registered".to_string(),
                    ));
                }
                self.delegation_state
                    .delegations
                    .insert(stake_credential.clone(), *pool_id);
            }
            Certificate::PoolRegistration(pool_reg) => {
                let deposit = protocol_params.pool_deposit;
                self.pool_state
                    .pool_params
                    .insert(pool_reg.pool_id, pool_reg.clone());
                self.pool_state
                    .pool_deposits
                    .insert(pool_reg.pool_id, deposit);
                self.deposited = self.deposited.checked_add(deposit).ok_or_else(|| {
                    LedgerError::ValueOverflow("Pool deposit overflow".to_string())
                })?;
            }
            Certificate::PoolRetirement {
                pool_id,
                retirement_epoch,
            } => {
                if self.pool_state.pool_params.contains_key(pool_id) {
                    self.pool_state
                        .retiring_pools
                        .insert(*pool_id, *retirement_epoch);
                }
            }
        }
        Ok(())
    }

    /// Process a reward withdrawal
    fn process_withdrawal(&mut self, reward_addr: &RewardAddress, amount: Coin) -> Result<()> {
        let current_balance = self
            .account_state
            .reward_accounts
            .get(reward_addr)
            .copied()
            .unwrap_or(0);
        if current_balance < amount {
            return Err(LedgerError::InsufficientFunds(
                "Insufficient reward balance".to_string(),
            ));
        }

        let new_balance = current_balance - amount;
        if new_balance == 0 {
            self.account_state.reward_accounts.remove(reward_addr);
        } else {
            self.account_state
                .reward_accounts
                .insert(reward_addr.clone(), new_balance);
        }

        Ok(())
    }

    /// Distribute rewards to stake holders (simplified)
    pub fn distribute_rewards(&mut self, total_rewards: Coin) -> Result<()> {
        // Simplified reward distribution
        // In reality, this would be based on stake delegation and pool performance
        let num_delegators = self.delegation_state.delegations.len() as u64;
        if num_delegators == 0 {
            return Ok(());
        }

        let reward_per_delegator = total_rewards / num_delegators;
        if reward_per_delegator == 0 {
            return Ok(());
        }

        for stake_cred in self.delegation_state.delegations.keys() {
            if let StakeCredential::Key(_) = stake_cred {
                let reward_addr = RewardAddress::new(NetworkId::Mainnet, stake_cred.clone());
                let current_balance = self
                    .account_state
                    .reward_accounts
                    .get(&reward_addr)
                    .copied()
                    .unwrap_or(0);
                let new_balance = current_balance
                    .checked_add(reward_per_delegator)
                    .ok_or_else(|| {
                        LedgerError::ValueOverflow("Reward balance overflow".to_string())
                    })?;
                self.account_state
                    .reward_accounts
                    .insert(reward_addr, new_balance);
            }
        }

        Ok(())
    }
}

impl Default for AccountState {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountState {
    /// Create new account state
    pub fn new() -> Self {
        Self {
            treasury: 0,
            reserves: 0,
            reward_accounts: BTreeMap::new(),
            stake_credentials: BTreeMap::new(),
        }
    }
}

impl Default for PoolState {
    fn default() -> Self {
        Self::new()
    }
}

impl PoolState {
    /// Create new pool state
    pub fn new() -> Self {
        Self {
            pool_params: BTreeMap::new(),
            pool_deposits: BTreeMap::new(),
            retiring_pools: BTreeMap::new(),
        }
    }

    /// Get active pools (not retiring)
    pub fn active_pools(&self) -> Vec<&PoolId> {
        self.pool_params
            .keys()
            .filter(|pool_id| !self.retiring_pools.contains_key(pool_id))
            .collect()
    }
}

impl Default for DelegationState {
    fn default() -> Self {
        Self::new()
    }
}

impl DelegationState {
    /// Create new delegation state
    pub fn new() -> Self {
        Self {
            delegations: BTreeMap::new(),
            ptr_map: BTreeMap::new(),
        }
    }

    /// Get pool for stake credential
    pub fn get_delegation(&self, stake_cred: &StakeCredential) -> Option<&PoolId> {
        self.delegations.get(stake_cred)
    }
}

impl ShelleyProtocolParameters {
    /// Create mainnet protocol parameters
    pub fn mainnet() -> Self {
        Self {
            min_fee_a: 44,                                         // 0.000044 ADA per byte
            min_fee_b: 155381,                                     // 0.155381 ADA base fee
            max_block_body_size: 90112,                            // ~90KB
            max_tx_size: 16384,                                    // 16KB
            max_block_header_size: 1100,                           // 1.1KB
            key_deposit: 2_000_000,                                // 2 ADA
            pool_deposit: 500_000_000,                             // 500 ADA
            min_utxo: 1_000_000,                                   // 1 ADA
            min_pool_cost: 340_000_000,                            // 340 ADA
            price_mem: UnitInterval::new(577, 10000).unwrap(),     // 0.0577 per memory unit
            price_step: UnitInterval::new(721, 10000000).unwrap(), // 0.0000721 per step
            max_tx_ex_mem: 14_000_000,                             // 14M memory units
            max_tx_ex_steps: 10_000_000_000,                       // 10B steps
            max_block_ex_mem: 62_000_000,                          // 62M memory units
            max_block_ex_steps: 40_000_000_000,                    // 40B steps
            max_val_size: 5000,                                    // 5000 bytes
            collateral_percentage: 150,                            // 150% (1.5x)
            max_collateral_inputs: 3,                              // Maximum 3 collateral inputs
        }
    }

    /// Create testnet protocol parameters
    pub fn testnet() -> Self {
        let mut params = Self::mainnet();
        params.key_deposit = 2_000_000; // 2 ADA
        params.pool_deposit = 500_000_000; // 500 ADA
        params
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_interval() {
        let interval = UnitInterval::new(1, 4).unwrap();
        assert_eq!(interval.as_f64(), 0.25);
        assert_eq!(interval.numerator(), 1);
        assert_eq!(interval.denominator(), 4);

        // Test invalid intervals
        assert!(UnitInterval::new(5, 4).is_err()); // > 1.0
        assert!(UnitInterval::new(1, 0).is_err()); // Zero denominator
    }

    #[test]
    fn test_shelley_address() {
        let payment_cred = StakeCredential::Key(Ed25519KeyHash::from_test_data(b"payment"));
        let stake_cred = StakeCredential::Key(Ed25519KeyHash::from_test_data(b"stake"));

        let address = ShelleyAddress::new_payment(
            NetworkId::Testnet,
            payment_cred.clone(),
            Some(stake_cred.clone()),
        );

        assert_eq!(address.network, NetworkId::Testnet);
        assert_eq!(address.payment_credential, payment_cred);
        assert_eq!(address.stake_credential, Some(stake_cred));
        assert!(address.has_staking());

        let enterprise_addr = ShelleyAddress::new_enterprise(NetworkId::Mainnet, payment_cred);
        assert!(!enterprise_addr.has_staking());
    }

    #[test]
    fn test_reward_address() {
        let stake_cred = StakeCredential::Key(Ed25519KeyHash::from_test_data(b"stake"));
        let reward_addr = RewardAddress::new(NetworkId::Mainnet, stake_cred.clone());

        assert_eq!(reward_addr.network, NetworkId::Mainnet);
        assert_eq!(reward_addr.stake_credential, stake_cred);
    }

    #[test]
    fn test_pool_registration_validation() {
        let pool_reg = PoolRegistration {
            pool_id: Ed25519KeyHash::from_test_data(b"pool"),
            vrf_key: VrfKey::from_bytes([1; 32]),
            pledge: 1_000_000_000,                     // 1000 ADA
            cost: 340_000_000,                         // 340 ADA
            margin: UnitInterval::new(1, 20).unwrap(), // 5%
            reward_account: RewardAddress::new(
                NetworkId::Mainnet,
                StakeCredential::Key(Ed25519KeyHash::from_test_data(b"pool_owner")),
            ),
            pool_owners: vec![Ed25519KeyHash::from_test_data(b"owner1")]
                .into_iter()
                .collect(),
            relays: vec![],
            pool_metadata: None,
        };

        assert!(pool_reg.validate().is_ok());

        // Test invalid pool (no owners)
        let mut invalid_pool = pool_reg.clone();
        invalid_pool.pool_owners.clear();
        assert!(invalid_pool.validate().is_err());
    }

    #[test]
    fn test_shelley_transaction_validation() {
        let params = ShelleyProtocolParameters::testnet();

        let tx = ShelleyTransaction {
            inputs: vec![ShelleyTxIn {
                transaction_id: Blake2b256Hash::hash(b"input_tx"),
                output_index: 0,
            }]
            .into_iter()
            .collect(),
            outputs: vec![ShelleyTxOut {
                address: ShelleyAddress::new_enterprise(
                    NetworkId::Testnet,
                    StakeCredential::Key(Ed25519KeyHash::from_test_data(b"output")),
                ),
                amount: 2_000_000, // 2 ADA
            }],
            fee: 200_000, // 0.2 ADA
            ttl: Some(1000),
            certificates: vec![],
            withdrawals: BTreeMap::new(),
            auxiliary_data_hash: None,
        };

        assert!(tx.validate(&params).is_ok());

        // Test minimum fee calculation
        let min_fee = tx.calculate_min_fee(&params);
        assert!(min_fee > 0);
    }

    #[test]
    fn test_shelley_utxo_operations() {
        let mut utxo = ShelleyUtxo::new();

        let tx_in = ShelleyTxIn {
            transaction_id: Blake2b256Hash::hash(b"test_tx"),
            output_index: 0,
        };

        let tx_out = ShelleyTxOut {
            address: ShelleyAddress::new_enterprise(
                NetworkId::Testnet,
                StakeCredential::Key(Ed25519KeyHash::from_test_data(b"test")),
            ),
            amount: 1_000_000,
        };

        // Add output
        utxo.add_output(tx_in.clone(), tx_out.clone());
        assert!(utxo.contains(&tx_in));
        assert_eq!(utxo.get(&tx_in).unwrap().amount, 1_000_000);

        // Remove output
        let removed = utxo.remove_output(&tx_in);
        assert!(removed.is_some());
        assert!(!utxo.contains(&tx_in));
    }

    #[test]
    fn test_shelley_ledger_state() {
        let mut ledger = ShelleyLedgerState::new();
        let params = ShelleyProtocolParameters::testnet();

        // Create test UTxO
        let tx_in = ShelleyTxIn {
            transaction_id: Blake2b256Hash::hash(b"genesis_tx"),
            output_index: 0,
        };
        let tx_out = ShelleyTxOut {
            address: ShelleyAddress::new_enterprise(
                NetworkId::Testnet,
                StakeCredential::Key(Ed25519KeyHash::from_test_data(b"genesis")),
            ),
            amount: 10_000_000, // 10 ADA
        };
        ledger.utxo.add_output(tx_in.clone(), tx_out);

        // Create transaction
        let tx = ShelleyTransaction {
            inputs: vec![tx_in].into_iter().collect(),
            outputs: vec![ShelleyTxOut {
                address: ShelleyAddress::new_enterprise(
                    NetworkId::Testnet,
                    StakeCredential::Key(Ed25519KeyHash::from_test_data(b"output")),
                ),
                amount: 9_000_000, // 9 ADA
            }],
            fee: 1_000_000, // 1 ADA fee
            ttl: Some(1000),
            certificates: vec![],
            withdrawals: BTreeMap::new(),
            auxiliary_data_hash: None,
        };

        let result = ledger.apply_transaction(&tx, &params);
        assert!(
            result.is_ok(),
            "Transaction application failed: {:?}",
            result.err()
        );

        // Check that fee was collected
        assert_eq!(ledger.fees, 1_000_000);
    }

    #[test]
    fn test_stake_delegation_workflow() {
        let mut ledger = ShelleyLedgerState::new();
        let params = ShelleyProtocolParameters::testnet();

        let stake_cred = StakeCredential::Key(Ed25519KeyHash::from_test_data(b"delegator"));
        let pool_id = Ed25519KeyHash::from_test_data(b"pool");

        // First register stake key
        let stake_reg_cert = Certificate::StakeRegistration(stake_cred.clone());
        assert!(ledger.apply_certificate(&stake_reg_cert, &params).is_ok());

        // Check deposit was taken
        assert_eq!(ledger.deposited, params.key_deposit);
        assert!(ledger
            .account_state
            .stake_credentials
            .contains_key(&stake_cred));

        // Then delegate to pool
        let delegation_cert = Certificate::StakeDelegation {
            stake_credential: stake_cred.clone(),
            pool_id,
        };
        assert!(ledger.apply_certificate(&delegation_cert, &params).is_ok());

        // Check delegation was recorded
        assert_eq!(
            ledger.delegation_state.get_delegation(&stake_cred),
            Some(&pool_id)
        );
    }

    #[test]
    fn test_pool_registration_workflow() {
        let mut ledger = ShelleyLedgerState::new();
        let params = ShelleyProtocolParameters::testnet();

        let pool_reg = PoolRegistration {
            pool_id: Ed25519KeyHash::from_test_data(b"pool"),
            vrf_key: VrfKey::from_bytes([1; 32]),
            pledge: 1_000_000_000,
            cost: 340_000_000,
            margin: UnitInterval::new(1, 20).unwrap(),
            reward_account: RewardAddress::new(
                NetworkId::Testnet,
                StakeCredential::Key(Ed25519KeyHash::from_test_data(b"pool_owner")),
            ),
            pool_owners: vec![Ed25519KeyHash::from_test_data(b"owner1")]
                .into_iter()
                .collect(),
            relays: vec![],
            pool_metadata: None,
        };

        let pool_cert = Certificate::PoolRegistration(pool_reg.clone());
        assert!(ledger.apply_certificate(&pool_cert, &params).is_ok());

        // Check pool was registered
        assert!(ledger
            .pool_state
            .pool_params
            .contains_key(&pool_reg.pool_id));
        assert_eq!(ledger.deposited, params.pool_deposit);
        assert!(ledger
            .pool_state
            .active_pools()
            .contains(&&pool_reg.pool_id));
    }

    #[test]
    fn test_protocol_parameters() {
        let mainnet_params = ShelleyProtocolParameters::mainnet();
        let testnet_params = ShelleyProtocolParameters::testnet();

        // Check that parameters are reasonable
        assert!(mainnet_params.min_fee_a > 0);
        assert!(mainnet_params.min_fee_b > 0);
        assert!(mainnet_params.key_deposit >= 1_000_000); // At least 1 ADA
        assert!(mainnet_params.pool_deposit >= 100_000_000); // At least 100 ADA

        // Testnet should have same structure as mainnet
        assert_eq!(mainnet_params.min_fee_a, testnet_params.min_fee_a);
    }
}
