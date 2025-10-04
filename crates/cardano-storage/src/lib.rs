//! Cardano Storage Layer
//!
//! Provides persistent storage for blockchain data and ledger state.
//!
//! # Storage Backends
//!
//! ## Legacy Backends (to be phased out)
//! - LMDB and RocksDB wrappers (C/C++ dependencies)
//!
//! ## CardanoDB (NEW - Pure Rust)
//! - ImmutableDB: Chunk-based storage for ancient blocks
//! - VolatileDB: Ring buffer for recent blocks
//! - LedgerDB: In-memory ledger with snapshots
//!
//! See the `cardanodb` module for the new pure-Rust storage engine.

pub mod backends;
pub mod cardanodb; // NEW: Pure Rust storage engine
pub mod chaindb;
pub mod ledgerdb;

/// Storage error types
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, StorageError>;

// Re-export commonly used types
#[cfg(feature = "legacy")]
pub use backends::{
    BackendStats, BatchOperation, LmdbBackend, LmdbConfig, RocksDbBackend, RocksDbConfig,
    StorageBackend,
};
pub use chaindb::{ChainDatabase, ChainDatabaseImpl, ChainDatabaseStats, ChainMetadata};
pub use ledgerdb::{
    EpochInfo, LedgerDatabase, LedgerDatabaseImpl, LedgerDatabaseStats, PoolParameters,
};

// T066: LMDB Backend Tests Module
#[cfg(test)]
mod lmdb_tests;
