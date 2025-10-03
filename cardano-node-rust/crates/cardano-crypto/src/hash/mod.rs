//! Hash Functions
//!
//! Cryptographic hash functions used throughout Cardano

use crate::{CryptoError, Result};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

/// Blake2b 256-bit hash
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Copy,
    PartialOrd,
    Ord,
    Default,
    minicbor::Encode,
    minicbor::Decode,
)]
pub struct Blake2b256Hash(#[n(0)] [u8; 32]);

/// Blake2b 512-bit hash
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub struct Blake2b512Hash([u8; 64]);

/// SHA256 hash
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Copy)]
pub struct Sha256Hash([u8; 32]);

/// SHA3-256 hash
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Copy)]
pub struct Sha3_256Hash([u8; 32]);

/// Keccak256 hash
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Copy)]
pub struct Keccak256Hash([u8; 32]);

impl Hash for Blake2b256Hash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Hash for Blake2b512Hash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Hash for Sha256Hash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Hash for Sha3_256Hash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Hash for Keccak256Hash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl AsRef<[u8]> for Blake2b256Hash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Blake2b256Hash {
    /// Hash input data with Blake2b-256 (simplified implementation)
    pub fn hash(input: &[u8]) -> Self {
        // Simplified hash - in real implementation would use blake2 crate
        let mut hash = [0u8; 32];
        for (i, byte) in input.iter().enumerate() {
            hash[i % 32] ^= byte;
        }
        Self(hash)
    }

    /// Create hash from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(CryptoError::HashError(format!(
                "Invalid Blake2b256 hash length: expected 32, got {}",
                bytes.len()
            )));
        }
        let mut hash = [0u8; 32];
        hash.copy_from_slice(bytes);
        Ok(Self(hash))
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Sha256Hash {
    /// Hash input data with SHA256 (simplified implementation)
    pub fn hash(input: &[u8]) -> Self {
        // Simplified hash - in real implementation would use sha2 crate
        let mut hash = [0u8; 32];
        for (i, byte) in input.iter().enumerate() {
            hash[i % 32] ^= byte.wrapping_add(i as u8);
        }
        Self(hash)
    }

    /// Create hash from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(CryptoError::HashError(format!(
                "Invalid SHA256 hash length: expected 32, got {}",
                bytes.len()
            )));
        }
        let mut hash = [0u8; 32];
        hash.copy_from_slice(bytes);
        Ok(Self(hash))
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Blake2b512Hash {
    /// Hash input data with Blake2b-512 (simplified implementation)
    pub fn hash(input: &[u8]) -> Self {
        // Simplified hash - in real implementation would use blake2 crate
        let mut hash = [0u8; 64];
        for (i, byte) in input.iter().enumerate() {
            hash[i % 64] ^= byte;
        }
        Self(hash)
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 64 {
            return Err(CryptoError::HashError(
                "BLAKE2b-512 hash must be 64 bytes".to_string(),
            ));
        }
        let mut hash = [0u8; 64];
        hash.copy_from_slice(bytes);
        Ok(Self(hash))
    }

    /// Get raw bytes representation
    pub fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }
}

// Cardano-specific hash functions

/// Hash used for block headers (BLAKE2b-256)
pub type BlockHeaderHash = Blake2b256Hash;

/// Hash used for transaction IDs (BLAKE2b-256)
pub type TransactionHash = Blake2b256Hash;

/// Hash used for script validation (BLAKE2b-256)
pub type ScriptHash = Blake2b256Hash;
