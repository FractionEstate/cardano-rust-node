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
}

// Key prefixes for different data types
const BLOCK_PREFIX: &[u8] = b"block:";
const TX_PREFIX: &[u8] = b"tx:";
const METADATA_KEY: &[u8] = b"chain:metadata";
const BLOCK_TXS_PREFIX: &[u8] = b"block_txs:";

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
        block_data: &[u8],
        tx_data: &[(Blake2b256Hash, Vec<u8>)],
    ) -> Result<()> {
        // Store block
        self.backend.put(&block_key(block_hash), block_data).await?;

        // Store transactions and collect hashes
        let mut tx_hashes = Vec::new();
        for (tx_hash, data) in tx_data {
            self.backend.put(&tx_key(tx_hash), data).await?;
            tx_hashes.push(*tx_hash);
        }

        // Store block-to-transactions mapping
        if !tx_hashes.is_empty() {
            self.store_block_transactions(block_hash, &tx_hashes)
                .await?;
        }

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
        _start_hash: &Blake2b256Hash,
        _count: u32,
    ) -> Result<Vec<Vec<u8>>> {
        // TODO: Implement efficient range queries
        // This would require additional indexing by height or chain order
        Ok(Vec::new())
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
        // TODO: Implement efficient statistics collection
        // This could be cached and updated incrementally
        Ok(ChainDatabaseStats {
            total_blocks: 0,
            total_transactions: 0,
            chain_height: 0,
            database_size: 0,
        })
    }
}

#[cfg(test)]
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
            .store_block(&block_hash, &block_data, &[(tx_hash, tx_data.clone())])
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
            .store_block(&block_hash, &block_data, &[(tx_hash, tx_data.clone())])
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
