//! Backend storage implementations
//!
//! This module provides pluggable storage backends for CardanoDB.

#[cfg(feature = "legacy")]
pub mod lmdb;
#[cfg(feature = "legacy")]
pub mod rocksdb;

#[cfg(feature = "legacy")]
pub use lmdb::{LmdbBackend, LmdbConfig};
#[cfg(feature = "legacy")]
pub use rocksdb::{RocksDbBackend, RocksDbConfig};

use crate::Result;
use async_trait::async_trait;

/// Generic storage backend trait
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Initialize the storage backend
    async fn init(&self) -> Result<()>;

    /// Close the storage backend
    async fn close(&self) -> Result<()>;

    /// Store a key-value pair
    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()>;

    /// Retrieve a value by key
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Delete a key-value pair
    async fn delete(&self, key: &[u8]) -> Result<()>;

    /// Check if a key exists
    async fn exists(&self, key: &[u8]) -> Result<bool>;

    /// Batch operations for atomic writes
    async fn batch(&self, operations: Vec<BatchOperation>) -> Result<()>;

    /// Sync data to disk
    async fn sync(&self) -> Result<()>;

    /// Get backend statistics
    async fn stats(&self) -> Result<BackendStats>;
}

/// Batch operation types
#[derive(Debug, Clone)]
pub enum BatchOperation {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}

/// Backend statistics
#[derive(Debug, Clone)]
pub struct BackendStats {
    pub total_keys: u64,
    pub total_size: u64,
    pub memory_usage: u64,
}
