//! In-Memory Storage Backend
//!
//! A simple in-memory storage backend for testing purposes.
//! Uses HashMap for storage with no persistence.
//!
//! ⚠️  NOT FOR PRODUCTION USE - Data is lost when dropped.

use super::{BackendStats, BatchOperation, StorageBackend};
use crate::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// In-memory storage backend using HashMap
#[derive(Debug, Clone)]
pub struct MemoryBackend {
    store: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl MemoryBackend {
    /// Create a new in-memory backend
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the number of stored entries (for testing)
    pub fn len(&self) -> usize {
        self.store.read().unwrap().len()
    }

    /// Check if the backend is empty (for testing)
    pub fn is_empty(&self) -> bool {
        self.store.read().unwrap().is_empty()
    }

    /// Clear all data (for testing)
    pub fn clear(&self) {
        self.store.write().unwrap().clear();
    }
}

impl Default for MemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StorageBackend for MemoryBackend {
    async fn init(&self) -> Result<()> {
        // No initialization needed for memory backend
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        // No cleanup needed
        Ok(())
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let mut store = self.store.write().unwrap();
        store.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let store = self.store.read().unwrap();
        Ok(store.get(key).cloned())
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        let mut store = self.store.write().unwrap();
        store.remove(key);
        Ok(())
    }

    async fn exists(&self, key: &[u8]) -> Result<bool> {
        let store = self.store.read().unwrap();
        Ok(store.contains_key(key))
    }

    async fn batch(&self, operations: Vec<BatchOperation>) -> Result<()> {
        let mut store = self.store.write().unwrap();

        for op in operations {
            match op {
                BatchOperation::Put { key, value } => {
                    store.insert(key, value);
                }
                BatchOperation::Delete { key } => {
                    store.remove(&key);
                }
            }
        }

        Ok(())
    }

    async fn sync(&self) -> Result<()> {
        // No-op for memory backend (always "synced")
        Ok(())
    }

    async fn stats(&self) -> Result<BackendStats> {
        let store = self.store.read().unwrap();

        let total_keys = store.len() as u64;

        let total_size: usize = store.iter().map(|(k, v)| k.len() + v.len()).sum();

        Ok(BackendStats {
            total_keys,
            total_size: total_size as u64,
            memory_usage: total_size as u64, // Approximate
        })
    }

    async fn scan_prefix(
        &self,
        prefix: &[u8],
        limit: Option<usize>,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let store = self.store.read().unwrap();
        let mut results = Vec::new();

        for (key, value) in store.iter() {
            if key.starts_with(prefix) {
                results.push((key.clone(), value.clone()));
                if let Some(limit) = limit {
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_backend_basic() {
        let backend = MemoryBackend::new();

        backend.init().await.unwrap();

        // Put and get
        backend.put(b"key1", b"value1").await.unwrap();
        let value = backend.get(b"key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Exists
        assert!(backend.exists(b"key1").await.unwrap());
        assert!(!backend.exists(b"key2").await.unwrap());

        // Delete
        backend.delete(b"key1").await.unwrap();
        assert!(!backend.exists(b"key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_backend_batch() {
        let backend = MemoryBackend::new();

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
                key: b"key1".to_vec(),
            },
        ];

        backend.batch(operations).await.unwrap();

        assert!(!backend.exists(b"key1").await.unwrap());
        assert!(backend.exists(b"key2").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_backend_stats() {
        let backend = MemoryBackend::new();

        backend.put(b"key1", b"value1").await.unwrap();
        backend.put(b"key2", b"value2").await.unwrap();

        let stats = backend.stats().await.unwrap();
        assert_eq!(stats.total_keys, 2);
        assert!(stats.total_size > 0);
    }
}
