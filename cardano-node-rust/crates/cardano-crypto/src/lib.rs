//! Cardano Cryptographic Operations
//!
//! This crate provides cryptographic primitives used in Cardano:
//! - Ed25519 digital signatures
//! - VRF (Verifiable Random Functions)
//! - BLAKE2b and SHA256 hashing
//! - BLS12-381 operations
//!
//! All implementations must maintain byte-for-byte compatibility
//! with the Haskell Cardano Node.

pub mod ed25519;
pub mod vrf;
pub mod hash;
pub mod bls;

pub use ed25519::*;
pub use vrf::*;
pub use hash::*;
pub use bls::*;

/// Cardano-specific error types for cryptographic operations
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("Invalid public key")]
    InvalidPublicKey,

    #[error("Invalid VRF proof: {0}")]
    InvalidVrfProof(String),

    #[error("Hash operation failed: {0}")]
    HashError(String),

    #[error("BLS operation failed: {0}")]
    BlsError(String),

    #[error("Invalid key length")]
    InvalidKeyLength,

    #[error("Invalid hex encoding")]
    InvalidHexEncoding,

    #[error("Invalid proof length")]
    InvalidProofLength,

    #[error("Invalid output length")]
    InvalidOutputLength,
}

pub type Result<T> = std::result::Result<T, CryptoError>;
