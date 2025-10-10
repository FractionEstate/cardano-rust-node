//! Integration tests for T067: Storage Interface Compatibility Tests
//!
//! This test module validates storage interface compatibility across different backends.
//! Tests ensure LMDB and CardanoDB backends work correctly with Cardano-specific patterns.

use super::test_storage_interface::*;

#[tokio::test]
async fn integration_test_storage_interface_lmdb_basic_operations() {
    test_storage_interface_lmdb_basic_operations().await;
}

#[tokio::test]
async fn integration_test_storage_interface_cardanodb_basic_operations() {
    test_storage_interface_cardanodb_basic_operations().await;
}

#[tokio::test]
async fn integration_test_storage_interface_lmdb_concurrent_access() {
    test_storage_interface_lmdb_concurrent_access().await;
}

#[tokio::test]
async fn integration_test_storage_interface_cardanodb_concurrent_access() {
    test_storage_interface_cardanodb_concurrent_access().await;
}

#[tokio::test]
async fn integration_test_storage_interface_lmdb_batch_operations() {
    test_storage_interface_lmdb_batch_operations().await;
}

#[tokio::test]
async fn integration_test_storage_interface_cardanodb_batch_operations() {
    test_storage_interface_cardanodb_batch_operations().await;
}

#[tokio::test]
async fn integration_test_storage_interface_lmdb_cardano_patterns() {
    test_storage_interface_lmdb_cardano_patterns().await;
}

#[tokio::test]
async fn integration_test_storage_interface_cardanodb_cardano_patterns() {
    test_storage_interface_cardanodb_cardano_patterns().await;
}

#[tokio::test]
async fn integration_test_storage_interface_lmdb_error_handling() {
    test_storage_interface_lmdb_error_handling().await;
}

#[tokio::test]
async fn integration_test_storage_interface_cardanodb_error_handling() {
    test_storage_interface_cardanodb_error_handling().await;
}

#[tokio::test]
async fn integration_test_storage_interface_backend_statistics() {
    test_storage_interface_backend_statistics().await;
}

#[tokio::test]
async fn integration_test_storage_interface_haskell_compatibility() {
    test_storage_interface_haskell_compatibility().await;
}
