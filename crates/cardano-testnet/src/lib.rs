//! Cardano Testnet Utilities
//!
//! Testing utilities and frameworks for Cardano Node development.

/// Testnet error types
#[derive(Debug, thiserror::Error)]
pub enum TestnetError {
    #[error("Test setup error: {0}")]
    SetupError(String),

    #[error("Test execution error: {0}")]
    ExecutionError(String),
}

pub type Result<T> = std::result::Result<T, TestnetError>;
