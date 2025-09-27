//! Shelley Era Validation Tests
//!
//! Tests for Shelley era ledger rules validation.
//! Shelley introduced proof-of-stake consensus and delegation:
//! - Stake pools and delegation
//! - Rewards and treasury
//! - Certificates for stake pool operations
//! - Multi-signature scripts (native scripts)

use cardano_ledger::{LedgerError, Result};
use cardano_crypto::{Ed25519Signature, Blake2b256Hash, Ed25519KeyHash};
use std::collections::HashMap;

/// Shelley address with stake delegation info
#[derive(Debug, Clone, PartialEq)]
pub struct ShelleyAddress {
    pub payment_credential: Credential,
    pub stake_credential: Option<StakeReference>,
    pub network_id: NetworkId,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Credential {
    Key(Ed25519KeyHash),
    Script(Blake2b256Hash),
}

#[derive(Debug, Clone, PartialEq)]
pub enum StakeReference {
    Credential(Credential),
    Pointer(StakePointer),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StakePointer {
    pub slot: u64,
    pub tx_index: u32,
    pub cert_index: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NetworkId {
    Testnet,
    Mainnet,
}

/// Shelley transaction input
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ShelleyTxIn {
    pub tx_id: Blake2b256Hash,
    pub output_index: u32,
}

/// Shelley transaction output
#[derive(Debug, Clone)]
pub struct ShelleyTxOut {
    pub address: ShelleyAddress,
    pub value: u64,
}

/// Shelley certificates for stake pool operations
#[derive(Debug, Clone)]
pub enum Certificate {
    StakeRegistration(Credential),
    StakeDeregistration(Credential),
    StakeDelegation {
        stake_credential: Credential,
        pool_id: Ed25519KeyHash,
    },
    PoolRegistration {
        pool_id: Ed25519KeyHash,
        vrf_key: Blake2b256Hash,
        pledge: u64,
        cost: u64,
        margin: f64,
        reward_account: ShelleyAddress,
        owners: Vec<Ed25519KeyHash>,
        relays: Vec<PoolRelay>,
        metadata: Option<PoolMetadata>,
    },
    PoolRetirement {
        pool_id: Ed25519KeyHash,
        epoch: u64,
    },
}

#[derive(Debug, Clone)]
pub struct PoolRelay {
    pub port: Option<u32>,
    pub host: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PoolMetadata {
    pub url: String,
    pub hash: Blake2b256Hash,
}

/// Shelley transaction with certificates
#[derive(Debug, Clone)]
pub struct ShelleyTransaction {
    pub inputs: Vec<ShelleyTxIn>,
    pub outputs: Vec<ShelleyTxOut>,
    pub fee: u64,
    pub ttl: Option<u64>, // Time to live (slot number)
    pub certificates: Vec<Certificate>,
    pub withdrawals: HashMap<ShelleyAddress, u64>,
}

/// Shelley UTXO
#[derive(Debug, Clone)]
pub struct ShelleyUtxo {
    pub tx_out: ShelleyTxOut,
    pub spent: bool,
}

/// Stake pool information
#[derive(Debug, Clone)]
pub struct StakePool {
    pub pool_id: Ed25519KeyHash,
    pub vrf_key: Blake2b256Hash,
    pub pledge: u64,
    pub cost: u64,
    pub margin: f64,
    pub reward_account: ShelleyAddress,
    pub owners: Vec<Ed25519KeyHash>,
    pub active: bool,
    pub retirement_epoch: Option<u64>,
}

/// Shelley ledger state
#[derive(Debug, Clone)]
pub struct ShelleyLedgerState {
    pub utxo_set: HashMap<ShelleyTxIn, ShelleyUtxo>,
    pub stake_pools: HashMap<Ed25519KeyHash, StakePool>,
    pub stake_credentials: HashMap<Credential, Option<Ed25519KeyHash>>, // credential -> delegated pool
    pub rewards: HashMap<ShelleyAddress, u64>,
    pub treasury: u64,
    pub reserves: u64,
    pub current_slot: u64,
    pub current_epoch: u64,
}

impl ShelleyTransaction {
    /// Validate transaction against Shelley era rules
    pub fn validate(&self, ledger_state: &ShelleyLedgerState) -> Result<()> {
        // Check TTL
        if let Some(ttl) = self.ttl {
            if ledger_state.current_slot > ttl {
                return Err(LedgerError::InvalidTransaction("Transaction expired".to_string()));
            }
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

        // Calculate input and output values
        let input_sum: u64 = self.inputs.iter()
            .map(|input| ledger_state.utxo_set[input].tx_out.value)
            .sum();

        let output_sum: u64 = self.outputs.iter()
            .map(|output| output.value)
            .sum();

        let withdrawal_sum: u64 = self.withdrawals.values().sum();

        // Check value conservation: inputs + withdrawals = outputs + fee
        if input_sum + withdrawal_sum != output_sum + self.fee {
            return Err(LedgerError::InvalidTransaction("Value not conserved".to_string()));
        }

        // Validate certificates
        for cert in &self.certificates {
            self.validate_certificate(cert, ledger_state)?;
        }

        // Validate withdrawals
        for (reward_addr, amount) in &self.withdrawals {
            let available_rewards = ledger_state.rewards.get(reward_addr).unwrap_or(&0);
            if amount > available_rewards {
                return Err(LedgerError::InvalidTransaction("Insufficient rewards for withdrawal".to_string()));
            }
        }

        Ok(())
    }

    fn validate_certificate(&self, cert: &Certificate, ledger_state: &ShelleyLedgerState) -> Result<()> {
        match cert {
            Certificate::StakeRegistration(cred) => {
                if ledger_state.stake_credentials.contains_key(cred) {
                    return Err(LedgerError::InvalidCertificate("Stake already registered".to_string()));
                }
            }

            Certificate::StakeDeregistration(cred) => {
                if !ledger_state.stake_credentials.contains_key(cred) {
                    return Err(LedgerError::InvalidCertificate("Stake not registered".to_string()));
                }
            }

            Certificate::StakeDelegation { stake_credential, pool_id } => {
                if !ledger_state.stake_credentials.contains_key(stake_credential) {
                    return Err(LedgerError::InvalidCertificate("Stake credential not registered".to_string()));
                }

                if !ledger_state.stake_pools.contains_key(pool_id) {
                    return Err(LedgerError::InvalidCertificate("Pool not registered".to_string()));
                }

                let pool = &ledger_state.stake_pools[pool_id];
                if !pool.active {
                    return Err(LedgerError::InvalidCertificate("Pool not active".to_string()));
                }
            }

            Certificate::PoolRegistration { pool_id, pledge, cost, margin, .. } => {
                if ledger_state.stake_pools.contains_key(pool_id) {
                    return Err(LedgerError::InvalidCertificate("Pool already registered".to_string()));
                }

                if *margin < 0.0 || *margin > 1.0 {
                    return Err(LedgerError::InvalidCertificate("Invalid pool margin".to_string()));
                }

                if *pledge == 0 {
                    return Err(LedgerError::InvalidCertificate("Pool pledge cannot be zero".to_string()));
                }
            }

            Certificate::PoolRetirement { pool_id, epoch } => {
                if !ledger_state.stake_pools.contains_key(pool_id) {
                    return Err(LedgerError::InvalidCertificate("Pool not registered".to_string()));
                }

                if *epoch <= ledger_state.current_epoch {
                    return Err(LedgerError::InvalidCertificate("Retirement epoch must be in future".to_string()));
                }
            }
        }

        Ok(())
    }

    pub fn hash(&self) -> Blake2b256Hash {
        Blake2b256Hash::new(&format!("{:?}", self).as_bytes())
    }
}

impl ShelleyLedgerState {
    pub fn new() -> Self {
        Self {
            utxo_set: HashMap::new(),
            stake_pools: HashMap::new(),
            stake_credentials: HashMap::new(),
            rewards: HashMap::new(),
            treasury: 0,
            reserves: 13_887_000_000_000_000, // Initial reserves in lovelace
            current_slot: 0,
            current_epoch: 0,
        }
    }

    pub fn apply_transaction(&mut self, tx: &ShelleyTransaction) -> Result<()> {
        tx.validate(self)?;

        // Process withdrawals
        for (reward_addr, amount) in &tx.withdrawals {
            if let Some(current_rewards) = self.rewards.get_mut(reward_addr) {
                *current_rewards -= amount;
            }
        }

        // Process certificates
        for cert in &tx.certificates {
            self.apply_certificate(cert)?;
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
            let utxo = ShelleyUtxo {
                tx_out: output.clone(),
                spent: false,
            };
            self.utxo_set.insert(tx_in, utxo);
        }

        // Add fee to treasury
        self.treasury += tx.fee;

        Ok(())
    }

    fn apply_certificate(&mut self, cert: &Certificate) -> Result<()> {
        match cert {
            Certificate::StakeRegistration(cred) => {
                self.stake_credentials.insert(cred.clone(), None);
            }

            Certificate::StakeDeregistration(cred) => {
                self.stake_credentials.remove(cred);
            }

            Certificate::StakeDelegation { stake_credential, pool_id } => {
                self.stake_credentials.insert(stake_credential.clone(), Some(pool_id.clone()));
            }

            Certificate::PoolRegistration { pool_id, vrf_key, pledge, cost, margin, reward_account, owners, .. } => {
                let pool = StakePool {
                    pool_id: pool_id.clone(),
                    vrf_key: vrf_key.clone(),
                    pledge: *pledge,
                    cost: *cost,
                    margin: *margin,
                    reward_account: reward_account.clone(),
                    owners: owners.clone(),
                    active: true,
                    retirement_epoch: None,
                };
                self.stake_pools.insert(pool_id.clone(), pool);
            }

            Certificate::PoolRetirement { pool_id, epoch } => {
                if let Some(pool) = self.stake_pools.get_mut(pool_id) {
                    pool.retirement_epoch = Some(*epoch);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_shelley_address_creation() {
        let addr = create_test_address();
        assert_eq!(addr.network_id, NetworkId::Testnet);
        assert!(matches!(addr.payment_credential, Credential::Key(_)));
    }

    #[test]
    fn test_shelley_transaction_ttl_validation() {
        let mut ledger = ShelleyLedgerState::new();
        ledger.current_slot = 1000;

        let tx = ShelleyTransaction {
            inputs: vec![],
            outputs: vec![],
            fee: 0,
            ttl: Some(500), // Expired TTL
            certificates: vec![],
            withdrawals: HashMap::new(),
        };

        let result = tx.validate(&ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidTransaction(_)));
    }

    #[test]
    fn test_shelley_stake_registration_certificate() {
        let mut ledger = ShelleyLedgerState::new();

        let stake_cred = Credential::Key(Ed25519KeyHash::new(b"test_stake_key"));
        let cert = Certificate::StakeRegistration(stake_cred.clone());

        let tx = ShelleyTransaction {
            inputs: vec![],
            outputs: vec![],
            fee: 2_000_000, // 2 ADA registration fee
            ttl: None,
            certificates: vec![cert],
            withdrawals: HashMap::new(),
        };

        // Should succeed for new stake credential
        let result = tx.validate(&ledger);
        assert!(result.is_ok());

        // Apply the transaction
        ledger.apply_transaction(&tx).unwrap();
        assert!(ledger.stake_credentials.contains_key(&stake_cred));

        // Should fail for already registered credential
        let result2 = tx.validate(&ledger);
        assert!(result2.is_err());
    }

    #[test]
    fn test_shelley_stake_delegation_validation() {
        let mut ledger = ShelleyLedgerState::new();

        let stake_cred = Credential::Key(Ed25519KeyHash::new(b"test_stake_key"));
        let pool_id = Ed25519KeyHash::new(b"test_pool_id");

        // Try to delegate without registering stake
        let cert = Certificate::StakeDelegation {
            stake_credential: stake_cred.clone(),
            pool_id: pool_id.clone(),
        };

        let tx = ShelleyTransaction {
            inputs: vec![],
            outputs: vec![],
            fee: 0,
            ttl: None,
            certificates: vec![cert],
            withdrawals: HashMap::new(),
        };

        let result = tx.validate(&ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidCertificate(_)));
    }

    #[test]
    fn test_shelley_pool_registration() {
        let ledger = ShelleyLedgerState::new();

        let pool_id = Ed25519KeyHash::new(b"test_pool_id");
        let cert = Certificate::PoolRegistration {
            pool_id: pool_id.clone(),
            vrf_key: Blake2b256Hash::new(b"test_vrf_key"),
            pledge: 100_000_000_000, // 100,000 ADA
            cost: 340_000_000,       // 340 ADA
            margin: 0.05,            // 5%
            reward_account: create_test_address(),
            owners: vec![Ed25519KeyHash::new(b"owner_key")],
            relays: vec![],
            metadata: None,
        };

        let tx = ShelleyTransaction {
            inputs: vec![],
            outputs: vec![],
            fee: 500_000_000, // 500 ADA registration fee
            ttl: None,
            certificates: vec![cert],
            withdrawals: HashMap::new(),
        };

        let result = tx.validate(&ledger);
        assert!(result.is_ok());
    }

    #[test]
    fn test_shelley_invalid_pool_margin() {
        let ledger = ShelleyLedgerState::new();

        let pool_id = Ed25519KeyHash::new(b"test_pool_id");
        let cert = Certificate::PoolRegistration {
            pool_id: pool_id.clone(),
            vrf_key: Blake2b256Hash::new(b"test_vrf_key"),
            pledge: 100_000_000_000,
            cost: 340_000_000,
            margin: 1.5, // Invalid margin > 1.0
            reward_account: create_test_address(),
            owners: vec![Ed25519KeyHash::new(b"owner_key")],
            relays: vec![],
            metadata: None,
        };

        let tx = ShelleyTransaction {
            inputs: vec![],
            outputs: vec![],
            fee: 500_000_000,
            ttl: None,
            certificates: vec![cert],
            withdrawals: HashMap::new(),
        };

        let result = tx.validate(&ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidCertificate(_)));
    }

    #[test]
    fn test_shelley_withdrawal_validation() {
        let mut ledger = ShelleyLedgerState::new();

        let reward_addr = create_test_address();
        ledger.rewards.insert(reward_addr.clone(), 1_000_000); // 1 ADA rewards

        let mut withdrawals = HashMap::new();
        withdrawals.insert(reward_addr.clone(), 2_000_000); // Try to withdraw 2 ADA

        let tx = ShelleyTransaction {
            inputs: vec![],
            outputs: vec![],
            fee: 0,
            ttl: None,
            certificates: vec![],
            withdrawals,
        };

        // Should fail - insufficient rewards
        let result = tx.validate(&ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidTransaction(_)));
    }
}
