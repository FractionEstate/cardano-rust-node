//! Storage layer integration tests
//!
//! This module contains comprehensive tests for storage backends including LMDB,
//! CardanoDB, and storage interface compatibility testing.

pub mod test_chaindb;
pub mod test_ledgerdb;
pub mod test_lmdb_backend;
pub mod test_storage_interface;

// Additional test modules (moved from tests root)
pub mod integration_storage;
pub mod test_lmdb_backend_standalone;
pub mod test_lmdb_integration;
pub mod test_storage_interface_standalone;

// Common test utilities for storage tests
use cardano_storage::{Result, StorageError};
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a temporary directory for test databases
pub fn create_test_dir() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let path = temp_dir.path().to_path_buf();
    (temp_dir, path)
}

/// Test data generators for storage testing
pub mod generators {
    use cardano_crypto::Blake2b256Hash;

    /// Generate test block hash
    pub fn test_block_hash(seed: u8) -> Blake2b256Hash {
        let mut bytes = [0u8; 32];
        bytes[0] = seed;
        bytes[31] = seed.wrapping_add(100);
        Blake2b256Hash::from(bytes)
    }

    /// Generate test transaction data
    pub fn test_transaction_data(id: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&id.to_be_bytes());
        data.extend_from_slice(b"test_transaction_data");
        data.extend_from_slice(&(id * 2).to_be_bytes());
        data
    }

    /// Generate test block data
    pub fn test_block_data(height: u64) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&height.to_be_bytes());
        data.extend_from_slice(b"test_block_data_");
        data.extend_from_slice(&(height % 256) as u8);
        // Simulate block data of varying sizes
        let size = 1000 + (height % 5000) as usize;
        data.resize(size, (height % 256) as u8);
        data
    }
}

/// Common assertion helpers
pub fn assert_storage_error_type(result: Result<()>, expected_contains: &str) {
    match result {
        Err(StorageError::DatabaseError(msg)) => {
            assert!(
                msg.contains(expected_contains),
                "Expected error message to contain '{}', got '{}'",
                expected_contains,
                msg
            );
        }
        Err(other) => panic!("Expected DatabaseError, got: {:?}", other),
        Ok(_) => panic!("Expected error but got Ok"),
    }
}
