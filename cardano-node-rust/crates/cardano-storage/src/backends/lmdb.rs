//! LMDB storage backend implementation
//!
//! This module provides a Lightning Memory-Mapped Database (LMDB) storage backend
//! for Cardano blockchain data. LMDB is particularly well-suited for immutable data
//! with excellent read performance and ACID properties.

use super::{BackendStats, BatchOperation, StorageBackend};
use crate::{Result, StorageError};
use async_trait::async_trait;
use lmdb::{
    Database, Environment, EnvironmentFlags, Transaction, WriteFlags,
};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task;

/// Configuration for LMDB backend
#[derive(Debug, Clone)]
pub struct LmdbConfig {
    /// Database path
    pub path: std::path::PathBuf,
    /// Maximum database size in bytes (default: 1TB)
    pub max_size: usize,
    /// Maximum number of readers (default: 126)
    pub max_readers: u32,
    /// Maximum number of databases (default: 16)
    pub max_dbs: u32,
    /// Sync mode (true for immediate sync, false for eventual consistency)
    pub sync_mode: bool,
    /// Read-only mode
    pub read_only: bool,
}

impl Default for LmdbConfig {
    fn default() -> Self {
        Self {
            path: std::path::PathBuf::from("./cardano-db"),
            max_size: 1024 * 1024 * 1024 * 1024, // 1TB
            max_readers: 126,
            max_dbs: 16,
            sync_mode: true,
            read_only: false,
        }
    }
}

impl LmdbConfig {
    /// Create a new configuration with the specified path
    pub fn with_path<P: Into<std::path::PathBuf>>(path: P) -> crate::Result<Self> {
        Ok(Self {
            path: path.into(),
            ..Self::default()
        })
    }
}

/// LMDB storage backend
pub struct LmdbBackend {
    env: Arc<Environment>,
    db: Arc<RwLock<Database>>,
    config: LmdbConfig,
}

impl LmdbBackend {
    /// Create a new LMDB backend with the given configuration
    pub fn new(config: LmdbConfig) -> Result<Self> {
        // Create directory if it doesn't exist
        std::fs::create_dir_all(&config.path)
            .map_err(|e| StorageError::DatabaseError(format!("Failed to create database directory: {}", e)))?;

        // Configure LMDB environment
        let mut env_flags = EnvironmentFlags::empty();

        if config.read_only {
            env_flags.insert(EnvironmentFlags::READ_ONLY);
        }

        if !config.sync_mode {
            env_flags.insert(EnvironmentFlags::NO_SYNC);
        }

        // Create environment
        let env = Environment::new()
            .set_flags(env_flags)
            .set_max_readers(config.max_readers)
            .set_max_dbs(config.max_dbs)
            .set_map_size(config.max_size)
            .open(&config.path)
            .map_err(|e| StorageError::DatabaseError(format!("Failed to open LMDB environment: {}", e)))?;

        let env = Arc::new(env);

        // Create or open the main database
        let db = {
            let txn = env.begin_ro_txn()
                .map_err(|e| StorageError::DatabaseError(format!("Failed to begin transaction: {}", e)))?;

            let db = env.open_db(Some("cardano_main"))
                .map_err(|e| StorageError::DatabaseError(format!("Failed to open database: {}", e)))?;

            txn.commit()
                .map_err(|e| StorageError::DatabaseError(format!("Failed to commit transaction: {}", e)))?;

            db
        };

        Ok(Self {
            env,
            db: Arc::new(RwLock::new(db)),
            config,
        })
    }

    /// Create a new LMDB backend with default configuration at the given path
    pub fn with_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut config = LmdbConfig::default();
        config.path = path.as_ref().to_path_buf();
        Self::new(config)
    }

    /// Get the environment reference
    pub fn env(&self) -> &Environment {
        &self.env
    }

    /// Begin a read-only transaction
    pub fn begin_ro_txn(&self) -> Result<lmdb::RoTransaction> {
        self.env.begin_ro_txn()
            .map_err(|e| StorageError::DatabaseError(format!("Failed to begin read transaction: {}", e)))
    }

    /// Begin a read-write transaction
    pub fn begin_rw_txn(&self) -> Result<lmdb::RwTransaction> {
        self.env.begin_rw_txn()
            .map_err(|e| StorageError::DatabaseError(format!("Failed to begin write transaction: {}", e)))
    }
}

#[async_trait]
impl StorageBackend for LmdbBackend {
    async fn init(&self) -> Result<()> {
        // LMDB is initialized during construction
        task::spawn_blocking({
            let env = Arc::clone(&self.env);
            move || -> Result<()> {
                // Perform any initialization checks
                let txn = env.begin_ro_txn()
                    .map_err(|e| StorageError::DatabaseError(format!("Failed to begin init transaction: {}", e)))?;

                txn.commit()
                    .map_err(|e| StorageError::DatabaseError(format!("Failed to commit init transaction: {}", e)))?;

                Ok(())
            }
        }).await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))??;

        Ok(())
    }

    async fn close(&self) -> Result<()> {
        // LMDB handles cleanup automatically when dropped
        // Perform final sync if configured
        if self.config.sync_mode {
            task::spawn_blocking({
                let env = Arc::clone(&self.env);
                move || -> Result<()> {
                    env.sync(true)
                        .map_err(|e| StorageError::DatabaseError(format!("Failed to sync on close: {}", e)))?;
                    Ok(())
                }
            }).await
            .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))??;
        }

        Ok(())
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let key = key.to_vec();
        let value = value.to_vec();

        task::spawn_blocking({
            let env = Arc::clone(&self.env);
            let db = Arc::clone(&self.db);
            move || -> Result<()> {
                let db = tokio::runtime::Handle::current().block_on(db.read());

                let mut txn = env.begin_rw_txn()
                    .map_err(|e| StorageError::DatabaseError(format!("Failed to begin write transaction: {}", e)))?;

                txn.put(*db, &key, &value, WriteFlags::empty())
                    .map_err(|e| StorageError::DatabaseError(format!("Failed to put data: {}", e)))?;

                txn.commit()
                    .map_err(|e| StorageError::DatabaseError(format!("Failed to commit transaction: {}", e)))?;

                Ok(())
            }
        }).await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))??;

        Ok(())
    }

    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let key = key.to_vec();

        task::spawn_blocking({
            let env = Arc::clone(&self.env);
            let db = Arc::clone(&self.db);
            move || -> Result<Option<Vec<u8>>> {
                let db = tokio::runtime::Handle::current().block_on(db.read());

                let txn = env.begin_ro_txn()
                    .map_err(|e| StorageError::DatabaseError(format!("Failed to begin read transaction: {}", e)))?;

                match txn.get(*db, &key) {
                    Ok(data) => Ok(Some(data.to_vec())),
                    Err(lmdb::Error::NotFound) => Ok(None),
                    Err(e) => Err(StorageError::DatabaseError(format!("Failed to get data: {}", e))),
                }
            }
        }).await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))?
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        let key = key.to_vec();

        task::spawn_blocking({
            let env = Arc::clone(&self.env);
            let db = Arc::clone(&self.db);
            move || -> Result<()> {
                let db = tokio::runtime::Handle::current().block_on(db.read());

                let mut txn = env.begin_rw_txn()
                    .map_err(|e| StorageError::DatabaseError(format!("Failed to begin write transaction: {}", e)))?;

                match txn.del(*db, &key, None) {
                    Ok(()) => {},
                    Err(lmdb::Error::NotFound) => {
                        // Key doesn't exist, this is not an error
                    },
                    Err(e) => return Err(StorageError::DatabaseError(format!("Failed to delete data: {}", e))),
                }

                txn.commit()
                    .map_err(|e| StorageError::DatabaseError(format!("Failed to commit transaction: {}", e)))?;

                Ok(())
            }
        }).await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))??;

        Ok(())
    }

    async fn exists(&self, key: &[u8]) -> Result<bool> {
        let key = key.to_vec();

        task::spawn_blocking({
            let env = Arc::clone(&self.env);
            let db = Arc::clone(&self.db);
            move || -> Result<bool> {
                let db = tokio::runtime::Handle::current().block_on(db.read());

                let txn = env.begin_ro_txn()
                    .map_err(|e| StorageError::DatabaseError(format!("Failed to begin read transaction: {}", e)))?;

                match txn.get(*db, &key) {
                    Ok(_) => Ok(true),
                    Err(lmdb::Error::NotFound) => Ok(false),
                    Err(e) => Err(StorageError::DatabaseError(format!("Failed to check existence: {}", e))),
                }
            }
        }).await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))?
    }

    async fn batch(&self, operations: Vec<BatchOperation>) -> Result<()> {
        task::spawn_blocking({
            let env = Arc::clone(&self.env);
            let db = Arc::clone(&self.db);
            move || -> Result<()> {
                let db = tokio::runtime::Handle::current().block_on(db.read());

                let mut txn = env.begin_rw_txn()
                    .map_err(|e| StorageError::DatabaseError(format!("Failed to begin batch transaction: {}", e)))?;

                for operation in operations {
                    match operation {
                        BatchOperation::Put { key, value } => {
                            txn.put(*db, &key, &value, WriteFlags::empty())
                                .map_err(|e| StorageError::DatabaseError(format!("Failed to put in batch: {}", e)))?;
                        },
                        BatchOperation::Delete { key } => {
                            match txn.del(*db, &key, None) {
                                Ok(()) => {},
                                Err(lmdb::Error::NotFound) => {
                                    // Key doesn't exist, this is not an error in batch operations
                                },
                                Err(e) => return Err(StorageError::DatabaseError(format!("Failed to delete in batch: {}", e))),
                            }
                        },
                    }
                }

                txn.commit()
                    .map_err(|e| StorageError::DatabaseError(format!("Failed to commit batch transaction: {}", e)))?;

                Ok(())
            }
        }).await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))??;

        Ok(())
    }

    async fn sync(&self) -> Result<()> {
        task::spawn_blocking({
            let env = Arc::clone(&self.env);
            move || -> Result<()> {
                env.sync(true)
                    .map_err(|e| StorageError::DatabaseError(format!("Failed to sync: {}", e)))?;
                Ok(())
            }
        }).await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))??;

        Ok(())
    }

    async fn stats(&self) -> Result<BackendStats> {
        task::spawn_blocking({
            let env = Arc::clone(&self.env);
            let db = Arc::clone(&self.db);
            move || -> Result<BackendStats> {
                let db = tokio::runtime::Handle::current().block_on(db.read());

                let txn = env.begin_ro_txn()
                    .map_err(|e| StorageError::DatabaseError(format!("Failed to begin stats transaction: {}", e)))?;

                let stat = env.stat()
                    .map_err(|e| StorageError::DatabaseError(format!("Failed to get database stats: {}", e)))?;

                // Calculate approximate statistics
                let total_keys = 0u64; // LMDB doesn't provide easy key count
                let page_size = stat.page_size() as u64;
                let total_pages = (stat.leaf_pages() + stat.branch_pages() + stat.overflow_pages()) as u64;
                let total_size = total_pages * page_size;
                let memory_usage = total_size; // Approximate memory usage

                Ok(BackendStats {
                    total_keys,
                    total_size,
                    memory_usage,
                })
            }
        }).await
        .map_err(|e| StorageError::DatabaseError(format!("Task join error: {}", e)))?
    }
}

// Implement Drop to ensure clean shutdown
impl Drop for LmdbBackend {
    fn drop(&mut self) {
        // LMDB environment will be cleaned up automatically
        // No explicit cleanup needed as all operations use RAII
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_backend() -> (LmdbBackend, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let mut config = LmdbConfig {
            path: temp_dir.path().to_path_buf(),
            max_size: 10 * 1024 * 1024, // 10MB for testing
            ..Default::default()
        };
        // Use a unique sub-directory to avoid LMDB lock conflicts
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let counter = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        config.path = config.path.join(format!("test_{}", counter));
        config.max_readers = 1; // Avoid reader slot conflicts in tests
        let backend = LmdbBackend::new(config).unwrap();
        (backend, temp_dir)
    }

    #[tokio::test]
    async fn test_lmdb_backend_basic_operations() {
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
        assert!(!backend.exists(key).await.unwrap());
        assert_eq!(backend.get(key).await.unwrap(), None);

        // Clean shutdown
        backend.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_lmdb_backend_batch_operations() {
        let (backend, _temp_dir) = create_test_backend().await;
        backend.init().await.unwrap();

        let operations = vec![
            BatchOperation::Put {
                key: b"batch_key1".to_vec(),
                value: b"batch_value1".to_vec(),
            },
            BatchOperation::Put {
                key: b"batch_key2".to_vec(),
                value: b"batch_value2".to_vec(),
            },
        ];

        backend.batch(operations).await.unwrap();

        // Verify both keys exist
        assert_eq!(
            backend.get(b"batch_key1").await.unwrap().as_deref(),
            Some(b"batch_value1".as_slice())
        );
        assert_eq!(
            backend.get(b"batch_key2").await.unwrap().as_deref(),
            Some(b"batch_value2".as_slice())
        );

        backend.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_lmdb_backend_stats() {
        let (backend, _temp_dir) = create_test_backend().await;
        backend.init().await.unwrap();

        // Add some data
        for i in 0..10 {
            let key = format!("key_{}", i);
            let value = format!("value_{}", i);
            backend.put(key.as_bytes(), value.as_bytes()).await.unwrap();
        }

        let stats = backend.stats().await.unwrap();
        assert!(stats.total_keys >= 10);
        assert!(stats.total_size > 0);
        assert!(stats.memory_usage > 0);

        backend.close().await.unwrap();
    }
}
