//! Integration tests for T067: Storage Interface Compatibility Tests
//!
//! This test module validates storage interface compatibility across different backends.
//! Tests ensure LMDB and RocksDB backends work correctly with Cardano-specific patterns.

use super::test_storage_interface::*;

#[tokio::test]
async fn integration_test_storage_interface_lmdb_basic_operations() {
    test_storage_interface_lmdb_basic_operations().await;
}

#[tokio::test]
async fn integration_test_storage_interface_rocksdb_basic_operations() {
    test_storage_interface_rocksdb_basic_operations().await;
}

#[tokio::test]
async fn integration_test_storage_interface_lmdb_concurrent_access() {
    test_storage_interface_lmdb_concurrent_access().await;
}

#[tokio::test]
async fn integration_test_storage_interface_rocksdb_concurrent_access() {
    test_storage_interface_rocksdb_concurrent_access().await;
}

#[tokio::test]
async fn integration_test_storage_interface_lmdb_batch_operations() {
    test_storage_interface_lmdb_batch_operations().await;
}

#[tokio::test]
async fn integration_test_storage_interface_rocksdb_batch_operations() {
    test_storage_interface_rocksdb_batch_operations().await;
}

#[tokio::test]
async fn integration_test_storage_interface_lmdb_cardano_patterns() {
    test_storage_interface_lmdb_cardano_patterns().await;
}

#[tokio::test]
async fn integration_test_storage_interface_rocksdb_cardano_patterns() {
    test_storage_interface_rocksdb_cardano_patterns().await;
}

#[tokio::test]
async fn integration_test_storage_interface_lmdb_error_handling() {
    test_storage_interface_lmdb_error_handling().await;
}

#[tokio::test]
async fn integration_test_storage_interface_rocksdb_error_handling() {
    test_storage_interface_rocksdb_error_handling().await;
}

#[tokio::test]
async fn integration_test_storage_interface_backend_statistics() {
    test_storage_interface_backend_statistics().await;
}

#[tokio::test]
async fn integration_test_storage_interface_haskell_compatibility() {
    test_storage_interface_haskell_compatibility().await;
}
