//! Ledger database operation tests for Cardano Node Rust
//!
//! This module tests the ledger database functionality including:
//! - UTXO set management and operations
//! - Transaction validation and application
//! - Stake pool registration and operations
//! - Protocol parameter updates and governance
//! - Reward distribution and calculations
//! - Epoch boundary processing
//!
//! These tests ensure compatibility with the Haskell Cardano Node implementation.

use cardano_crypto::hash::{TxHash, StakePoolId, VrfKeyHash};
use cardano_ledger::{
    Address, Certificate, Coin, Epoch, PoolMetadata, PoolParams, ProtocolParameters,
    Stake, StakeCredential, Transaction, TxIn, TxOut, UTxO, UTxOSet, Withdrawal
};
use cardano_storage::{LedgerDB, LedgerDBError, LedgerState, RewardAccount};
use std::collections::{HashMap, BTreeMap, BTreeSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock Ledger Database implementation for testing
#[derive(Debug, Clone)]
pub struct MockLedgerDB {
    /// Current UTXO set
    utxo_set: Arc<RwLock<UTxOSet>>,
    /// Stake pool registrations
    stake_pools: Arc<RwLock<HashMap<StakePoolId, PoolParams>>>,
    /// Stake delegations (stake credential -> pool id)
    delegations: Arc<RwLock<HashMap<StakeCredential, StakePoolId>>>,
    /// Reward accounts and balances
    rewards: Arc<RwLock<HashMap<RewardAccount, Coin>>>,
    /// Current protocol parameters
    protocol_params: Arc<RwLock<ProtocolParameters>>,
    /// Current epoch
    current_epoch: Arc<RwLock<Epoch>>,
    /// Total stake distribution
    stake_distribution: Arc<RwLock<HashMap<StakePoolId, Coin>>>,
    /// Reserve and treasury funds
    reserves: Arc<RwLock<Coin>>,
    treasury: Arc<RwLock<Coin>>,
}

impl MockLedgerDB {
    pub fn new() -> Self {
        Self {
            utxo_set: Arc::new(RwLock::new(UTxOSet::new())),
            stake_pools: Arc::new(RwLock::new(HashMap::new())),
            delegations: Arc::new(RwLock::new(HashMap::new())),
            rewards: Arc::new(RwLock::new(HashMap::new())),
            protocol_params: Arc::new(RwLock::new(ProtocolParameters::default())),
            current_epoch: Arc::new(RwLock::new(Epoch(0))),
            stake_distribution: Arc::new(RwLock::new(HashMap::new())),
            reserves: Arc::new(RwLock::new(Coin::max_value())),
            treasury: Arc::new(RwLock::new(Coin::zero())),
        }
    }

    /// Apply a transaction to the ledger state
    async fn apply_transaction(&self, tx: &Transaction) -> Result<(), LedgerDBError> {
        // Validate transaction inputs exist in UTXO set
        let mut utxo_set = self.utxo_set.write().await;

        for input in &tx.inputs {
            if !utxo_set.contains(input) {
                return Err(LedgerDBError::InputNotFound(input.clone()));
            }
        }

        // Calculate input and output values
        let input_value: Coin = tx.inputs.iter()
            .map(|input| utxo_set.get(input).map(|utxo| utxo.value).unwrap_or(Coin::zero()))
            .sum();

        let output_value: Coin = tx.outputs.iter()
            .map(|output| output.value)
            .sum();

        let fee = tx.fee;

        // Validate transaction balances
        if input_value != output_value + fee {
            return Err(LedgerDBError::InvalidTransactionBalance {
                inputs: input_value,
                outputs: output_value,
                fee,
            });
        }

        // Remove consumed inputs
        for input in &tx.inputs {
            utxo_set.remove(input);
        }

        // Add new outputs
        for (index, output) in tx.outputs.iter().enumerate() {
            let tx_in = TxIn {
                tx_hash: tx.hash(),
                output_index: index as u32,
            };
            utxo_set.insert(tx_in, output.clone());
        }

        // Process certificates
        for cert in &tx.certificates {
            self.process_certificate(cert).await?;
        }

        // Process withdrawals
        let mut rewards = self.rewards.write().await;
        for (reward_account, amount) in &tx.withdrawals {
            let current_reward = rewards.get(reward_account).copied().unwrap_or(Coin::zero());
            if current_reward < *amount {
                return Err(LedgerDBError::InsufficientRewards {
                    account: reward_account.clone(),
                    available: current_reward,
                    requested: *amount,
                });
            }
            rewards.insert(reward_account.clone(), current_reward - *amount);
        }

        Ok(())
    }

    /// Process a certificate
    async fn process_certificate(&self, cert: &Certificate) -> Result<(), LedgerDBError> {
        match cert {
            Certificate::StakeRegistration(stake_cred) => {
                // Register stake credential
                let mut rewards = self.rewards.write().await;
                let reward_account = RewardAccount::from_stake_credential(stake_cred.clone());
                rewards.insert(reward_account, Coin::zero());
            }
            Certificate::StakeDeregistration(stake_cred) => {
                // Deregister stake credential
                let mut rewards = self.rewards.write().await;
                let mut delegations = self.delegations.write().await;
                let reward_account = RewardAccount::from_stake_credential(stake_cred.clone());

                // Remove rewards (should be withdrawn first)
                if let Some(balance) = rewards.get(&reward_account) {
                    if *balance > Coin::zero() {
                        return Err(LedgerDBError::NonZeroRewardBalance(reward_account));
                    }
                }
                rewards.remove(&reward_account);
                delegations.remove(stake_cred);
            }
            Certificate::StakeDelegation { stake_cred, pool_id } => {
                // Delegate stake to pool
                let mut delegations = self.delegations.write().await;
                let stake_pools = self.stake_pools.read().await;

                if !stake_pools.contains_key(pool_id) {
                    return Err(LedgerDBError::PoolNotFound(pool_id.clone()));
                }

                delegations.insert(stake_cred.clone(), pool_id.clone());
            }
            Certificate::PoolRegistration(pool_params) => {
                // Register stake pool
                let mut stake_pools = self.stake_pools.write().await;
                stake_pools.insert(pool_params.id.clone(), pool_params.clone());
            }
            Certificate::PoolRetirement { pool_id, epoch } => {
                // Schedule pool retirement
                let mut stake_pools = self.stake_pools.write().await;
                if let Some(pool_params) = stake_pools.get_mut(pool_id) {
                    pool_params.retirement_epoch = Some(*epoch);
                } else {
                    return Err(LedgerDBError::PoolNotFound(pool_id.clone()));
                }
            }
        }
        Ok(())
    }

    /// Get UTXO for a given transaction input
    async fn get_utxo(&self, tx_in: &TxIn) -> Option<TxOut> {
        self.utxo_set.read().await.get(tx_in).cloned()
    }

    /// Get all UTXOs for a given address
    async fn get_utxos_by_address(&self, address: &Address) -> Vec<(TxIn, TxOut)> {
        let utxo_set = self.utxo_set.read().await;
        utxo_set.iter()
            .filter(|(_, utxo)| &utxo.address == address)
            .map(|(tx_in, utxo)| (tx_in.clone(), utxo.clone()))
            .collect()
    }

    /// Get total UTXO set size
    async fn get_utxo_set_size(&self) -> usize {
        self.utxo_set.read().await.len()
    }

    /// Get total value in UTXO set
    async fn get_total_utxo_value(&self) -> Coin {
        self.utxo_set.read().await.values()
            .map(|utxo| utxo.value)
            .sum()
    }

    /// Register a new stake pool
    async fn register_stake_pool(&self, pool_params: PoolParams) -> Result<(), LedgerDBError> {
        let mut stake_pools = self.stake_pools.write().await;

        // Validate pool parameters
        if pool_params.cost > pool_params.pledge {
            return Err(LedgerDBError::InvalidPoolParameters("Cost exceeds pledge".to_string()));
        }

        if pool_params.margin > 1.0 {
            return Err(LedgerDBError::InvalidPoolParameters("Margin exceeds 100%".to_string()));
        }

        stake_pools.insert(pool_params.id.clone(), pool_params);
        Ok(())
    }

    /// Get stake pool information
    async fn get_stake_pool(&self, pool_id: &StakePoolId) -> Option<PoolParams> {
        self.stake_pools.read().await.get(pool_id).cloned()
    }

    /// Get all registered stake pools
    async fn get_all_stake_pools(&self) -> Vec<PoolParams> {
        self.stake_pools.read().await.values().cloned().collect()
    }

    /// Delegate stake to a pool
    async fn delegate_stake(&self, stake_cred: StakeCredential, pool_id: StakePoolId) -> Result<(), LedgerDBError> {
        let stake_pools = self.stake_pools.read().await;
        if !stake_pools.contains_key(&pool_id) {
            return Err(LedgerDBError::PoolNotFound(pool_id));
        }
        drop(stake_pools);

        let mut delegations = self.delegations.write().await;
        delegations.insert(stake_cred, pool_id);
        Ok(())
    }

    /// Get delegation for a stake credential
    async fn get_delegation(&self, stake_cred: &StakeCredential) -> Option<StakePoolId> {
        self.delegations.read().await.get(stake_cred).cloned()
    }

    /// Update protocol parameters
    async fn update_protocol_parameters(&self, new_params: ProtocolParameters) -> Result<(), LedgerDBError> {
        // Validate protocol parameters
        if new_params.min_fee_a == 0 || new_params.min_fee_b == 0 {
            return Err(LedgerDBError::InvalidProtocolParameters("Fee parameters cannot be zero".to_string()));
        }

        if new_params.max_tx_size == 0 {
            return Err(LedgerDBError::InvalidProtocolParameters("Max transaction size cannot be zero".to_string()));
        }

        *self.protocol_params.write().await = new_params;
        Ok(())
    }

    /// Get current protocol parameters
    async fn get_protocol_parameters(&self) -> ProtocolParameters {
        self.protocol_params.read().await.clone()
    }

    /// Process epoch boundary
    async fn process_epoch_boundary(&self, new_epoch: Epoch) -> Result<(), LedgerDBError> {
        *self.current_epoch.write().await = new_epoch;

        // Calculate and distribute rewards
        self.calculate_rewards().await?;

        // Process pool retirements
        self.process_pool_retirements(new_epoch).await?;

        // Update stake distribution
        self.update_stake_distribution().await?;

        Ok(())
    }

    /// Calculate and distribute rewards
    async fn calculate_rewards(&self) -> Result<(), LedgerDBError> {
        let total_stake = self.get_total_utxo_value().await;
        let mut rewards = self.rewards.write().await;
        let delegations = self.delegations.read().await;
        let stake_pools = self.stake_pools.read().await;

        // Simple reward calculation (in reality this would be much more complex)
        let epoch_rewards = Coin(1000000); // 1 ADA worth of rewards per epoch

        for (stake_cred, pool_id) in delegations.iter() {
            if let Some(pool_params) = stake_pools.get(pool_id) {
                // Calculate delegator's share based on their stake
                let delegator_stake = self.get_stake_for_credential(stake_cred).await;
                let pool_total_stake = self.get_pool_total_stake(pool_id).await;

                if pool_total_stake > Coin::zero() {
                    let delegator_share = (delegator_stake.0 * epoch_rewards.0) / pool_total_stake.0;
                    let delegator_reward = Coin(delegator_share);

                    let reward_account = RewardAccount::from_stake_credential(stake_cred.clone());
                    let current_reward = rewards.get(&reward_account).copied().unwrap_or(Coin::zero());
                    rewards.insert(reward_account, current_reward + delegator_reward);
                }
            }
        }

        Ok(())
    }

    /// Process pool retirements for the current epoch
    async fn process_pool_retirements(&self, current_epoch: Epoch) -> Result<(), LedgerDBError> {
        let mut stake_pools = self.stake_pools.write().await;
        let mut pools_to_remove = Vec::new();

        for (pool_id, pool_params) in stake_pools.iter() {
            if let Some(retirement_epoch) = pool_params.retirement_epoch {
                if retirement_epoch <= current_epoch {
                    pools_to_remove.push(pool_id.clone());
                }
            }
        }

        for pool_id in pools_to_remove {
            stake_pools.remove(&pool_id);

            // Remove delegations to retired pool
            let mut delegations = self.delegations.write().await;
            delegations.retain(|_, delegated_pool_id| *delegated_pool_id != pool_id);
        }

        Ok(())
    }

    /// Update stake distribution among pools
    async fn update_stake_distribution(&self) -> Result<(), LedgerDBError> {
        let mut stake_distribution = self.stake_distribution.write().await;
        stake_distribution.clear();

        let delegations = self.delegations.read().await;

        for (stake_cred, pool_id) in delegations.iter() {
            let stake = self.get_stake_for_credential(stake_cred).await;
            let current_stake = stake_distribution.get(pool_id).copied().unwrap_or(Coin::zero());
            stake_distribution.insert(pool_id.clone(), current_stake + stake);
        }

        Ok(())
    }

    /// Get stake amount for a stake credential
    async fn get_stake_for_credential(&self, _stake_cred: &StakeCredential) -> Coin {
        // Simplified: in reality this would involve calculating based on UTXOs
        // controlled by addresses associated with the stake credential
        Coin(1000000) // 1 ADA
    }

    /// Get total stake delegated to a pool
    async fn get_pool_total_stake(&self, pool_id: &StakePoolId) -> Coin {
        self.stake_distribution.read().await
            .get(pool_id)
            .copied()
            .unwrap_or(Coin::zero())
    }

    /// Get reward balance for an account
    async fn get_reward_balance(&self, account: &RewardAccount) -> Coin {
        self.rewards.read().await
            .get(account)
            .copied()
            .unwrap_or(Coin::zero())
    }

    /// Withdraw rewards
    async fn withdraw_rewards(&self, account: &RewardAccount, amount: Coin) -> Result<(), LedgerDBError> {
        let mut rewards = self.rewards.write().await;
        let current_balance = rewards.get(account).copied().unwrap_or(Coin::zero());

        if current_balance < amount {
            return Err(LedgerDBError::InsufficientRewards {
                account: account.clone(),
                available: current_balance,
                requested: amount,
            });
        }

        rewards.insert(account.clone(), current_balance - amount);
        Ok(())
    }
}

// Test helper functions
fn create_test_tx_in(tx_hash: TxHash, output_index: u32) -> TxIn {
    TxIn { tx_hash, output_index }
}

fn create_test_tx_out(address: Address, value: Coin) -> TxOut {
    TxOut { address, value }
}

fn create_test_transaction(
    inputs: Vec<TxIn>,
    outputs: Vec<TxOut>,
    fee: Coin,
    certificates: Vec<Certificate>,
    withdrawals: BTreeMap<RewardAccount, Coin>,
) -> Transaction {
    Transaction {
        inputs,
        outputs,
        fee,
        certificates,
        withdrawals,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_ledgerdb_utxo_management() {
        let ledgerdb = MockLedgerDB::new();

        // Create initial UTXOs
        let address = Address::from_bech32("addr_test1").unwrap();
        let tx_hash = TxHash::from_hex("abcd1234").unwrap();
        let tx_in = create_test_tx_in(tx_hash, 0);
        let tx_out = create_test_tx_out(address.clone(), Coin(1000000));

        // Manually add UTXO to set for testing
        {
            let mut utxo_set = ledgerdb.utxo_set.write().await;
            utxo_set.insert(tx_in.clone(), tx_out.clone());
        }

        // Test UTXO retrieval
        let retrieved_utxo = ledgerdb.get_utxo(&tx_in).await;
        assert!(retrieved_utxo.is_some());
        assert_eq!(retrieved_utxo.unwrap().value, Coin(1000000));

        // Test UTXOs by address
        let utxos = ledgerdb.get_utxos_by_address(&address).await;
        assert_eq!(utxos.len(), 1);
        assert_eq!(utxos[0].0, tx_in);
        assert_eq!(utxos[0].1.value, Coin(1000000));

        // Test UTXO set size and value
        let size = ledgerdb.get_utxo_set_size().await;
        assert_eq!(size, 1);

        let total_value = ledgerdb.get_total_utxo_value().await;
        assert_eq!(total_value, Coin(1000000));
    }

    #[tokio::test]
    async fn test_ledgerdb_transaction_validation() {
        let ledgerdb = MockLedgerDB::new();

        // Setup initial UTXO
        let address = Address::from_bech32("addr_test1").unwrap();
        let tx_hash = TxHash::from_hex("abcd1234").unwrap();
        let input = create_test_tx_in(tx_hash, 0);
        let utxo = create_test_tx_out(address.clone(), Coin(1000000));

        {
            let mut utxo_set = ledgerdb.utxo_set.write().await;
            utxo_set.insert(input.clone(), utxo);
        }

        // Create valid transaction
        let output1 = create_test_tx_out(address.clone(), Coin(500000));
        let output2 = create_test_tx_out(address.clone(), Coin(400000));
        let fee = Coin(100000);

        let tx = create_test_transaction(
            vec![input],
            vec![output1, output2],
            fee,
            vec![],
            BTreeMap::new(),
        );

        // Apply transaction should succeed
        let result = ledgerdb.apply_transaction(&tx).await;
        assert!(result.is_ok());

        // Verify UTXO set updated correctly
        let size = ledgerdb.get_utxo_set_size().await;
        assert_eq!(size, 2); // Original UTXO consumed, 2 new outputs created

        let total_value = ledgerdb.get_total_utxo_value().await;
        assert_eq!(total_value, Coin(900000)); // 1M - 100K fee
    }

    #[tokio::test]
    async fn test_ledgerdb_invalid_transaction_balance() {
        let ledgerdb = MockLedgerDB::new();

        // Setup initial UTXO
        let address = Address::from_bech32("addr_test1").unwrap();
        let tx_hash = TxHash::from_hex("abcd1234").unwrap();
        let input = create_test_tx_in(tx_hash, 0);
        let utxo = create_test_tx_out(address.clone(), Coin(1000000));

        {
            let mut utxo_set = ledgerdb.utxo_set.write().await;
            utxo_set.insert(input.clone(), utxo);
        }

        // Create invalid transaction (outputs + fee > inputs)
        let output = create_test_tx_out(address, Coin(1100000)); // More than input
        let fee = Coin(100000);

        let tx = create_test_transaction(
            vec![input],
            vec![output],
            fee,
            vec![],
            BTreeMap::new(),
        );

        // Apply transaction should fail
        let result = ledgerdb.apply_transaction(&tx).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerDBError::InvalidTransactionBalance { .. }));
    }

    #[tokio::test]
    async fn test_ledgerdb_missing_input() {
        let ledgerdb = MockLedgerDB::new();

        // Create transaction with non-existent input
        let address = Address::from_bech32("addr_test1").unwrap();
        let fake_hash = TxHash::from_hex("deadbeef").unwrap();
        let input = create_test_tx_in(fake_hash, 0);
        let output = create_test_tx_out(address, Coin(500000));

        let tx = create_test_transaction(
            vec![input.clone()],
            vec![output],
            Coin(0),
            vec![],
            BTreeMap::new(),
        );

        // Apply transaction should fail
        let result = ledgerdb.apply_transaction(&tx).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerDBError::InputNotFound(_)));
    }

    #[tokio::test]
    async fn test_ledgerdb_stake_pool_registration() {
        let ledgerdb = MockLedgerDB::new();

        let pool_id = StakePoolId::from_hex("pool123").unwrap();
        let vrf_key = VrfKeyHash::from_hex("vrf456").unwrap();

        let pool_params = PoolParams {
            id: pool_id.clone(),
            vrf_key_hash: vrf_key,
            pledge: Coin(10000000), // 10 ADA
            cost: Coin(340000000), // 340 ADA
            margin: 0.05, // 5%
            reward_account: RewardAccount::from_bech32("stake_test1").unwrap(),
            owners: vec![],
            relays: vec![],
            metadata: None,
            retirement_epoch: None,
        };

        // Register stake pool
        let result = ledgerdb.register_stake_pool(pool_params.clone()).await;
        assert!(result.is_ok());

        // Verify pool can be retrieved
        let retrieved_pool = ledgerdb.get_stake_pool(&pool_id).await;
        assert!(retrieved_pool.is_some());
        assert_eq!(retrieved_pool.unwrap().id, pool_id);

        // Verify in all pools list
        let all_pools = ledgerdb.get_all_stake_pools().await;
        assert_eq!(all_pools.len(), 1);
        assert_eq!(all_pools[0].id, pool_id);
    }

    #[tokio::test]
    async fn test_ledgerdb_invalid_pool_parameters() {
        let ledgerdb = MockLedgerDB::new();

        let pool_id = StakePoolId::from_hex("pool123").unwrap();
        let vrf_key = VrfKeyHash::from_hex("vrf456").unwrap();

        // Create pool with cost > pledge (invalid)
        let invalid_pool = PoolParams {
            id: pool_id,
            vrf_key_hash: vrf_key,
            pledge: Coin(1000000), // 1 ADA
            cost: Coin(2000000),   // 2 ADA (more than pledge)
            margin: 0.05,
            reward_account: RewardAccount::from_bech32("stake_test1").unwrap(),
            owners: vec![],
            relays: vec![],
            metadata: None,
            retirement_epoch: None,
        };

        // Registration should fail
        let result = ledgerdb.register_stake_pool(invalid_pool).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerDBError::InvalidPoolParameters(_)));
    }

    #[tokio::test]
    async fn test_ledgerdb_stake_delegation() {
        let ledgerdb = MockLedgerDB::new();

        // First register a stake pool
        let pool_id = StakePoolId::from_hex("pool123").unwrap();
        let pool_params = PoolParams {
            id: pool_id.clone(),
            vrf_key_hash: VrfKeyHash::from_hex("vrf456").unwrap(),
            pledge: Coin(10000000),
            cost: Coin(340000000),
            margin: 0.05,
            reward_account: RewardAccount::from_bech32("stake_test1").unwrap(),
            owners: vec![],
            relays: vec![],
            metadata: None,
            retirement_epoch: None,
        };

        ledgerdb.register_stake_pool(pool_params).await.unwrap();

        // Delegate stake to the pool
        let stake_cred = StakeCredential::from_keyhash([1; 28]);
        let result = ledgerdb.delegate_stake(stake_cred.clone(), pool_id.clone()).await;
        assert!(result.is_ok());

        // Verify delegation
        let delegation = ledgerdb.get_delegation(&stake_cred).await;
        assert!(delegation.is_some());
        assert_eq!(delegation.unwrap(), pool_id);
    }

    #[tokio::test]
    async fn test_ledgerdb_delegate_to_nonexistent_pool() {
        let ledgerdb = MockLedgerDB::new();

        let fake_pool_id = StakePoolId::from_hex("fake123").unwrap();
        let stake_cred = StakeCredential::from_keyhash([1; 28]);

        // Attempt to delegate to non-existent pool should fail
        let result = ledgerdb.delegate_stake(stake_cred, fake_pool_id.clone()).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerDBError::PoolNotFound(_)));
    }

    #[tokio::test]
    async fn test_ledgerdb_protocol_parameters() {
        let ledgerdb = MockLedgerDB::new();

        // Get initial parameters
        let initial_params = ledgerdb.get_protocol_parameters().await;
        assert!(initial_params.min_fee_a > 0);

        // Update parameters
        let new_params = ProtocolParameters {
            min_fee_a: 500,
            min_fee_b: 200000,
            max_tx_size: 16384,
            key_deposit: Coin(2000000),
            pool_deposit: Coin(500000000),
            ..initial_params
        };

        let result = ledgerdb.update_protocol_parameters(new_params.clone()).await;
        assert!(result.is_ok());

        // Verify parameters updated
        let updated_params = ledgerdb.get_protocol_parameters().await;
        assert_eq!(updated_params.min_fee_a, 500);
        assert_eq!(updated_params.max_tx_size, 16384);
    }

    #[tokio::test]
    async fn test_ledgerdb_invalid_protocol_parameters() {
        let ledgerdb = MockLedgerDB::new();

        let initial_params = ledgerdb.get_protocol_parameters().await;

        // Try to set invalid parameters (zero fee)
        let invalid_params = ProtocolParameters {
            min_fee_a: 0, // Invalid
            min_fee_b: 0, // Invalid
            ..initial_params
        };

        let result = ledgerdb.update_protocol_parameters(invalid_params).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerDBError::InvalidProtocolParameters(_)));
    }

    #[tokio::test]
    async fn test_ledgerdb_reward_distribution() {
        let ledgerdb = MockLedgerDB::new();

        // Setup stake pool and delegation
        let pool_id = StakePoolId::from_hex("pool123").unwrap();
        let pool_params = PoolParams {
            id: pool_id.clone(),
            vrf_key_hash: VrfKeyHash::from_hex("vrf456").unwrap(),
            pledge: Coin(10000000),
            cost: Coin(340000000),
            margin: 0.05,
            reward_account: RewardAccount::from_bech32("stake_test1").unwrap(),
            owners: vec![],
            relays: vec![],
            metadata: None,
            retirement_epoch: None,
        };

        ledgerdb.register_stake_pool(pool_params).await.unwrap();

        let stake_cred = StakeCredential::from_keyhash([1; 28]);
        ledgerdb.delegate_stake(stake_cred.clone(), pool_id.clone()).await.unwrap();

        // Register reward account
        let reward_account = RewardAccount::from_stake_credential(stake_cred);
        {
            let mut rewards = ledgerdb.rewards.write().await;
            rewards.insert(reward_account.clone(), Coin::zero());
        }

        // Process epoch boundary to trigger reward calculation
        let result = ledgerdb.process_epoch_boundary(Epoch(1)).await;
        assert!(result.is_ok());

        // Check that rewards were distributed
        let reward_balance = ledgerdb.get_reward_balance(&reward_account).await;
        assert!(reward_balance > Coin::zero());
    }

    #[tokio::test]
    async fn test_ledgerdb_reward_withdrawal() {
        let ledgerdb = MockLedgerDB::new();

        let reward_account = RewardAccount::from_bech32("stake_test1").unwrap();

        // Add some rewards
        {
            let mut rewards = ledgerdb.rewards.write().await;
            rewards.insert(reward_account.clone(), Coin(1000000)); // 1 ADA
        }

        // Withdraw part of rewards
        let result = ledgerdb.withdraw_rewards(&reward_account, Coin(400000)).await;
        assert!(result.is_ok());

        // Verify balance updated
        let balance = ledgerdb.get_reward_balance(&reward_account).await;
        assert_eq!(balance, Coin(600000)); // 1 ADA - 0.4 ADA

        // Try to withdraw more than available
        let result = ledgerdb.withdraw_rewards(&reward_account, Coin(1000000)).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerDBError::InsufficientRewards { .. }));
    }

    #[tokio::test]
    async fn test_ledgerdb_pool_retirement() {
        let ledgerdb = MockLedgerDB::new();

        // Register a stake pool
        let pool_id = StakePoolId::from_hex("pool123").unwrap();
        let mut pool_params = PoolParams {
            id: pool_id.clone(),
            vrf_key_hash: VrfKeyHash::from_hex("vrf456").unwrap(),
            pledge: Coin(10000000),
            cost: Coin(340000000),
            margin: 0.05,
            reward_account: RewardAccount::from_bech32("stake_test1").unwrap(),
            owners: vec![],
            relays: vec![],
            metadata: None,
            retirement_epoch: Some(Epoch(2)), // Retire at epoch 2
        };

        ledgerdb.register_stake_pool(pool_params.clone()).await.unwrap();

        // Delegate stake to the pool
        let stake_cred = StakeCredential::from_keyhash([1; 28]);
        ledgerdb.delegate_stake(stake_cred.clone(), pool_id.clone()).await.unwrap();

        // Process epoch boundary at retirement epoch
        let result = ledgerdb.process_epoch_boundary(Epoch(2)).await;
        assert!(result.is_ok());

        // Verify pool is removed
        let pool = ledgerdb.get_stake_pool(&pool_id).await;
        assert!(pool.is_none());

        // Verify delegation is removed
        let delegation = ledgerdb.get_delegation(&stake_cred).await;
        assert!(delegation.is_none());
    }

    #[tokio::test]
    async fn test_ledgerdb_stake_distribution() {
        let ledgerdb = MockLedgerDB::new();

        // Register two stake pools
        let pool1_id = StakePoolId::from_hex("pool111").unwrap();
        let pool2_id = StakePoolId::from_hex("pool222").unwrap();

        for (i, pool_id) in [&pool1_id, &pool2_id].iter().enumerate() {
            let pool_params = PoolParams {
                id: (*pool_id).clone(),
                vrf_key_hash: VrfKeyHash::from_hex(&format!("vrf{}", i + 1)).unwrap(),
                pledge: Coin(10000000),
                cost: Coin(340000000),
                margin: 0.05,
                reward_account: RewardAccount::from_bech32("stake_test1").unwrap(),
                owners: vec![],
                relays: vec![],
                metadata: None,
                retirement_epoch: None,
            };
            ledgerdb.register_stake_pool(pool_params).await.unwrap();
        }

        // Create delegations to both pools
        let stake_cred1 = StakeCredential::from_keyhash([1; 28]);
        let stake_cred2 = StakeCredential::from_keyhash([2; 28]);
        let stake_cred3 = StakeCredential::from_keyhash([3; 28]);

        ledgerdb.delegate_stake(stake_cred1, pool1_id.clone()).await.unwrap();
        ledgerdb.delegate_stake(stake_cred2, pool1_id.clone()).await.unwrap();
        ledgerdb.delegate_stake(stake_cred3, pool2_id.clone()).await.unwrap();

        // Update stake distribution
        ledgerdb.update_stake_distribution().await.unwrap();

        // Verify stake distribution
        let pool1_stake = ledgerdb.get_pool_total_stake(&pool1_id).await;
        let pool2_stake = ledgerdb.get_pool_total_stake(&pool2_id).await;

        assert_eq!(pool1_stake, Coin(2000000)); // 2 delegators * 1 ADA each
        assert_eq!(pool2_stake, Coin(1000000));  // 1 delegator * 1 ADA
    }

    #[tokio::test]
    async fn test_ledgerdb_certificate_processing() {
        let ledgerdb = MockLedgerDB::new();

        let stake_cred = StakeCredential::from_keyhash([1; 28]);

        // Process stake registration certificate
        let reg_cert = Certificate::StakeRegistration(stake_cred.clone());
        let result = ledgerdb.process_certificate(&reg_cert).await;
        assert!(result.is_ok());

        // Verify reward account created
        let reward_account = RewardAccount::from_stake_credential(stake_cred.clone());
        let balance = ledgerdb.get_reward_balance(&reward_account).await;
        assert_eq!(balance, Coin::zero());

        // Process stake deregistration certificate
        let dereg_cert = Certificate::StakeDeregistration(stake_cred.clone());
        let result = ledgerdb.process_certificate(&dereg_cert).await;
        assert!(result.is_ok());

        // Verify reward account removed
        let balance = ledgerdb.get_reward_balance(&reward_account).await;
        assert_eq!(balance, Coin::zero()); // Should return zero for non-existent account
    }

    #[tokio::test]
    async fn test_ledgerdb_concurrent_operations() {
        use std::sync::Arc;
        use tokio::task::JoinSet;

        let ledgerdb = Arc::new(MockLedgerDB::new());
        let mut join_set = JoinSet::new();

        // Spawn concurrent UTXO operations
        for i in 0..10 {
            let ledgerdb_clone = Arc::clone(&ledgerdb);

            join_set.spawn(async move {
                let address = Address::from_bech32("addr_test1").unwrap();
                let tx_hash = TxHash::from_hex(&format!("{:08x}", i)).unwrap();
                let tx_in = create_test_tx_in(tx_hash, 0);
                let tx_out = create_test_tx_out(address, Coin(1000000));

                // Add UTXO
                {
                    let mut utxo_set = ledgerdb_clone.utxo_set.write().await;
                    utxo_set.insert(tx_in.clone(), tx_out);
                }

                // Retrieve UTXO
                ledgerdb_clone.get_utxo(&tx_in).await.is_some()
            });
        }

        // Wait for all operations to complete
        let mut success_count = 0;
        while let Some(result) = join_set.join_next().await {
            if result.unwrap() {
                success_count += 1;
            }
        }

        assert_eq!(success_count, 10);

        // Verify final state
        let total_utxos = ledgerdb.get_utxo_set_size().await;
        assert_eq!(total_utxos, 10);
    }
}
