//! Chain database abstraction layer
//!
//! This module provides a high-level interface for storing and retrieving
//! blockchain data including blocks, transactions, and chain metadata.

use crate::backends::StorageBackend;
use crate::{Result, StorageError};
use async_trait::async_trait;
use cardano_crypto::Blake2b256Hash;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Chain database interface for blockchain data storage
#[async_trait]
pub trait ChainDatabase: Send + Sync {
    /// Store a block and its transactions (generic data)
    async fn store_block(
        &self,
        block_hash: &Blake2b256Hash,
        height: u64,
        block_data: &[u8],
        tx_data: &[(Blake2b256Hash, Vec<u8>)],
    ) -> Result<()>;

    /// Retrieve a block by its hash
    async fn get_block(&self, block_hash: &Blake2b256Hash) -> Result<Option<Vec<u8>>>;

    /// Retrieve a transaction by its hash
    async fn get_transaction(&self, tx_hash: &Blake2b256Hash) -> Result<Option<Vec<u8>>>;

    /// Store chain metadata (tip, genesis, epoch info)
    async fn store_chain_metadata(&self, metadata: &ChainMetadata) -> Result<()>;

    /// Get current chain metadata
    async fn get_chain_metadata(&self) -> Result<Option<ChainMetadata>>;

    /// Get blocks in a range (for chain sync)
    async fn get_blocks_range(
        &self,
        start_hash: &Blake2b256Hash,
        count: u32,
    ) -> Result<Vec<Vec<u8>>>;

    /// Check if a block exists
    async fn has_block(&self, block_hash: &Blake2b256Hash) -> Result<bool>;

    /// Check if a transaction exists
    async fn has_transaction(&self, tx_hash: &Blake2b256Hash) -> Result<bool>;

    /// Store block-to-transaction mapping
    async fn store_block_transactions(
        &self,
        block_hash: &Blake2b256Hash,
        tx_hashes: &[Blake2b256Hash],
    ) -> Result<()>;

    /// Get all transaction hashes for a block
    async fn get_block_transactions(
        &self,
        block_hash: &Blake2b256Hash,
    ) -> Result<Vec<Blake2b256Hash>>;

    /// Get database statistics
    async fn get_chain_stats(&self) -> Result<ChainDatabaseStats>;

    /// Force recomputation of chain statistics from storage
    async fn recalculate_chain_stats(&self) -> Result<ChainDatabaseStats>;
}

/// Chain metadata stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, minicbor::Encode, minicbor::Decode)]
pub struct ChainMetadata {
    /// Current tip of the chain
    #[n(0)]
    pub tip_hash: Blake2b256Hash,
    /// Height of the tip block
    #[n(1)]
    pub tip_height: u64,
    /// Genesis block hash
    #[n(2)]
    pub genesis_hash: Blake2b256Hash,
    /// Current epoch number
    #[n(3)]
    pub current_epoch: u64,
    /// Current slot number
    #[n(4)]
    pub current_slot: u64,
    /// Network magic number
    #[n(5)]
    pub network_magic: u32,
}

/// Statistics about the chain database
#[derive(Debug, Clone)]
pub struct ChainDatabaseStats {
    /// Total number of blocks stored
    pub total_blocks: u64,
    /// Total number of transactions stored
    pub total_transactions: u64,
    /// Current chain height
    pub chain_height: u64,
    /// Database size in bytes
    pub database_size: u64,
}

/// Implementation of ChainDatabase using a storage backend
pub struct ChainDatabaseImpl<B: StorageBackend> {
    backend: Arc<B>,
}

impl<B: StorageBackend> ChainDatabaseImpl<B> {
    /// Create a new chain database with the given storage backend
    pub fn new(backend: Arc<B>) -> Self {
        Self { backend }
    }

    /// Get the storage backend
    pub fn backend(&self) -> &Arc<B> {
        &self.backend
    }

    async fn load_persisted_stats(&self) -> Result<Option<PersistedChainStats>> {
        match self.backend.get(CHAIN_STATS_KEY).await? {
            Some(data) => {
                let stats = minicbor::decode(&data).map_err(|e| {
                    StorageError::SerializationError(format!(
                        "Failed to deserialize chain stats: {}",
                        e
                    ))
                })?;
                Ok(Some(stats))
            }
            None => Ok(None),
        }
    }

    async fn save_persisted_stats(&self, stats: &PersistedChainStats) -> Result<()> {
        let data = minicbor::to_vec(stats).map_err(|e| {
            StorageError::SerializationError(format!("Failed to serialize chain stats: {}", e))
        })?;
        self.backend.put(CHAIN_STATS_KEY, &data).await
    }

    async fn combine_stats_with_backend(
        &self,
        persisted: PersistedChainStats,
    ) -> Result<ChainDatabaseStats> {
        let backend_stats = self.backend.stats().await?;

        Ok(ChainDatabaseStats {
            total_blocks: persisted.total_blocks,
            total_transactions: persisted.total_transactions,
            chain_height: persisted.chain_height,
            database_size: backend_stats.total_size,
        })
    }

    async fn compute_stats_from_storage(&self) -> Result<PersistedChainStats> {
        let block_index_entries = self.backend.scan_prefix(BLOCK_INDEX_PREFIX, None).await?;
        let mut stats = PersistedChainStats::default();
        stats.total_blocks = block_index_entries.len() as u64;
        for (_, value) in block_index_entries {
            let bytes: [u8; 8] = value.as_slice().try_into().map_err(|_| {
                StorageError::SerializationError("Invalid block height entry in index".to_string())
            })?;
            let height = u64::from_be_bytes(bytes);
            stats.chain_height = stats.chain_height.max(height);
        }

        let tx_entries = self.backend.scan_prefix(TX_PREFIX, None).await?;
        stats.total_transactions = tx_entries.len() as u64;

        Ok(stats)
    }

    async fn update_chain_stats(
        &self,
        height: u64,
        is_new_block: bool,
        new_tx_count: u64,
    ) -> Result<()> {
        let mut stats = self.load_persisted_stats().await?.unwrap_or_default();

        if is_new_block {
            stats.total_blocks = stats.total_blocks.saturating_add(1);
        }

        if new_tx_count > 0 {
            stats.total_transactions = stats.total_transactions.saturating_add(new_tx_count);
        }

        stats.chain_height = stats.chain_height.max(height);

        self.save_persisted_stats(&stats).await
    }
}

// Key prefixes for different data types
const BLOCK_PREFIX: &[u8] = b"block:";
const TX_PREFIX: &[u8] = b"tx:";
const METADATA_KEY: &[u8] = b"chain:metadata";
const BLOCK_TXS_PREFIX: &[u8] = b"block_txs:";
const BLOCK_HEIGHT_PREFIX: &[u8] = b"block_height:";
const BLOCK_INDEX_PREFIX: &[u8] = b"block_index:";
const CHAIN_STATS_KEY: &[u8] = b"chain:stats";

#[derive(Debug, Clone, Default, Serialize, Deserialize, minicbor::Encode, minicbor::Decode)]
struct PersistedChainStats {
    #[n(0)]
    total_blocks: u64,
    #[n(1)]
    total_transactions: u64,
    #[n(2)]
    chain_height: u64,
}

fn block_height_prefix(height: u64) -> Vec<u8> {
    let mut prefix = BLOCK_HEIGHT_PREFIX.to_vec();
    prefix.extend_from_slice(&height.to_be_bytes());
    prefix
}

fn block_height_key(block_hash: &Blake2b256Hash, height: u64) -> Vec<u8> {
    let mut key = block_height_prefix(height);
    key.extend_from_slice(block_hash.as_ref());
    key
}

fn block_index_key(block_hash: &Blake2b256Hash) -> Vec<u8> {
    let mut key = BLOCK_INDEX_PREFIX.to_vec();
    key.extend_from_slice(block_hash.as_ref());
    key
}

fn block_key(block_hash: &Blake2b256Hash) -> Vec<u8> {
    let mut key = BLOCK_PREFIX.to_vec();
    key.extend_from_slice(block_hash.as_ref());
    key
}

fn tx_key(tx_hash: &Blake2b256Hash) -> Vec<u8> {
    let mut key = TX_PREFIX.to_vec();
    key.extend_from_slice(tx_hash.as_ref());
    key
}

fn block_txs_key(block_hash: &Blake2b256Hash) -> Vec<u8> {
    let mut key = BLOCK_TXS_PREFIX.to_vec();
    key.extend_from_slice(block_hash.as_ref());
    key
}

#[async_trait]
impl<B: StorageBackend> ChainDatabase for ChainDatabaseImpl<B> {
    async fn store_block(
        &self,
        block_hash: &Blake2b256Hash,
        height: u64,
        block_data: &[u8],
        tx_data: &[(Blake2b256Hash, Vec<u8>)],
    ) -> Result<()> {
        let block_key_vec = block_key(block_hash);
        let is_new_block = !self.backend.exists(&block_key_vec).await?;

        // Store block content
        self.backend.put(&block_key_vec, block_data).await?;

        // Store transactions and collect hashes
        let mut tx_hashes = Vec::new();
        let mut new_tx_count = 0u64;
        for (tx_hash, data) in tx_data {
            let tx_key_vec = tx_key(tx_hash);
            let is_new_tx = !self.backend.exists(&tx_key_vec).await?;
            self.backend.put(&tx_key_vec, data).await?;
            if is_new_tx {
                new_tx_count = new_tx_count.saturating_add(1);
            }
            tx_hashes.push(*tx_hash);
        }

        // Store block-to-transactions mapping
        if !tx_hashes.is_empty() {
            self.store_block_transactions(block_hash, &tx_hashes)
                .await?;
        }

        // Index block by height for range scans
        let height_bytes = height.to_be_bytes();
        self.backend
            .put(&block_index_key(block_hash), &height_bytes)
            .await?;
        self.backend
            .put(&block_height_key(block_hash, height), block_hash.as_ref())
            .await?;

        self.update_chain_stats(height, is_new_block, new_tx_count)
            .await?;

        Ok(())
    }

    async fn get_block(&self, block_hash: &Blake2b256Hash) -> Result<Option<Vec<u8>>> {
        let key = block_key(block_hash);
        self.backend.get(&key).await
    }

    async fn get_transaction(&self, tx_hash: &Blake2b256Hash) -> Result<Option<Vec<u8>>> {
        let key = tx_key(tx_hash);
        self.backend.get(&key).await
    }

    async fn store_chain_metadata(&self, metadata: &ChainMetadata) -> Result<()> {
        let data = minicbor::to_vec(metadata).map_err(|e| {
            StorageError::SerializationError(format!("Failed to serialize metadata: {}", e))
        })?;
        self.backend.put(METADATA_KEY, &data).await
    }

    async fn get_chain_metadata(&self) -> Result<Option<ChainMetadata>> {
        match self.backend.get(METADATA_KEY).await? {
            Some(data) => {
                let metadata = minicbor::decode(&data).map_err(|e| {
                    StorageError::SerializationError(format!(
                        "Failed to deserialize metadata: {}",
                        e
                    ))
                })?;
                Ok(Some(metadata))
            }
            None => Ok(None),
        }
    }

    async fn get_blocks_range(
        &self,
        start_hash: &Blake2b256Hash,
        count: u32,
    ) -> Result<Vec<Vec<u8>>> {
        if count == 0 {
            return Ok(Vec::new());
        }

        let index_key = block_index_key(start_hash);
        let Some(height_bytes) = self.backend.get(&index_key).await? else {
            return Err(StorageError::DatabaseError(format!(
                "Block hash {} not indexed",
                hex::encode(start_hash.as_ref())
            )));
        };

        let height_array: [u8; 8] = height_bytes
            .as_slice()
            .try_into()
            .map_err(|_| StorageError::SerializationError("Invalid height entry".to_string()))?;
        let mut current_height = u64::from_be_bytes(height_array);
        let mut current_hash = *start_hash;
        let mut blocks = Vec::new();

        for _ in 0..count {
            match self.get_block(&current_hash).await? {
                Some(block) => blocks.push(block),
                None => break,
            }

            if blocks.len() == count as usize {
                break;
            }

            let Some(next_height) = current_height.checked_add(1) else {
                break;
            };
            let height_prefix = block_height_prefix(next_height);
            let entries = self.backend.scan_prefix(&height_prefix, None).await?;

            let mut next_hash = None;
            for (_, value) in entries {
                if value.len() != 32 {
                    continue;
                }
                match Blake2b256Hash::from_bytes(&value) {
                    Ok(candidate) => {
                        next_hash = Some(candidate);
                        break;
                    }
                    Err(_) => continue,
                }
            }

            let Some(next_hash_value) = next_hash else {
                break;
            };

            current_hash = next_hash_value;
            current_height = next_height;
        }

        Ok(blocks)
    }

    async fn has_block(&self, block_hash: &Blake2b256Hash) -> Result<bool> {
        let key = block_key(block_hash);
        self.backend.exists(&key).await
    }

    async fn has_transaction(&self, tx_hash: &Blake2b256Hash) -> Result<bool> {
        let key = tx_key(tx_hash);
        self.backend.exists(&key).await
    }

    async fn store_block_transactions(
        &self,
        block_hash: &Blake2b256Hash,
        tx_hashes: &[Blake2b256Hash],
    ) -> Result<()> {
        let data = minicbor::to_vec(tx_hashes).map_err(|e| {
            StorageError::SerializationError(format!(
                "Failed to serialize transaction hashes: {}",
                e
            ))
        })?;
        let key = block_txs_key(block_hash);
        self.backend.put(&key, &data).await
    }

    async fn get_block_transactions(
        &self,
        block_hash: &Blake2b256Hash,
    ) -> Result<Vec<Blake2b256Hash>> {
        let key = block_txs_key(block_hash);
        match self.backend.get(&key).await? {
            Some(data) => {
                let tx_hashes = minicbor::decode(&data).map_err(|e| {
                    StorageError::SerializationError(format!(
                        "Failed to deserialize transaction hashes: {}",
                        e
                    ))
                })?;
                Ok(tx_hashes)
            }
            None => Ok(Vec::new()),
        }
    }

    async fn get_chain_stats(&self) -> Result<ChainDatabaseStats> {
        if let Some(persisted) = self.load_persisted_stats().await? {
            self.combine_stats_with_backend(persisted).await
        } else {
            self.recalculate_chain_stats().await
        }
    }

    async fn recalculate_chain_stats(&self) -> Result<ChainDatabaseStats> {
        let computed = self.compute_stats_from_storage().await?;
        self.save_persisted_stats(&computed).await?;
        self.combine_stats_with_backend(computed).await
    }
}

#[cfg(all(test, feature = "legacy"))]
mod tests {
    use super::*;
    use crate::backends::{LmdbBackend, LmdbConfig};
    use tempfile::TempDir;
    async fn create_test_chaindb() -> (ChainDatabaseImpl<LmdbBackend>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = LmdbConfig::with_path(temp_dir.path()).unwrap();
        let backend = Arc::new(LmdbBackend::new(config).unwrap());
        backend.init().await.unwrap();
        let chaindb = ChainDatabaseImpl::new(backend);
        (chaindb, temp_dir)
    }

    #[tokio::test]
    async fn test_store_and_retrieve_block() {
        let (chaindb, _temp_dir) = create_test_chaindb().await;

        let block_hash = Blake2b256Hash::default();
        let block_data = b"test_block_data".to_vec();
        let tx_hash = Blake2b256Hash::default();
        let tx_data = b"test_tx_data".to_vec();

        // Store block
        chaindb
            .store_block(&block_hash, 0, &block_data, &[(tx_hash, tx_data.clone())])
            .await
            .unwrap();

        // Retrieve block
        let retrieved_block = chaindb.get_block(&block_hash).await.unwrap().unwrap();
        assert_eq!(retrieved_block, block_data);

        // Check if block exists
        assert!(chaindb.has_block(&block_hash).await.unwrap());
    }

    #[tokio::test]
    async fn test_store_and_retrieve_transaction() {
        let (chaindb, _temp_dir) = create_test_chaindb().await;

        let block_hash = Blake2b256Hash::default();
        let block_data = b"test_block_data".to_vec();
        let tx_hash = Blake2b256Hash::default();
        let tx_data = b"test_tx_data".to_vec();

        // Store block with transaction
        chaindb
            .store_block(&block_hash, 0, &block_data, &[(tx_hash, tx_data.clone())])
            .await
            .unwrap();

        // Retrieve transaction
        let retrieved_tx = chaindb.get_transaction(&tx_hash).await.unwrap().unwrap();
        assert_eq!(retrieved_tx, tx_data);

        // Check if transaction exists
        assert!(chaindb.has_transaction(&tx_hash).await.unwrap());
    }

    #[tokio::test]
    async fn test_chain_metadata() {
        let (chaindb, _temp_dir) = create_test_chaindb().await;

        let metadata = ChainMetadata {
            tip_hash: Blake2b256Hash::default(),
            tip_height: 1000,
            genesis_hash: Blake2b256Hash::default(),
            current_epoch: 50,
            current_slot: 2000000,
            network_magic: 764824073, // Mainnet magic
        };

        // Store metadata
        chaindb.store_chain_metadata(&metadata).await.unwrap();

        // Retrieve metadata
        let retrieved_metadata = chaindb.get_chain_metadata().await.unwrap().unwrap();
        assert_eq!(retrieved_metadata.tip_height, 1000);
        assert_eq!(retrieved_metadata.current_epoch, 50);
        assert_eq!(retrieved_metadata.network_magic, 764824073);
    }

    #[tokio::test]
    async fn test_block_transactions_mapping() {
        let (chaindb, _temp_dir) = create_test_chaindb().await;

        let block_hash = Blake2b256Hash::default();
        let block_data = b"test_block_data".to_vec();
        let tx1_hash = Blake2b256Hash::default();
        let tx1_data = b"test_tx1_data".to_vec();
        let tx2_hash = Blake2b256Hash::default();
        let tx2_data = b"test_tx2_data".to_vec();

        // Store block with transactions
        chaindb
            .store_block(
                &block_hash,
                0,
                &block_data,
                &[(tx1_hash, tx1_data), (tx2_hash, tx2_data)],
            )
            .await
            .unwrap();

        // Retrieve transaction hashes for the block
        let tx_hashes = chaindb.get_block_transactions(&block_hash).await.unwrap();
        assert_eq!(tx_hashes.len(), 2);

        // Verify we can retrieve each transaction
        for tx_hash in &tx_hashes {
            assert!(chaindb.has_transaction(tx_hash).await.unwrap());
        }
    }
}

#[cfg(test)]
mod memory_backend_tests {
    use super::*;
    use crate::backends::MemoryBackend;

    fn make_hash(seed: &[u8]) -> Blake2b256Hash {
        Blake2b256Hash::hash(seed)
    }

    #[tokio::test]
    async fn test_get_blocks_range_memory_backend() {
        let backend = Arc::new(MemoryBackend::new());
        let chaindb = ChainDatabaseImpl::new(backend);

        let hash0 = make_hash(b"block0");
        let hash1 = make_hash(b"block1");
        let hash2 = make_hash(b"block2");

        let empty_txs: Vec<(Blake2b256Hash, Vec<u8>)> = Vec::new();
        chaindb
            .store_block(&hash0, 0, b"block0".as_ref(), &empty_txs)
            .await
            .unwrap();
        chaindb
            .store_block(&hash1, 1, b"block1".as_ref(), &empty_txs)
            .await
            .unwrap();
        chaindb
            .store_block(&hash2, 2, b"block2".as_ref(), &empty_txs)
            .await
            .unwrap();

        let range = chaindb.get_blocks_range(&hash1, 2).await.unwrap();
        assert_eq!(range.len(), 2);
        assert_eq!(range[0], b"block1".to_vec());
        assert_eq!(range[1], b"block2".to_vec());
    }

    #[tokio::test]
    async fn test_get_blocks_range_missing_start() {
        let backend = Arc::new(MemoryBackend::new());
        let chaindb = ChainDatabaseImpl::new(backend);

        let result = chaindb
            .get_blocks_range(&Blake2b256Hash::hash(b"missing"), 1)
            .await;

        assert!(matches!(result, Err(StorageError::DatabaseError(_))));
    }

    #[tokio::test]
    async fn test_get_chain_stats_memory_backend() {
        let backend = Arc::new(MemoryBackend::new());
        let chaindb = ChainDatabaseImpl::new(backend);

        let hash0 = make_hash(b"block0-stats");
        let hash1 = make_hash(b"block1-stats");
        let hash2 = make_hash(b"block2-stats");

        let tx0 = make_hash(b"tx0");
        let tx1 = make_hash(b"tx1");
        let tx2 = make_hash(b"tx2");

        let block0_txs = vec![(tx0, b"tx0".to_vec())];
        chaindb
            .store_block(&hash0, 0, b"block0".as_ref(), &block0_txs)
            .await
            .unwrap();

        let block1_txs = vec![(tx1, b"tx1".to_vec()), (tx2, b"tx2".to_vec())];
        chaindb
            .store_block(&hash1, 1, b"block1".as_ref(), &block1_txs)
            .await
            .unwrap();

        let empty_txs: Vec<(Blake2b256Hash, Vec<u8>)> = Vec::new();
        chaindb
            .store_block(&hash2, 2, b"block2".as_ref(), &empty_txs)
            .await
            .unwrap();

        let stats = chaindb.get_chain_stats().await.unwrap();

        assert_eq!(stats.total_blocks, 3);
        assert_eq!(stats.chain_height, 2);
        assert_eq!(stats.total_transactions, 3);
        assert!(stats.database_size > 0);

        let tx3 = make_hash(b"tx3");
        let mut block1_updated = block1_txs.clone();
        block1_updated.push((tx3, b"tx3".to_vec()));
        chaindb
            .store_block(&hash1, 1, b"block1".as_ref(), &block1_updated)
            .await
            .unwrap();

        let stats_after_update = chaindb.get_chain_stats().await.unwrap();
        assert_eq!(stats_after_update.total_blocks, 3);
        assert_eq!(stats_after_update.total_transactions, 4);
        assert_eq!(stats_after_update.chain_height, 2);

        chaindb.backend().delete(CHAIN_STATS_KEY).await.unwrap();

        let stats_recomputed = chaindb.get_chain_stats().await.unwrap();
        assert_eq!(stats_recomputed.total_blocks, 3);
        assert_eq!(stats_recomputed.total_transactions, 4);
        assert_eq!(stats_recomputed.chain_height, 2);
        assert!(stats_recomputed.database_size > 0);
        assert!(chaindb.backend().exists(CHAIN_STATS_KEY).await.unwrap());

        let stats_forced = chaindb.recalculate_chain_stats().await.unwrap();
        assert_eq!(stats_forced.total_blocks, 3);
        assert_eq!(stats_forced.total_transactions, 4);
        assert_eq!(stats_forced.chain_height, 2);
        assert!(stats_forced.database_size > 0);
    }
}
