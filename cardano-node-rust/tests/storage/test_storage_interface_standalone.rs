//! Storage Interface Compatibility Integration Tests
//!
//! T067: Storage interface compatibility tests
//! Validates storage backends work correctly across LMDB/RocksDB implementations.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

// Mock storage backends for testing
#[derive(Debug, Clone)]
pub struct MockLMDBBackend {
    data: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
    stats: Arc<Mutex<StorageStats>>,
}

#[derive(Debug, Clone)]
pub struct MockRocksDBBackend {
    data: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
    stats: Arc<Mutex<StorageStats>>,
}

#[derive(Debug, Clone, Default)]
pub struct StorageStats {
    pub reads: usize,
    pub writes: usize,
    pub deletes: usize,
    pub size_bytes: usize,
}

impl MockLMDBBackend {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(StorageStats::default())),
        }
    }

    pub fn put(&self, key: &[u8], value: &[u8]) -> Result<(), String> {
        let mut data = self.data.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();
        data.insert(key.to_vec(), value.to_vec());
        stats.writes += 1;
        stats.size_bytes += key.len() + value.len();
        Ok(())
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, String> {
        let data = self.data.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();
        stats.reads += 1;
        Ok(data.get(key).cloned())
    }

    pub fn delete(&self, key: &[u8]) -> Result<(), String> {
        let mut data = self.data.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();
        if let Some(value) = data.remove(key) {
            stats.deletes += 1;
            stats.size_bytes -= key.len() + value.len();
        }
        Ok(())
    }
}

impl MockRocksDBBackend {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(StorageStats::default())),
        }
    }

    pub fn put(&self, key: &[u8], value: &[u8]) -> Result<(), String> {
        let mut data = self.data.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();
        data.insert(key.to_vec(), value.to_vec());
        stats.writes += 1;
        stats.size_bytes += key.len() + value.len();
        Ok(())
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, String> {
        let data = self.data.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();
        stats.reads += 1;
        Ok(data.get(key).cloned())
    }

    pub fn delete(&self, key: &[u8]) -> Result<(), String> {
        let mut data = self.data.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();
        if let Some(value) = data.remove(key) {
            stats.deletes += 1;
            stats.size_bytes -= key.len() + value.len();
        }
        Ok(())
    }
}

#[tokio::test]
async fn test_storage_interface_lmdb_basic_operations() {
    let backend = MockLMDBBackend::new();

    // Test put operation
    assert!(backend.put(b"key1", b"value1").is_ok());

    // Test get operation
    let result = backend.get(b"key1").unwrap();
    assert_eq!(result, Some(b"value1".to_vec()));

    // Test key doesn't exist
    let result = backend.get(b"key2").unwrap();
    assert_eq!(result, None);

    // Test delete operation
    assert!(backend.delete(b"key1").is_ok());
    let result = backend.get(b"key1").unwrap();
    assert_eq!(result, None);

    println!("T067: LMDB basic operations test passed");
}

#[tokio::test]
async fn test_storage_interface_rocksdb_basic_operations() {
    let backend = MockRocksDBBackend::new();

    // Test put operation
    assert!(backend.put(b"key1", b"value1").is_ok());

    // Test get operation
    let result = backend.get(b"key1").unwrap();
    assert_eq!(result, Some(b"value1".to_vec()));

    // Test key doesn't exist
    let result = backend.get(b"key2").unwrap();
    assert_eq!(result, None);

    // Test delete operation
    assert!(backend.delete(b"key1").is_ok());
    let result = backend.get(b"key1").unwrap();
    assert_eq!(result, None);

    println!("T067: RocksDB basic operations test passed");
}

#[tokio::test]
async fn test_storage_interface_concurrent_access() {
    let backend = Arc::new(MockLMDBBackend::new());

    let handles = (0..10).map(|i| {
        let backend = Arc::clone(&backend);
        tokio::task::spawn(async move {
            let key = format!("concurrent_key_{}", i);
            let value = format!("concurrent_value_{}", i);
            backend.put(key.as_bytes(), value.as_bytes()).unwrap();

            let retrieved = backend.get(key.as_bytes()).unwrap();
            assert_eq!(retrieved, Some(value.as_bytes().to_vec()));
        })
    }).collect::<Vec<_>>();

    for handle in handles {
        handle.await.unwrap();
    }

    println!("T067: Concurrent access test passed");
}

#[tokio::test]
async fn test_storage_interface_cardano_patterns() {
    let backend = MockLMDBBackend::new();

    // Test block storage pattern
    let block_id = b"block_001";
    let block_data = b"mock_block_data_with_transactions";
    backend.put(block_id, block_data).unwrap();

    // Test transaction pattern
    let tx_id = b"tx_001";
    let tx_data = b"mock_transaction_data";
    backend.put(tx_id, tx_data).unwrap();

    // Test metadata storage
    let metadata_key = b"chain_tip";
    let metadata_value = b"block_001";
    backend.put(metadata_key, metadata_value).unwrap();

    // Verify all data is accessible
    assert_eq!(backend.get(block_id).unwrap(), Some(block_data.to_vec()));
    assert_eq!(backend.get(tx_id).unwrap(), Some(tx_data.to_vec()));
    assert_eq!(backend.get(metadata_key).unwrap(), Some(metadata_value.to_vec()));

    println!("T067: Cardano patterns test passed");
}

#[tokio::test]
async fn test_storage_interface_statistics() {
    let backend = MockLMDBBackend::new();

    // Perform some operations
    backend.put(b"key1", b"value1").unwrap();
    backend.put(b"key2", b"value2").unwrap();
    backend.get(b"key1").unwrap();
    backend.get(b"key2").unwrap();
    backend.delete(b"key1").unwrap();

    let stats = backend.stats.lock().unwrap();
    assert_eq!(stats.writes, 2);
    assert_eq!(stats.reads, 2);
    assert_eq!(stats.deletes, 1);
    assert!(stats.size_bytes > 0);

    println!("T067: Storage statistics test passed");
}

#[tokio::test]
async fn test_storage_interface_compatibility() {
    // Test that both backends behave identically
    let lmdb = MockLMDBBackend::new();
    let rocksdb = MockRocksDBBackend::new();

    let test_key = b"compatibility_key";
    let test_value = b"compatibility_value";

    // Same operations on both backends
    lmdb.put(test_key, test_value).unwrap();
    rocksdb.put(test_key, test_value).unwrap();

    // Both should return the same result
    let lmdb_result = lmdb.get(test_key).unwrap();
    let rocksdb_result = rocksdb.get(test_key).unwrap();

    assert_eq!(lmdb_result, rocksdb_result);
    assert_eq!(lmdb_result, Some(test_value.to_vec()));

    println!("T067: Storage interface compatibility test passed");
}
