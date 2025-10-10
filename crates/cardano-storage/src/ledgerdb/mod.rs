//! Ledger database abstraction layer
//!
//! This module provides a high-level interface for storing and retrieving
//! ledger state including UTxO sets, stake pools, delegation certificates,
//! and protocol parameters.

use crate::backends::StorageBackend;
use crate::{Result, StorageError};
use async_trait::async_trait;
use cardano_crypto::Blake2b256Hash;
use cardano_ledger::{Coin, Epoch};

// Type aliases for ledger database
pub type TransactionHash = Blake2b256Hash;
pub type PoolId = Blake2b256Hash;
pub type EpochNo = Epoch;

// PoolId is already defined as Blake2b256Hash in cardano-crypto
// We'll use extension trait or helper functions instead of inherent impl

// Coin helper methods
pub trait CoinExt {
    fn zero() -> Self;
    fn new(amount: u64) -> Self;
}

impl CoinExt for Coin {
    fn zero() -> Self {
        0
    }

    fn new(amount: u64) -> Self {
        amount
    }
}

/// Transaction input identifier
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, minicbor::Encode, minicbor::Decode,
)]
pub struct TransactionInput {
    #[n(0)]
    pub transaction_id: TransactionHash,
    #[n(1)]
    pub index: u32,
}

/// Transaction output (simplified for storage)
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, minicbor::Encode, minicbor::Decode, Default,
)]
pub struct TransactionOutput {
    #[n(0)]
    pub amount: Coin,
    #[n(1)]
    pub address_data: Vec<u8>, // Serialized address
}

/// Stake credential (simplified for storage)
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    minicbor::Encode,
    minicbor::Decode,
    Default,
)]
pub struct StakeCredential {
    #[n(0)]
    pub credential_data: Vec<u8>, // Serialized credential
}

impl StakeCredential {
    pub fn as_bytes(&self) -> &[u8] {
        &self.credential_data
    }
}

/// Certificate (simplified for storage)
#[derive(Debug, Clone, Serialize, Deserialize, minicbor::Encode, minicbor::Decode)]
pub struct Certificate {
    #[n(0)]
    pub certificate_data: Vec<u8>, // Serialized certificate
}

/// Protocol Parameters (simplified for storage)
#[derive(Debug, Clone, Serialize, Deserialize, minicbor::Encode, minicbor::Decode)]
pub struct ProtocolParameters {
    #[n(0)]
    pub params_data: Vec<u8>, // Serialized parameters
}
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::convert::TryInto;
use std::sync::Arc;
use tracing::warn;

/// Ledger database interface for UTxO and stake state management
#[async_trait]
pub trait LedgerDatabase: Send + Sync {
    /// UTxO Management
    async fn store_utxo(&self, input: &TransactionInput, output: &TransactionOutput) -> Result<()>;
    async fn get_utxo(&self, input: &TransactionInput) -> Result<Option<TransactionOutput>>;
    async fn delete_utxo(&self, input: &TransactionInput) -> Result<()>;
    async fn has_utxo(&self, input: &TransactionInput) -> Result<bool>;

    /// Batch UTxO operations for transaction processing
    async fn apply_transaction_utxos(
        &self,
        consumed: &[TransactionInput],
        produced: &[(TransactionInput, TransactionOutput)],
    ) -> Result<()>;

    /// Export UTxO entries for snapshotting or analysis.
    ///
    /// Implementations may override this to provide an efficient streaming
    /// export. The default implementation returns an error indicating that the
    /// operation is unsupported.
    async fn export_utxos(
        &self,
        _limit: Option<usize>,
    ) -> Result<Vec<(TransactionInput, TransactionOutput)>> {
        Err(StorageError::DatabaseError(
            "export_utxos not implemented for this backend".to_string(),
        ))
    }

    /// Stake Pool Management
    async fn store_pool(&self, pool_id: &PoolId, pool_params: &PoolParameters) -> Result<()>;
    async fn get_pool(&self, pool_id: &PoolId) -> Result<Option<PoolParameters>>;
    async fn delete_pool(&self, pool_id: &PoolId) -> Result<()>;
    async fn list_active_pools(&self) -> Result<Vec<PoolId>>;

    /// Delegation Management
    async fn store_delegation(
        &self,
        stake_credential: &StakeCredential,
        pool_id: &PoolId,
    ) -> Result<()>;
    async fn get_delegation(&self, stake_credential: &StakeCredential) -> Result<Option<PoolId>>;
    async fn delete_delegation(&self, stake_credential: &StakeCredential) -> Result<()>;

    /// Stake and Rewards
    async fn store_stake(&self, stake_credential: &StakeCredential, stake: Coin) -> Result<()>;
    async fn get_stake(&self, stake_credential: &StakeCredential) -> Result<Option<Coin>>;
    async fn store_rewards(&self, stake_credential: &StakeCredential, rewards: Coin) -> Result<()>;
    async fn get_rewards(&self, stake_credential: &StakeCredential) -> Result<Option<Coin>>;

    /// Protocol Parameters
    async fn store_protocol_parameters(
        &self,
        epoch: EpochNo,
        params: &ProtocolParameters,
    ) -> Result<()>;
    async fn get_protocol_parameters(&self, epoch: EpochNo) -> Result<Option<ProtocolParameters>>;
    async fn get_current_protocol_parameters(&self) -> Result<Option<ProtocolParameters>>;

    /// Epoch Management
    async fn store_epoch_info(&self, epoch: EpochNo, info: &EpochInfo) -> Result<()>;
    async fn get_epoch_info(&self, epoch: EpochNo) -> Result<Option<EpochInfo>>;

    /// Ledger Statistics
    async fn get_ledger_stats(&self) -> Result<LedgerDatabaseStats>;

    /// Snapshots for rollback support
    async fn create_snapshot(&self, epoch: EpochNo) -> Result<()>;
    async fn rollback_to_snapshot(&self, epoch: EpochNo) -> Result<()>;
}

/// Pool parameters stored in the ledger
#[derive(Debug, Clone, Serialize, Deserialize, minicbor::Encode, minicbor::Decode)]
pub struct PoolParameters {
    #[n(0)]
    pub pool_id: PoolId,
    #[n(1)]
    pub pledge: Coin,
    #[n(2)]
    pub cost: Coin,
    #[n(3)]
    pub margin: f64,
    #[n(4)]
    pub reward_account: StakeCredential,
    #[n(5)]
    pub owners: BTreeSet<StakeCredential>,
    #[n(6)]
    pub relays: Vec<PoolRelay>,
    #[n(7)]
    pub metadata: Option<PoolMetadata>,
}

/// Pool relay information
#[derive(Debug, Clone, Serialize, Deserialize, minicbor::Encode, minicbor::Decode)]
pub struct PoolRelay {
    #[n(0)]
    pub dns_name: Option<String>,
    #[n(1)]
    pub ipv4: Option<[u8; 4]>,
    #[n(2)]
    pub ipv6: Option<[u8; 16]>,
    #[n(3)]
    pub port: Option<u16>,
}

/// Pool metadata
#[derive(Debug, Clone, Serialize, Deserialize, minicbor::Encode, minicbor::Decode)]
pub struct PoolMetadata {
    #[n(0)]
    pub url: String,
    #[n(1)]
    pub hash: [u8; 32],
}

/// Epoch information
#[derive(Debug, Clone, Serialize, Deserialize, minicbor::Encode, minicbor::Decode)]
pub struct EpochInfo {
    #[n(0)]
    pub epoch_no: EpochNo,
    #[n(1)]
    pub start_slot: u64,
    #[n(2)]
    pub end_slot: u64,
    #[n(3)]
    pub active_stake: Coin,
    #[n(4)]
    pub total_rewards: Coin,
    #[n(5)]
    pub fees_collected: Coin,
}

/// Statistics about the ledger database
#[derive(Debug, Clone)]
pub struct LedgerDatabaseStats {
    /// Total UTxOs in the set
    pub total_utxos: u64,
    /// Total value locked in UTxOs
    pub total_value: Coin,
    /// Number of active stake pools
    pub active_pools: u64,
    /// Number of delegations
    pub delegations: u64,
    /// Total active stake
    pub total_stake: Coin,
    /// Database size in bytes
    pub database_size: u64,
    /// Treasury balance calculated from outstanding rewards
    pub treasury: Coin,
    /// Remaining reserves based on max ADA supply
    pub reserves: Coin,
}

/// Implementation of LedgerDatabase using a storage backend
pub struct LedgerDatabaseImpl<B: StorageBackend> {
    backend: Arc<B>,
}

impl<B: StorageBackend> LedgerDatabaseImpl<B> {
    /// Create a new ledger database with the given storage backend
    pub fn new(backend: Arc<B>) -> Self {
        Self { backend }
    }

    /// Get the storage backend
    pub fn backend(&self) -> &Arc<B> {
        &self.backend
    }
}

// Key prefixes for different data types
const UTXO_PREFIX: &[u8] = b"utxo:";
const POOL_PREFIX: &[u8] = b"pool:";
const DELEGATION_PREFIX: &[u8] = b"delegation:";
const STAKE_PREFIX: &[u8] = b"stake:";
const REWARDS_PREFIX: &[u8] = b"rewards:";
const PROTOCOL_PARAMS_PREFIX: &[u8] = b"protocol_params:";
const EPOCH_INFO_PREFIX: &[u8] = b"epoch_info:";
const CURRENT_EPOCH_KEY: &[u8] = b"ledger:current_epoch";

fn utxo_key(input: &TransactionInput) -> Vec<u8> {
    let mut key = UTXO_PREFIX.to_vec();
    key.extend_from_slice(input.transaction_id.as_bytes());
    key.extend_from_slice(&input.index.to_be_bytes());
    key
}

fn parse_utxo_key(key: &[u8]) -> Result<TransactionInput> {
    if !key.starts_with(UTXO_PREFIX) {
        return Err(StorageError::SerializationError(
            "UTxO key missing expected prefix".to_string(),
        ));
    }

    let payload = &key[UTXO_PREFIX.len()..];
    if payload.len() != 32 + 4 {
        return Err(StorageError::SerializationError(format!(
            "Invalid UTxO key length: expected {} bytes, got {}",
            32 + 4,
            payload.len()
        )));
    }

    let (hash_bytes, index_bytes) = payload.split_at(32);
    let transaction_id = Blake2b256Hash::from_bytes(hash_bytes).map_err(|e| {
        StorageError::SerializationError(format!("Failed to decode UTxO hash: {}", e))
    })?;
    let index_bytes: [u8; 4] = index_bytes
        .try_into()
        .map_err(|_| StorageError::SerializationError("Invalid index bytes".to_string()))?;
    let index = u32::from_be_bytes(index_bytes);

    Ok(TransactionInput {
        transaction_id,
        index,
    })
}

fn pool_key(pool_id: &PoolId) -> Vec<u8> {
    let mut key = POOL_PREFIX.to_vec();
    key.extend_from_slice(pool_id.as_bytes());
    key
}

fn parse_pool_key(key: &[u8]) -> Result<PoolId> {
    if !key.starts_with(POOL_PREFIX) {
        return Err(StorageError::SerializationError(
            "Pool key missing expected prefix".to_string(),
        ));
    }

    let payload = &key[POOL_PREFIX.len()..];
    Blake2b256Hash::from_bytes(payload)
        .map_err(|e| StorageError::SerializationError(format!("Failed to decode pool ID: {}", e)))
}

fn delegation_key(stake_credential: &StakeCredential) -> Vec<u8> {
    let mut key = DELEGATION_PREFIX.to_vec();
    key.extend_from_slice(stake_credential.as_bytes());
    key
}

fn stake_key(stake_credential: &StakeCredential) -> Vec<u8> {
    let mut key = STAKE_PREFIX.to_vec();
    key.extend_from_slice(stake_credential.as_bytes());
    key
}

fn rewards_key(stake_credential: &StakeCredential) -> Vec<u8> {
    let mut key = REWARDS_PREFIX.to_vec();
    key.extend_from_slice(stake_credential.as_bytes());
    key
}

fn protocol_params_key(epoch: EpochNo) -> Vec<u8> {
    let mut key = PROTOCOL_PARAMS_PREFIX.to_vec();
    key.extend_from_slice(&epoch.to_be_bytes());
    key
}

fn epoch_info_key(epoch: EpochNo) -> Vec<u8> {
    let mut key = EPOCH_INFO_PREFIX.to_vec();
    key.extend_from_slice(&epoch.to_be_bytes());
    key
}

#[async_trait]
impl<B: StorageBackend> LedgerDatabase for LedgerDatabaseImpl<B> {
    async fn store_utxo(&self, input: &TransactionInput, output: &TransactionOutput) -> Result<()> {
        let key = utxo_key(input);
        let data = minicbor::to_vec(output).map_err(|e| {
            StorageError::SerializationError(format!("Failed to serialize UTxO: {}", e))
        })?;
        self.backend.put(&key, &data).await
    }

    async fn get_utxo(&self, input: &TransactionInput) -> Result<Option<TransactionOutput>> {
        let key = utxo_key(input);
        match self.backend.get(&key).await? {
            Some(data) => {
                let output = minicbor::decode(&data).map_err(|e| {
                    StorageError::SerializationError(format!("Failed to deserialize UTxO: {}", e))
                })?;
                Ok(Some(output))
            }
            None => Ok(None),
        }
    }

    async fn delete_utxo(&self, input: &TransactionInput) -> Result<()> {
        let key = utxo_key(input);
        self.backend.delete(&key).await
    }

    async fn has_utxo(&self, input: &TransactionInput) -> Result<bool> {
        let key = utxo_key(input);
        self.backend.exists(&key).await
    }

    async fn apply_transaction_utxos(
        &self,
        consumed: &[TransactionInput],
        produced: &[(TransactionInput, TransactionOutput)],
    ) -> Result<()> {
        use crate::backends::BatchOperation;

        let mut operations = Vec::new();

        // Delete consumed UTxOs
        for input in consumed {
            let key = utxo_key(input);
            operations.push(BatchOperation::Delete { key });
        }

        // Add produced UTxOs
        for (input, output) in produced {
            let key = utxo_key(input);
            let data = minicbor::to_vec(output).map_err(|e| {
                StorageError::SerializationError(format!("Failed to serialize UTxO: {}", e))
            })?;
            operations.push(BatchOperation::Put { key, value: data });
        }

        self.backend.batch(operations).await
    }

    async fn export_utxos(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<(TransactionInput, TransactionOutput)>> {
        let entries = self.backend.scan_prefix(UTXO_PREFIX, limit).await?;
        let mut utxos = Vec::with_capacity(entries.len());

        for (key, value) in entries {
            match parse_utxo_key(&key) {
                Ok(input) => match minicbor::decode::<TransactionOutput>(&value) {
                    Ok(output) => utxos.push((input, output)),
                    Err(e) => warn!("Failed to decode UTxO value: {}", e),
                },
                Err(e) => warn!("Failed to decode UTxO key: {}", e),
            }
        }

        Ok(utxos)
    }

    async fn store_pool(&self, pool_id: &PoolId, pool_params: &PoolParameters) -> Result<()> {
        let key = pool_key(pool_id);
        let data = minicbor::to_vec(pool_params).map_err(|e| {
            StorageError::SerializationError(format!("Failed to serialize pool parameters: {}", e))
        })?;
        self.backend.put(&key, &data).await
    }

    async fn get_pool(&self, pool_id: &PoolId) -> Result<Option<PoolParameters>> {
        let key = pool_key(pool_id);
        match self.backend.get(&key).await? {
            Some(data) => {
                let pool_params = minicbor::decode(&data).map_err(|e| {
                    StorageError::SerializationError(format!(
                        "Failed to deserialize pool parameters: {}",
                        e
                    ))
                })?;
                Ok(Some(pool_params))
            }
            None => Ok(None),
        }
    }

    async fn delete_pool(&self, pool_id: &PoolId) -> Result<()> {
        let key = pool_key(pool_id);
        self.backend.delete(&key).await
    }

    async fn list_active_pools(&self) -> Result<Vec<PoolId>> {
        let entries = self.backend.scan_prefix(POOL_PREFIX, None).await?;
        let mut pools = Vec::with_capacity(entries.len());

        for (key, _value) in entries {
            match parse_pool_key(&key) {
                Ok(pool_id) => pools.push(pool_id),
                Err(e) => warn!("Failed to decode pool key: {}", e),
            }
        }

        Ok(pools)
    }

    async fn store_delegation(
        &self,
        stake_credential: &StakeCredential,
        pool_id: &PoolId,
    ) -> Result<()> {
        let key = delegation_key(stake_credential);
        let data = minicbor::to_vec(pool_id.as_ref()).map_err(|e| {
            StorageError::SerializationError(format!("Failed to serialize pool ID: {}", e))
        })?;
        self.backend.put(&key, &data).await
    }

    async fn get_delegation(&self, stake_credential: &StakeCredential) -> Result<Option<PoolId>> {
        let key = delegation_key(stake_credential);
        match self.backend.get(&key).await? {
            Some(data) => {
                let pool_id_bytes: Vec<u8> = minicbor::decode(&data)
                    .map_err(|e| StorageError::SerializationError(e.to_string()))?;
                let pool_id = Blake2b256Hash::from_bytes(&pool_id_bytes).map_err(|e| {
                    StorageError::SerializationError(format!(
                        "Failed to deserialize pool ID: {}",
                        e
                    ))
                })?;
                Ok(Some(pool_id))
            }
            None => Ok(None),
        }
    }

    async fn delete_delegation(&self, stake_credential: &StakeCredential) -> Result<()> {
        let key = delegation_key(stake_credential);
        self.backend.delete(&key).await
    }

    async fn store_stake(&self, stake_credential: &StakeCredential, stake: Coin) -> Result<()> {
        let key = stake_key(stake_credential);
        let data = minicbor::to_vec(stake).map_err(|e| {
            StorageError::SerializationError(format!("Failed to serialize stake: {}", e))
        })?;
        self.backend.put(&key, &data).await
    }

    async fn get_stake(&self, stake_credential: &StakeCredential) -> Result<Option<Coin>> {
        let key = stake_key(stake_credential);
        match self.backend.get(&key).await? {
            Some(data) => {
                let stake = minicbor::decode(&data).map_err(|e| {
                    StorageError::SerializationError(format!("Failed to deserialize stake: {}", e))
                })?;
                Ok(Some(stake))
            }
            None => Ok(None),
        }
    }

    async fn store_rewards(&self, stake_credential: &StakeCredential, rewards: Coin) -> Result<()> {
        let key = rewards_key(stake_credential);
        let data = minicbor::to_vec(rewards).map_err(|e| {
            StorageError::SerializationError(format!("Failed to serialize rewards: {}", e))
        })?;
        self.backend.put(&key, &data).await
    }

    async fn get_rewards(&self, stake_credential: &StakeCredential) -> Result<Option<Coin>> {
        let key = rewards_key(stake_credential);
        match self.backend.get(&key).await? {
            Some(data) => {
                let rewards = minicbor::decode(&data).map_err(|e| {
                    StorageError::SerializationError(format!(
                        "Failed to deserialize rewards: {}",
                        e
                    ))
                })?;
                Ok(Some(rewards))
            }
            None => Ok(None),
        }
    }

    async fn store_protocol_parameters(
        &self,
        epoch: EpochNo,
        params: &ProtocolParameters,
    ) -> Result<()> {
        let key = protocol_params_key(epoch);
        let data = minicbor::to_vec(params).map_err(|e| {
            StorageError::SerializationError(format!(
                "Failed to serialize protocol parameters: {}",
                e
            ))
        })?;
        self.backend.put(&key, &data).await
    }

    async fn get_protocol_parameters(&self, epoch: EpochNo) -> Result<Option<ProtocolParameters>> {
        let key = protocol_params_key(epoch);
        match self.backend.get(&key).await? {
            Some(data) => {
                let params = minicbor::decode(&data).map_err(|e| {
                    StorageError::SerializationError(format!(
                        "Failed to deserialize protocol parameters: {}",
                        e
                    ))
                })?;
                Ok(Some(params))
            }
            None => Ok(None),
        }
    }

    async fn get_current_protocol_parameters(&self) -> Result<Option<ProtocolParameters>> {
        // Get current epoch and return its parameters
        match self.backend.get(CURRENT_EPOCH_KEY).await? {
            Some(epoch_data) => {
                let epoch: EpochNo = minicbor::decode(&epoch_data).map_err(|e| {
                    StorageError::SerializationError(format!(
                        "Failed to deserialize current epoch: {}",
                        e
                    ))
                })?;
                self.get_protocol_parameters(epoch).await
            }
            None => Ok(None),
        }
    }

    async fn store_epoch_info(&self, epoch: EpochNo, info: &EpochInfo) -> Result<()> {
        let key = epoch_info_key(epoch);
        let data = minicbor::to_vec(info).map_err(|e| {
            StorageError::SerializationError(format!("Failed to serialize epoch info: {}", e))
        })?;
        self.backend.put(&key, &data).await
    }

    async fn get_epoch_info(&self, epoch: EpochNo) -> Result<Option<EpochInfo>> {
        let key = epoch_info_key(epoch);
        match self.backend.get(&key).await? {
            Some(data) => {
                let info = minicbor::decode(&data).map_err(|e| {
                    StorageError::SerializationError(format!(
                        "Failed to deserialize epoch info: {}",
                        e
                    ))
                })?;
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }

    async fn get_ledger_stats(&self) -> Result<LedgerDatabaseStats> {
        const MAX_ADA_SUPPLY: u128 = 45_000_000_000_000_000u128;

        let utxos = self.export_utxos(None).await?;
        let total_utxos = utxos.len() as u64;
        let total_value: u128 = utxos.iter().fold(0u128, |acc, (_, output)| {
            acc.saturating_add(output.amount as u128)
        });

        let active_pools = self.list_active_pools().await?.len() as u64;
        let delegations = self
            .backend
            .scan_prefix(DELEGATION_PREFIX, None)
            .await?
            .len() as u64;

        let stake_entries = self.backend.scan_prefix(STAKE_PREFIX, None).await?;
        let total_stake: u128 = stake_entries.into_iter().fold(0u128, |acc, (_, value)| {
            match minicbor::decode::<Coin>(&value) {
                Ok(stake) => acc.saturating_add(stake as u128),
                Err(e) => {
                    warn!("Failed to decode stake entry: {}", e);
                    acc
                }
            }
        });

        let reward_entries = self.backend.scan_prefix(REWARDS_PREFIX, None).await?;
        let treasury_value: u128 = reward_entries.into_iter().fold(0u128, |acc, (_, value)| {
            match minicbor::decode::<Coin>(&value) {
                Ok(reward) => acc.saturating_add(reward as u128),
                Err(e) => {
                    warn!("Failed to decode rewards entry: {}", e);
                    acc
                }
            }
        });

        let reserves_value = MAX_ADA_SUPPLY
            .saturating_sub(total_value)
            .saturating_sub(treasury_value);

        let backend_stats = self.backend.stats().await?;

        Ok(LedgerDatabaseStats {
            total_utxos,
            total_value: total_value.min(u64::MAX as u128) as u64,
            active_pools,
            delegations,
            total_stake: total_stake.min(u64::MAX as u128) as u64,
            database_size: backend_stats.total_size,
            treasury: treasury_value.min(u64::MAX as u128) as u64,
            reserves: reserves_value.min(u64::MAX as u128) as u64,
        })
    }

    async fn create_snapshot(&self, _epoch: EpochNo) -> Result<()> {
        // TODO: Implement snapshotting mechanism
        // This would likely involve backend-specific operations
        Ok(())
    }

    async fn rollback_to_snapshot(&self, _epoch: EpochNo) -> Result<()> {
        // TODO: Implement rollback mechanism
        // This would restore state from a previous snapshot
        Ok(())
    }
}

#[cfg(all(test, feature = "legacy"))]
mod tests {
    use super::*;
    use crate::backends::{LmdbBackend, LmdbConfig};
    use std::sync::atomic::{AtomicU64, Ordering};
    use tempfile::TempDir;

    static UTXO_COUNTER: AtomicU64 = AtomicU64::new(0);

    async fn create_test_ledgerdb() -> (LedgerDatabaseImpl<LmdbBackend>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = LmdbConfig::with_path(temp_dir.path()).unwrap();
        let backend = Arc::new(LmdbBackend::new(config).unwrap());
        backend.init().await.unwrap();
        let ledgerdb = LedgerDatabaseImpl::new(backend);
        (ledgerdb, temp_dir)
    }

    fn create_test_utxo() -> (TransactionInput, TransactionOutput) {
        let count = UTXO_COUNTER.fetch_add(1, Ordering::SeqCst);
        let transaction_id = TransactionHash::hash(&count.to_be_bytes());
        let input = TransactionInput {
            transaction_id,
            index: (count % u32::MAX as u64) as u32,
        };
        let output = TransactionOutput::default();
        (input, output)
    }

    #[tokio::test]
    async fn test_utxo_operations() {
        let (ledgerdb, _temp_dir) = create_test_ledgerdb().await;

        let (input, output) = create_test_utxo();

        // Store UTxO
        ledgerdb.store_utxo(&input, &output).await.unwrap();

        // Check if UTxO exists
        assert!(ledgerdb.has_utxo(&input).await.unwrap());

        // Retrieve UTxO
        let retrieved_output = ledgerdb.get_utxo(&input).await.unwrap().unwrap();
        assert_eq!(retrieved_output, output);

        // Delete UTxO
        ledgerdb.delete_utxo(&input).await.unwrap();
        assert!(!ledgerdb.has_utxo(&input).await.unwrap());
    }

    #[tokio::test]
    async fn test_batch_utxo_operations() {
        let (ledgerdb, _temp_dir) = create_test_ledgerdb().await;

        let (input1, output1) = create_test_utxo();
        let (input2, output2) = create_test_utxo();

        // Store initial UTxOs
        ledgerdb.store_utxo(&input1, &output1).await.unwrap();

        // Apply transaction: consume input1, produce input2
        ledgerdb
            .apply_transaction_utxos(
                std::slice::from_ref(&input1),
                &[(input2.clone(), output2.clone())],
            )
            .await
            .unwrap();

        // Verify state
        assert!(!ledgerdb.has_utxo(&input1).await.unwrap());
        assert!(ledgerdb.has_utxo(&input2).await.unwrap());

        let retrieved_output2 = ledgerdb.get_utxo(&input2).await.unwrap().unwrap();
        assert_eq!(retrieved_output2, output2);
    }

    #[tokio::test]
    async fn test_stake_pool_operations() {
        let (ledgerdb, _temp_dir) = create_test_ledgerdb().await;

        let pool_id = PoolId::default();
        let pool_params = PoolParameters {
            pool_id,
            pledge: CoinExt::new(1000000),
            cost: CoinExt::new(340000000),
            margin: 0.05,
            reward_account: StakeCredential::default(),
            owners: BTreeSet::new(),
            relays: Vec::new(),
            metadata: None,
        };

        // Store pool
        ledgerdb.store_pool(&pool_id, &pool_params).await.unwrap();

        // Retrieve pool
        let retrieved_params = ledgerdb.get_pool(&pool_id).await.unwrap().unwrap();
        assert_eq!(retrieved_params.pledge, pool_params.pledge);
        assert_eq!(retrieved_params.cost, pool_params.cost);
        assert_eq!(retrieved_params.margin, pool_params.margin);

        // Delete pool
        ledgerdb.delete_pool(&pool_id).await.unwrap();
        assert!(ledgerdb.get_pool(&pool_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delegation_operations() {
        let (ledgerdb, _temp_dir) = create_test_ledgerdb().await;

        let stake_credential = StakeCredential::default();
        let pool_id = PoolId::default();

        // Store delegation
        ledgerdb
            .store_delegation(&stake_credential, &pool_id)
            .await
            .unwrap();

        // Retrieve delegation
        let retrieved_pool_id = ledgerdb
            .get_delegation(&stake_credential)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retrieved_pool_id, pool_id);

        // Delete delegation
        ledgerdb.delete_delegation(&stake_credential).await.unwrap();
        assert!(ledgerdb
            .get_delegation(&stake_credential)
            .await
            .unwrap()
            .is_none());
    }
}
