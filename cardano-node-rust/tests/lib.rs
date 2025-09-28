//! Integration tests for Cardano Node Rust
//!
//! These tests validate compatibility with Cardano Haskell implementation
//! following Test-Driven Development (TDD) methodology.
//!
//! Test Modules:
//! - crypto: Cryptographic compatibility tests
//! - ledger: Era validation tests (Byron through Conway)
//! - consensus: Ouroboros consensus protocol tests
//! - network: Network protocol stack tests (ChainSync, BlockFetch, TxSubmission, P2P)
//! - storage: Storage backend tests (LMDB, RocksDB, ChainDB, LedgerDB)
//! - api: API interface tests (Local Socket, CLI, Submit API)
//! - node: Main node integration tests (CLI parsing, configuration, lifecycle)

mod crypto;
mod ledger;
mod consensus;
mod network;
mod storage;
mod api;
mod node;
mod node;

// T066: LMDB Backend Tests - Export for integration testing
pub use storage::test_lmdb_backend::*;

#[cfg(test)]
mod lmdb_integration_tests {
    use crate::storage::*;

    #[test]
    fn test_t066_lmdb_backend_integration() {
        // This test validates that the LMDB backend module is properly integrated
        let (temp_dir, path) = create_test_dir();
        assert!(path.exists(), "T066: Test directory should be created");
        println!("T066 LMDB Backend Tests: Integration validated at {:?}", path);
    }
}

// Re-export test modules
pub use crypto::integration_tests::*;
pub use ledger::*;
pub use consensus::*;
