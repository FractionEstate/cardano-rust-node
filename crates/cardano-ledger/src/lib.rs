//! Cardano Ledger Rules Implementation
//!
//! This crate implements the ledger rules for different Cardano eras,
//! including transaction validation, UTxO management, and state transitions.

use std::error::Error;
use std::fmt;

/// Result type for ledger operations
pub type Result<T> = std::result::Result<T, LedgerError>;

/// Coin amount type (lovelace)
pub type Coin = u64;

/// Epoch number
pub type Epoch = u64;

/// Slot number
pub type Slot = u64;

/// Ledger errors
#[derive(Debug, Clone)]
pub enum LedgerError {
    InvalidTransaction(String),
    InvalidInput(String),
    InvalidOutput(String),
    InvalidCertificate(String),
    InsufficientFunds(String),
    TransactionTooLarge(String),
    ValueOverflow(String),
    ValueUnderflow(String),
    ValidationError(String),
    InvalidBlock(String),
    InvalidSlot(String),
    ScriptError(String),
    SerializationError(String),
    GovernanceError(String),
    CryptoError(String),
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LedgerError::InvalidTransaction(msg) => write!(f, "Invalid transaction: {}", msg),
            LedgerError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            LedgerError::InvalidOutput(msg) => write!(f, "Invalid output: {}", msg),
            LedgerError::InvalidCertificate(msg) => write!(f, "Invalid certificate: {}", msg),
            LedgerError::InsufficientFunds(msg) => write!(f, "Insufficient funds: {}", msg),
            LedgerError::TransactionTooLarge(msg) => write!(f, "Transaction too large: {}", msg),
            LedgerError::ValueOverflow(msg) => write!(f, "Value overflow: {}", msg),
            LedgerError::ValueUnderflow(msg) => write!(f, "Value underflow: {}", msg),
            LedgerError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            LedgerError::InvalidBlock(msg) => write!(f, "Invalid block: {}", msg),
            LedgerError::InvalidSlot(msg) => write!(f, "Invalid slot: {}", msg),
            LedgerError::ScriptError(msg) => write!(f, "Script error: {}", msg),
            LedgerError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            LedgerError::GovernanceError(msg) => write!(f, "Governance error: {}", msg),
            LedgerError::CryptoError(msg) => write!(f, "Crypto error: {}", msg),
        }
    }
}

impl Error for LedgerError {}

/// Byron era implementation
pub mod byron;

/// Shelley era implementation
pub mod shelley;

/// Allegra era implementation
pub mod allegra;

/// Mary era implementation (multi-asset support)
pub mod mary;

/// Alonzo era implementation (Plutus V1 smart contracts)
pub mod alonzo;

/// Babbage era implementation (Plutus V2, reference inputs)
pub mod babbage;

/// Conway era implementation (on-chain governance)
pub mod conway;

/// Fee optimization and UTxO selection
pub mod fee_optimization;
