//! RocksDB storage backend implementation
//!
//! This module provides a RocksDB storage backend for Cardano blockchain data.
//! RocksDB is well-suited for UTXO state management with excellent write performance.

use super::{BackendStats, BatchOperation, StorageBackend};
use crate::{Result, StorageError};
use async_trait::async_trait;
use rocksdb::{IteratorMode, Options, WriteBatch, DB};
use std::path::Path;
use std::sync::Arc;
use tokio::task;

/// Configuration for RocksDB backend
#[derive(Debug, Clone)]
pub struct RocksDbConfig {
    /// Database path
    pub path: std::path::PathBuf,
    /// Write buffer size (default: 64MB)
    pub write_buffer_size: usize,
    /// Maximum number of background compactions (default: 4)
    pub max_background_compactions: i32,
    /// Block cache size (default: 256MB)
    pub block_cache_size: usize,
    /// Enable compression (default: true)
    pub compression: bool,
    /// Maximum number of open files (default: 1000)
    pub max_open_files: i32,
    /// Paranoid checks (default: false for performance)
    pub paranoid_checks: bool,
    /// Bloom filter bits per key for better read performance (default: 10)
    pub bloom_filter_bits_per_key: i32,
}

impl Default for RocksDbConfig {
    fn default() -> Self {
        Self {
            path: std::path::PathBuf::from("./cardano-rocksdb"),
            write_buffer_size: 64 * 1024 * 1024, // 64MB
            max_background_compactions: 4,
            block_cache_size: 256 * 1024 * 1024, // 256MB
            compression: true,
            max_open_files: 1000,
            paranoid_checks: false,
            bloom_filter_bits_per_key: 10,
        }
    }
}

impl RocksDbConfig {
    /// Create a new RocksDB configuration with the given path
    pub fn with_path<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            ..Default::default()
        }
    }

    /// Convert to RocksDB Options
    pub fn to_rocksdb_options(&self) -> Options {
        let mut opts = Options::default();

        // Basic settings
        opts.create_if_missing(true);
        opts.set_write_buffer_size(self.write_buffer_size);
        // Note: max_background_compactions is deprecated, RocksDB auto-decides based on max_background_jobs
        opts.set_max_background_jobs(self.max_background_compactions);
        opts.set_max_open_files(self.max_open_files);
        opts.set_paranoid_checks(self.paranoid_checks);

        // Compression
        if self.compression {
            opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        } else {
            opts.set_compression_type(rocksdb::DBCompressionType::None);
        }

        // Block-based table options for better caching
        let mut block_opts = rocksdb::BlockBasedOptions::default();
        block_opts.set_block_cache(&rocksdb::Cache::new_lru_cache(self.block_cache_size));
        if self.bloom_filter_bits_per_key > 0 {
            block_opts.set_bloom_filter(self.bloom_filter_bits_per_key as f64, false);
        }
        opts.set_block_based_table_factory(&block_opts);

        opts
    }
}

/// RocksDB storage backend
pub struct RocksDbBackend {
    db: Arc<DB>,
    config: RocksDbConfig,
}

impl RocksDbBackend {
    /// Create a new RocksDB backend with the given configuration
    pub fn new(config: RocksDbConfig) -> Result<Self> {
        // Create directory if it doesn't exist
        std::fs::create_dir_all(&config.path).map_err(|e| {
            StorageError::DatabaseError(format!("Failed to create database directory: {}", e))
        })?;

        let opts = config.to_rocksdb_options();
        let db = DB::open(&opts, &config.path)
            .map_err(|e| StorageError::DatabaseError(format!("Failed to open RocksDB: {}", e)))?;

        Ok(Self {
            db: Arc::new(db),
            config,
        })
    }

    /// Create a new RocksDB backend with default configuration at the given path
    pub fn with_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config = RocksDbConfig::with_path(path);
        Self::new(config)
    }

    /// Get the database reference
    pub fn db(&self) -> &Arc<DB> {
        &self.db
    }

    /// Get a reference to the backend configuration
    pub fn config(&self) -> &RocksDbConfig {
        &self.config
    }

    /// Flush all data to disk
    pub fn flush(&self) -> Result<()> {
        self.db
            .flush()
            .map_err(|e| StorageError::DatabaseError(format!("Failed to flush RocksDB: {}", e)))
    }

    /// Compact the database
    pub fn compact(&self) -> Result<()> {
        self.db.compact_range(None::<&[u8]>, None::<&[u8]>);
        Ok(())
    }
}

#[async_trait]
impl StorageBackend for RocksDbBackend {
    async fn init(&self) -> Result<()> {
        // RocksDB is initialized during construction
        // Perform basic validation
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> Result<()> {
            // Test basic read operation to ensure DB is working
            let _ = db
                .get(b"__health_check__")
                .map_err(|e| StorageError::DatabaseError(format!("Health check failed: {}", e)))?;
            Ok(())
        })
        .await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))??;

        Ok(())
    }

    async fn close(&self) -> Result<()> {
        // Flush any remaining data
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> Result<()> {
            db.flush().map_err(|e| {
                StorageError::DatabaseError(format!("Failed to flush during close: {}", e))
            })?;

            // RocksDB handles cleanup automatically when dropped
            Ok(())
        })
        .await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))??;

        Ok(())
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let db = Arc::clone(&self.db);
        let key = key.to_vec();
        let value = value.to_vec();

        task::spawn_blocking(move || {
            db.put(&key, &value)
                .map_err(|e| StorageError::DatabaseError(format!("Failed to put key-value: {}", e)))
        })
        .await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))??;

        Ok(())
    }

    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let db = Arc::clone(&self.db);
        let key = key.to_vec();

        let result = task::spawn_blocking(move || {
            db.get(&key)
                .map_err(|e| StorageError::DatabaseError(format!("Failed to get value: {}", e)))
        })
        .await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))??;

        Ok(result)
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        let db = Arc::clone(&self.db);
        let key = key.to_vec();

        task::spawn_blocking(move || {
            db.delete(&key)
                .map_err(|e| StorageError::DatabaseError(format!("Failed to delete key: {}", e)))
        })
        .await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))??;

        Ok(())
    }

    async fn exists(&self, key: &[u8]) -> Result<bool> {
        let db = Arc::clone(&self.db);
        let key = key.to_vec();

        let exists = task::spawn_blocking(move || match db.get(&key) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(StorageError::DatabaseError(format!(
                "Failed to check key existence: {}",
                e
            ))),
        })
        .await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))??;

        Ok(exists)
    }

    async fn batch(&self, operations: Vec<BatchOperation>) -> Result<()> {
        let db = Arc::clone(&self.db);

        task::spawn_blocking(move || {
            let mut batch = WriteBatch::default();

            for operation in operations {
                match operation {
                    BatchOperation::Put { key, value } => {
                        batch.put(&key, &value);
                    }
                    BatchOperation::Delete { key } => {
                        batch.delete(&key);
                    }
                }
            }

            db.write(batch).map_err(|e| {
                StorageError::DatabaseError(format!("Failed to execute batch operations: {}", e))
            })
        })
        .await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))??;

        Ok(())
    }

    async fn sync(&self) -> Result<()> {
        let db = Arc::clone(&self.db);

        task::spawn_blocking(move || {
            db.flush()
                .map_err(|e| StorageError::DatabaseError(format!("Failed to sync database: {}", e)))
        })
        .await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))??;

        Ok(())
    }

    async fn stats(&self) -> Result<BackendStats> {
        let db = Arc::clone(&self.db);

        let stats = task::spawn_blocking(move || -> Result<BackendStats> {
            // Estimate key count by iterating through all keys
            let iterator = db.iterator(IteratorMode::Start);
            let mut total_keys = 0u64;
            let mut total_size = 0u64;

            for item in iterator {
                match item {
                    Ok((key, value)) => {
                        total_keys += 1;
                        total_size += (key.len() + value.len()) as u64;
                    }
                    Err(e) => {
                        return Err(StorageError::DatabaseError(format!(
                            "Failed to iterate database: {}",
                            e
                        )));
                    }
                }
            }

            // Get memory usage statistics from RocksDB
            let memory_usage = match db.property_value(rocksdb::properties::BLOCK_CACHE_USAGE) {
                Ok(Some(usage_str)) => usage_str.parse::<u64>().unwrap_or(0),
                _ => 0, // Fallback if property is not available
            };

            Ok(BackendStats {
                total_keys,
                total_size,
                memory_usage,
            })
        })
        .await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))??;

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_backend() -> (RocksDbBackend, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = RocksDbConfig {
            path: temp_dir.path().to_path_buf(),
            // Use smaller values for testing
            write_buffer_size: 4 * 1024 * 1024, // 4MB
            block_cache_size: 16 * 1024 * 1024, // 16MB
            ..Default::default()
        };
        let backend = RocksDbBackend::new(config).unwrap();
        (backend, temp_dir)
    }

    #[tokio::test]
    async fn test_rocksdb_backend_basic_operations() {
        let (backend, _temp_dir) = create_test_backend().await;

        // Initialize backend
        backend.init().await.unwrap();

        // Test put and get
        let key = b"test_key";
        let value = b"test_value";

        backend.put(key, value).await.unwrap();
        let retrieved = backend.get(key).await.unwrap();
        assert_eq!(retrieved.as_deref(), Some(value.as_slice()));

        // Test exists
        assert!(backend.exists(key).await.unwrap());
        assert!(!backend.exists(b"nonexistent").await.unwrap());

        // Test delete
        backend.delete(key).await.unwrap();
        let retrieved = backend.get(key).await.unwrap();
        assert_eq!(retrieved, None);
        assert!(!backend.exists(key).await.unwrap());

        // Test close
        backend.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_rocksdb_backend_batch_operations() {
        let (backend, _temp_dir) = create_test_backend().await;

        backend.init().await.unwrap();

        // Prepare batch operations
        let operations = vec![
            BatchOperation::Put {
                key: b"key1".to_vec(),
                value: b"value1".to_vec(),
            },
            BatchOperation::Put {
                key: b"key2".to_vec(),
                value: b"value2".to_vec(),
            },
            BatchOperation::Delete {
                key: b"key3".to_vec(),
            },
        ];

        // Execute batch
        backend.batch(operations).await.unwrap();

        // Verify results
        assert_eq!(
            backend.get(b"key1").await.unwrap().as_deref(),
            Some(b"value1".as_slice())
        );
        assert_eq!(
            backend.get(b"key2").await.unwrap().as_deref(),
            Some(b"value2".as_slice())
        );
        assert_eq!(backend.get(b"key3").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_rocksdb_backend_stats() {
        let (backend, _temp_dir) = create_test_backend().await;

        backend.init().await.unwrap();

        // Add some data
        backend.put(b"key1", b"value1").await.unwrap();
        backend.put(b"key2", b"value2").await.unwrap();

        // Get statistics
        let stats = backend.stats().await.unwrap();
        assert_eq!(stats.total_keys, 2);
        assert!(stats.total_size > 0);
    }

    #[tokio::test]
    async fn test_rocksdb_backend_sync() {
        let (backend, _temp_dir) = create_test_backend().await;

        backend.init().await.unwrap();

        // Add data and sync
        backend.put(b"key", b"value").await.unwrap();
        backend.sync().await.unwrap();

        // Verify data persists
        let retrieved = backend.get(b"key").await.unwrap();
        assert_eq!(retrieved.as_deref(), Some(b"value".as_slice()));
    }
}
