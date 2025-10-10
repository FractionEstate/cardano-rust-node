//! Storage Interface Compatibility Tests
//!
//! This module tests storage interface compatibility across different backends
//! (LMDB, CardanoDB) to ensure consistent behavior and interoperability.
//! Based on the Cardano Haskell node storage patterns and requirements.

use super::{create_test_dir, generators, assert_storage_error_type};
use cardano_storage::{StorageError, Result};
use cardano_crypto::Blake2b256Hash;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

/// Generic storage backend trait for testing interface compatibility
pub trait StorageBackend: Send + Sync {
    /// Store a key-value pair
    fn put(&self, key: &[u8], value: &[u8]) -> Result<()>;

    /// Retrieve a value by key
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Delete a key-value pair
    fn delete(&self, key: &[u8]) -> Result<()>;

    /// Check if a key exists
    fn exists(&self, key: &[u8]) -> Result<bool>;

    /// Batch write operations (atomic)
    fn batch_write(&self, operations: &[BatchOperation]) -> Result<()>;

    /// Get storage backend name for testing
    fn backend_name(&self) -> &str;

    /// Sync data to disk
    fn sync(&self) -> Result<()>;

    /// Get database statistics
    fn get_stats(&self) -> Result<StorageStats>;
}

/// Batch operation types for atomic writes
#[derive(Debug, Clone)]
pub enum BatchOperation {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}

/// Storage backend statistics
#[derive(Debug, Clone, PartialEq)]
pub struct StorageStats {
    pub total_keys: u64,
    pub total_size: u64,
    pub backend_specific: HashMap<String, String>,
}

/// Mock LMDB backend implementation for testing
pub struct MockLMDBBackend {
    data: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
    path: std::path::PathBuf,
    _temp_dir: TempDir,
}

impl MockLMDBBackend {
    pub fn new() -> Result<Self> {
        let (temp_dir, path) = create_test_dir();
        Ok(Self {
            data: Arc::new(Mutex::new(HashMap::new())),
            path,
            _temp_dir: temp_dir,
        })
    }
}

impl StorageBackend for MockLMDBBackend {
    fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        if key.is_empty() {
            return Err(StorageError::DatabaseError("Empty key not allowed".to_string()));
        }

        if value.len() > 16 * 1024 * 1024 {
            return Err(StorageError::DatabaseError("Value exceeds LMDB limits".to_string()));
        }

        let mut data = self.data.lock().unwrap();
        data.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        if key.is_empty() {
            return Err(StorageError::DatabaseError("Empty key not allowed".to_string()));
        }

        let data = self.data.lock().unwrap();
        Ok(data.get(key).cloned())
    }

    fn delete(&self, key: &[u8]) -> Result<()> {
        if key.is_empty() {
            return Err(StorageError::DatabaseError("Empty key not allowed".to_string()));
        }

        let mut data = self.data.lock().unwrap();
        data.remove(key);
        Ok(())
    }

    fn exists(&self, key: &[u8]) -> Result<bool> {
        if key.is_empty() {
            return Err(StorageError::DatabaseError("Empty key not allowed".to_string()));
        }

        let data = self.data.lock().unwrap();
        Ok(data.contains_key(key))
    }

    fn batch_write(&self, operations: &[BatchOperation]) -> Result<()> {
        let mut data = self.data.lock().unwrap();

        // Simulate LMDB transaction behavior
        let mut temp_changes = HashMap::new();
        let mut temp_deletions = Vec::new();

        for op in operations {
            match op {
                BatchOperation::Put { key, value } => {
                    if key.is_empty() {
                        return Err(StorageError::DatabaseError("Empty key in batch operation".to_string()));
                    }
                    if value.len() > 16 * 1024 * 1024 {
                        return Err(StorageError::DatabaseError("Value exceeds LMDB limits".to_string()));
                    }
                    temp_changes.insert(key.clone(), value.clone());
                }
                BatchOperation::Delete { key } => {
                    if key.is_empty() {
                        return Err(StorageError::DatabaseError("Empty key in batch operation".to_string()));
                    }
                    temp_deletions.push(key.clone());
                }
            }
        }

        // Apply all changes atomically
        for (key, value) in temp_changes {
            data.insert(key, value);
        }
        for key in temp_deletions {
            data.remove(&key);
        }

        Ok(())
    }

    fn backend_name(&self) -> &str {
        "MockLMDB"
    }

    fn sync(&self) -> Result<()> {
        // Mock sync operation
        thread::sleep(Duration::from_millis(1));
        Ok(())
    }

    fn get_stats(&self) -> Result<StorageStats> {
        let data = self.data.lock().unwrap();
        let total_keys = data.len() as u64;
        let total_size: usize = data.iter()
            .map(|(k, v)| k.len() + v.len())
            .sum();

        let mut backend_specific = HashMap::new();
        backend_specific.insert("lmdb_map_size".to_string(), "16GB".to_string());
        backend_specific.insert("lmdb_max_dbs".to_string(), "10".to_string());

        Ok(StorageStats {
            total_keys,
            total_size: total_size as u64,
            backend_specific,
        })
    }
}

/// Mock CardanoDB backend implementation for testing
pub struct MockCardanoDbBackend {
    data: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
    path: std::path::PathBuf,
    _temp_dir: TempDir,
}

impl MockCardanoDbBackend {
    pub fn new() -> Result<Self> {
        let (temp_dir, path) = create_test_dir();
        Ok(Self {
            data: Arc::new(Mutex::new(HashMap::new())),
            path,
            _temp_dir: temp_dir,
        })
    }
}

impl StorageBackend for MockCardanoDbBackend {
    fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        // CardanoDB is more permissive with empty keys in some contexts
        if value.len() > 64 * 1024 * 1024 {
            return Err(StorageError::DatabaseError("Value exceeds CardanoDB limits".to_string()));
        }

        let mut data = self.data.lock().unwrap();
        data.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let data = self.data.lock().unwrap();
        Ok(data.get(key).cloned())
    }

    fn delete(&self, key: &[u8]) -> Result<()> {
        let mut data = self.data.lock().unwrap();
        data.remove(key);
        Ok(())
    }

    fn exists(&self, key: &[u8]) -> Result<bool> {
        let data = self.data.lock().unwrap();
        Ok(data.contains_key(key))
    }

    fn batch_write(&self, operations: &[BatchOperation]) -> Result<()> {
        let mut data = self.data.lock().unwrap();

        // Simulate CardanoDB WriteBatch behavior
        for op in operations {
            match op {
                BatchOperation::Put { key, value } => {
                    if value.len() > 64 * 1024 * 1024 {
                        return Err(StorageError::DatabaseError("Value exceeds CardanoDB limits".to_string()));
                    }
                    data.insert(key.clone(), value.clone());
                }
                BatchOperation::Delete { key } => {
                    data.remove(key);
                }
            }
        }

        Ok(())
    }

    fn backend_name(&self) -> &str {
        "MockCardanoDB"
    }

    fn sync(&self) -> Result<()> {
        // Mock sync operation
        thread::sleep(Duration::from_millis(2));
        Ok(())
    }

    fn get_stats(&self) -> Result<StorageStats> {
        let data = self.data.lock().unwrap();
        let total_keys = data.len() as u64;
        let total_size: usize = data.iter()
            .map(|(k, v)| k.len() + v.len())
            .sum();

        let mut backend_specific = HashMap::new();
        backend_specific.insert("cardanodb_version".to_string(), "8.7.3".to_string());
        backend_specific.insert("compression".to_string(), "lz4".to_string());

        Ok(StorageStats {
            total_keys,
            total_size: total_size as u64,
            backend_specific,
        })
    }
}

/// Test helper to run interface compatibility tests across backends
fn test_with_all_backends<F>(test_fn: F)
where
    F: Fn(&dyn StorageBackend) + std::panic::UnwindSafe + Copy,
{
    // Test with LMDB backend
    let lmdb_backend = MockLMDBBackend::new().expect("Failed to create LMDB backend");
    std::panic::catch_unwind(|| test_fn(&lmdb_backend))
        .unwrap_or_else(|_| panic!("Test failed for LMDB backend"));

    // Test with CardanoDB backend
    let cardanodb_backend = MockCardanoDbBackend::new().expect("Failed to create CardanoDB backend");
    std::panic::catch_unwind(|| test_fn(&cardanodb_backend))
        .unwrap_or_else(|_| panic!("Test failed for CardanoDB backend"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_put_get_compatibility() {
        test_with_all_backends(|backend| {
            let key = b"test_key";
            let value = b"test_value";

            // Put and get should work consistently across backends
            backend.put(key, value).expect("Put should succeed");
            let retrieved = backend.get(key).expect("Get should succeed");
            assert_eq!(retrieved, Some(value.to_vec()));

            // Non-existent key should return None
            let non_existent = backend.get(b"non_existent").expect("Get should succeed");
            assert_eq!(non_existent, None);
        });
    }

    #[test]
    fn test_delete_compatibility() {
        test_with_all_backends(|backend| {
            let key = b"delete_test";
            let value = b"value_to_delete";

            // Put, verify, delete, verify
            backend.put(key, value).expect("Put should succeed");
            assert!(backend.exists(key).expect("Exists should succeed"));

            backend.delete(key).expect("Delete should succeed");
            assert!(!backend.exists(key).expect("Exists should succeed"));

            let retrieved = backend.get(key).expect("Get should succeed");
            assert_eq!(retrieved, None);
        });
    }

    #[test]
    fn test_batch_operations_compatibility() {
        test_with_all_backends(|backend| {
            let operations = vec![
                BatchOperation::Put {
                    key: b"batch_key1".to_vec(),
                    value: b"batch_value1".to_vec()
                },
                BatchOperation::Put {
                    key: b"batch_key2".to_vec(),
                    value: b"batch_value2".to_vec()
                },
                BatchOperation::Delete {
                    key: b"batch_key1".to_vec()
                },
            ];

            backend.batch_write(&operations).expect("Batch write should succeed");

            // Verify results
            assert!(!backend.exists(b"batch_key1").expect("Exists should succeed"));
            assert!(backend.exists(b"batch_key2").expect("Exists should succeed"));

            let value2 = backend.get(b"batch_key2").expect("Get should succeed");
            assert_eq!(value2, Some(b"batch_value2".to_vec()));
        });
    }

    #[test]
    fn test_empty_key_handling() {
        // Note: Different backends may have different behaviors for empty keys
        let lmdb_backend = MockLMDBBackend::new().expect("Failed to create LMDB backend");
        let cardanodb_backend = MockCardanoDbBackend::new().expect("Failed to create CardanoDB backend");

        // LMDB should reject empty keys
        let lmdb_result = lmdb_backend.put(b"", b"value");
        assert!(lmdb_result.is_err());

        // CardanoDB might be more permissive (implementation dependent)
        let cardanodb_result = cardanodb_backend.put(b"", b"value");
        // For this test, we'll allow both behaviors but document the difference
        match cardanodb_result {
            Ok(()) => println!("CardanoDB allows empty keys"),
            Err(_) => println!("CardanoDB rejects empty keys"),
        }
    }

    #[test]
    fn test_large_value_limits() {
        test_with_all_backends(|backend| {
            let key = b"large_value_test";

            // Test normal size value
            let normal_value = vec![0u8; 1024]; // 1KB
            backend.put(key, &normal_value).expect("Normal value should succeed");

            // Different backends have different limits
            let large_value = match backend.backend_name() {
                "MockLMDB" => vec![0u8; 20 * 1024 * 1024], // Exceeds 16MB limit
                "MockCardanoDB" => vec![0u8; 70 * 1024 * 1024], // Exceeds 64MB limit
                _ => vec![0u8; 100 * 1024 * 1024], // Very large
            };

            let result = backend.put(b"large_key", &large_value);
            assert!(result.is_err(), "Large value should be rejected by {}", backend.backend_name());
        });
    }

    #[test]
    fn test_concurrent_access_compatibility() {
        test_with_all_backends(|backend| {
            let backend = Arc::new(backend);
            let mut handles = vec![];

            // Spawn multiple threads for concurrent access
            for i in 0..10 {
                let backend_clone = Arc::clone(&backend);
                let handle = thread::spawn(move || {
                    let key = format!("concurrent_key_{}", i);
                    let value = format!("concurrent_value_{}", i);

                    backend_clone.put(key.as_bytes(), value.as_bytes())
                        .expect("Concurrent put should succeed");

                    let retrieved = backend_clone.get(key.as_bytes())
                        .expect("Concurrent get should succeed");
                    assert_eq!(retrieved, Some(value.into_bytes()));
                });
                handles.push(handle);
            }

            // Wait for all threads to complete
            for handle in handles {
                handle.join().expect("Thread should complete successfully");
            }
        });
    }

    #[test]
    fn test_stats_compatibility() {
        test_with_all_backends(|backend| {
            // Get initial stats
            let initial_stats = backend.get_stats().expect("Get stats should succeed");

            // Add some data
            for i in 0..5 {
                let key = format!("stats_key_{}", i);
                let value = format!("stats_value_{}", i);
                backend.put(key.as_bytes(), value.as_bytes()).expect("Put should succeed");
            }

            // Get final stats
            let final_stats = backend.get_stats().expect("Get stats should succeed");

            // Verify stats increased
            assert!(final_stats.total_keys >= initial_stats.total_keys + 5);
            assert!(final_stats.total_size > initial_stats.total_size);

            // Verify backend-specific stats are present
            assert!(!final_stats.backend_specific.is_empty());
        });
    }

    #[test]
    fn test_sync_compatibility() {
        test_with_all_backends(|backend| {
            // Add some data
            backend.put(b"sync_key", b"sync_value").expect("Put should succeed");

            // Sync should succeed for all backends
            backend.sync().expect("Sync should succeed");

            // Data should still be accessible after sync
            let retrieved = backend.get(b"sync_key").expect("Get should succeed");
            assert_eq!(retrieved, Some(b"sync_value".to_vec()));
        });
    }

    #[test]
    fn test_cardano_specific_data_patterns() {
        test_with_all_backends(|backend| {
            // Test with Cardano-specific data patterns from Haskell node

            // Block hash keys (32 bytes)
            let block_hash = generators::test_block_hash(42);
            let block_data = generators::test_block_data(12345);

            backend.put(block_hash.as_bytes(), &block_data)
                .expect("Block data storage should succeed");

            let retrieved_block = backend.get(block_hash.as_bytes())
                .expect("Block data retrieval should succeed");
            assert_eq!(retrieved_block, Some(block_data));

            // Transaction data
            let tx_data = generators::test_transaction_data(67890);
            let tx_key = b"tx_67890";

            backend.put(tx_key, &tx_data)
                .expect("Transaction data storage should succeed");

            let retrieved_tx = backend.get(tx_key)
                .expect("Transaction data retrieval should succeed");
            assert_eq!(retrieved_tx, Some(tx_data));

            // UTXO keys (typical pattern: txhash + output_index)
            let utxo_key = [block_hash.as_bytes(), &0u32.to_be_bytes()].concat();
            let utxo_data = b"utxo_output_data";

            backend.put(&utxo_key, utxo_data)
                .expect("UTXO data storage should succeed");

            let retrieved_utxo = backend.get(&utxo_key)
                .expect("UTXO data retrieval should succeed");
            assert_eq!(retrieved_utxo, Some(utxo_data.to_vec()));
        });
    }

    #[test]
    fn test_error_consistency_across_backends() {
        test_with_all_backends(|backend| {
            // Test that similar error conditions produce appropriate errors
            // (though the specific error messages may differ)

            // Both backends should handle database errors gracefully
            let large_key = vec![0u8; 1024 * 1024]; // Very large key
            match backend.put(&large_key, b"value") {
                Ok(()) => {
                    // Some backends might accept this
                    println!("Backend {} accepts large keys", backend.backend_name());
                }
                Err(e) => {
                    // Error should be a DatabaseError
                    match e {
                        StorageError::DatabaseError(_) => {
                            // Expected
                        }
                        other => panic!("Expected DatabaseError, got {:?}", other),
                    }
                }
            }
        });
    }

    #[test]
    fn test_haskell_node_compatibility_patterns() {
        test_with_all_backends(|backend| {
            // Test patterns based on Haskell Cardano node storage usage

            // ChainDB patterns - immutable blocks
            let chain_tip_key = b"chain_tip";
            let chain_tip_data = [0u8; 8]; // BlockNo as u64
            backend.put(chain_tip_key, &chain_tip_data)
                .expect("Chain tip storage should succeed");

            // LedgerDB patterns - ledger state snapshots
            let ledger_snapshot_key = b"ledger_snapshot_12345";
            let ledger_data = vec![1u8; 4096]; // Simulated ledger state
            backend.put(ledger_snapshot_key, &ledger_data)
                .expect("Ledger snapshot storage should succeed");

            // VolatileDB patterns - recent blocks
            for height in 100..110 {
                let volatile_key = format!("volatile_block_{}", height);
                let volatile_data = generators::test_block_data(height);
                backend.put(volatile_key.as_bytes(), &volatile_data)
                    .expect("Volatile block storage should succeed");
            }

            // Verify all data is retrievable
            assert!(backend.exists(chain_tip_key).expect("Exists should succeed"));
            assert!(backend.exists(ledger_snapshot_key).expect("Exists should succeed"));
            assert!(backend.exists(b"volatile_block_105").expect("Exists should succeed"));

            // Test bulk operations similar to Haskell node patterns
            let bulk_ops: Vec<BatchOperation> = (200..210)
                .map(|i| BatchOperation::Put {
                    key: format!("bulk_block_{}", i).into_bytes(),
                    value: generators::test_block_data(i),
                })
                .collect();

            backend.batch_write(&bulk_ops)
                .expect("Bulk block storage should succeed");

            // Verify bulk operations
            for i in 200..210 {
                let key = format!("bulk_block_{}", i);
                assert!(backend.exists(key.as_bytes()).expect("Exists should succeed"));
            }
        });
    }
}

// Add helper function for storage error type assertion
#[allow(dead_code)]
pub fn assert_storage_error_type(result: Result<()>, expected_error_type: &str) {
    match result {
        Ok(()) => panic!("Expected error but got Ok(())"),
        Err(e) => {
            let error_string = format!("{}", e);
            assert!(
                error_string.contains(expected_error_type),
                "Expected error containing '{}', got '{}'",
                expected_error_type,
                error_string
            );
        }
    }
}

// Export test functions for integration testing
pub async fn test_storage_interface_lmdb_basic_operations() {
    let lmdb = MockLMDBBackend::new();
    test_backend_basic_operations(&lmdb).await;
}

pub async fn test_storage_interface_cardanodb_basic_operations() {
    let cardanodb = MockCardanoDbBackend::new();
    test_backend_basic_operations(&cardanodb).await;
}

pub async fn test_storage_interface_lmdb_concurrent_access() {
    let lmdb = MockLMDBBackend::new();
    test_backend_concurrent_access(&lmdb).await;
}

pub async fn test_storage_interface_cardanodb_concurrent_access() {
    let cardanodb = MockCardanoDbBackend::new();
    test_backend_concurrent_access(&cardanodb).await;
}

pub async fn test_storage_interface_lmdb_batch_operations() {
    let lmdb = MockLMDBBackend::new();
    test_backend_batch_operations(&lmdb).await;
}

pub async fn test_storage_interface_cardanodb_batch_operations() {
    let cardanodb = MockCardanoDbBackend::new();
    test_backend_batch_operations(&cardanodb).await;
}

pub async fn test_storage_interface_lmdb_cardano_patterns() {
    let lmdb = MockLMDBBackend::new();
    test_backend_cardano_patterns(&lmdb).await;
}

pub async fn test_storage_interface_cardanodb_cardano_patterns() {
    let cardanodb = MockCardanoDbBackend::new();
    test_backend_cardano_patterns(&cardanodb).await;
}

pub async fn test_storage_interface_lmdb_error_handling() {
    let lmdb = MockLMDBBackend::new();
    test_backend_error_handling(&lmdb).await;
}

pub async fn test_storage_interface_cardanodb_error_handling() {
    let cardanodb = MockCardanoDbBackend::new();
    test_backend_error_handling(&cardanodb).await;
}

pub async fn test_storage_interface_backend_statistics() {
    let lmdb = MockLMDBBackend::new();
    let cardanodb = MockCardanoDbBackend::new();
    test_backend_statistics(&lmdb).await;
    test_backend_statistics(&cardanodb).await;
}

pub async fn test_storage_interface_haskell_compatibility() {
    let lmdb = MockLMDBBackend::new();
    let cardanodb = MockCardanoDbBackend::new();
    test_backend_haskell_compatibility(&lmdb).await;
    test_backend_haskell_compatibility(&cardanodb).await;
}
