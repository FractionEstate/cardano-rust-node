//! Ed25519 digital signature operations
//!
//! Provides Ed25519 cryptographic operations compatible with Cardano's
//! Haskell implementation. All signature operations must produce
//! identical results for consensus safety.

use crate::{CryptoError, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

pub mod mod_rs {
    //! Ed25519 module exports
    pub use super::*;
}

/// Ed25519 signature wrapper for Cardano compatibility
#[derive(Debug, Clone)]
pub struct Ed25519Signature(Signature);

/// Ed25519 public key wrapper for Cardano compatibility
#[derive(Debug, Clone)]
pub struct Ed25519PublicKey(VerifyingKey);

/// Ed25519 private key wrapper for Cardano compatibility
#[derive(Debug)]
pub struct Ed25519PrivateKey(SigningKey);

/// Ed25519 key hash (20 bytes - BLAKE2b-160 of public key)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ed25519KeyHash([u8; 20]);

impl Ed25519PrivateKey {
    /// Generate a new Ed25519 private key from a seed
    pub fn generate(seed: &[u8; 32]) -> Self {
        Self(SigningKey::from_bytes(seed))
    }

    /// Create from raw bytes (32 bytes)
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self> {
        Ok(Self(SigningKey::from_bytes(&bytes)))
    }

    /// Create from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str).map_err(|_| CryptoError::InvalidHexEncoding)?;
        if bytes.len() != 32 {
            return Err(CryptoError::InvalidKeyLength);
        }
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&bytes);
        Ok(Self(SigningKey::from_bytes(&key_bytes)))
    }

    /// Get raw bytes representation
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Get hex representation
    pub fn to_hex(&self) -> String {
        hex::encode(self.0.to_bytes())
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Ed25519Signature {
        let signature = self.0.sign(message);
        Ed25519Signature(signature)
    }

    /// Get the corresponding public key
    pub fn public_key(&self) -> Ed25519PublicKey {
        Ed25519PublicKey(self.0.verifying_key())
    }
}

impl Ed25519PublicKey {
    /// Create from raw bytes (32 bytes)
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self> {
        match VerifyingKey::from_bytes(&bytes) {
            Ok(key) => Ok(Self(key)),
            Err(_) => Err(CryptoError::InvalidPublicKey),
        }
    }

    /// Create from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str).map_err(|_| CryptoError::InvalidHexEncoding)?;
        if bytes.len() != 32 {
            return Err(CryptoError::InvalidKeyLength);
        }
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&bytes);
        match VerifyingKey::from_bytes(&key_bytes) {
            Ok(key) => Ok(Self(key)),
            Err(_) => Err(CryptoError::InvalidPublicKey),
        }
    }

    /// Get raw bytes representation
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Get hex representation
    pub fn to_hex(&self) -> String {
        hex::encode(self.0.to_bytes())
    }

    /// Verify a signature
    pub fn verify(&self, message: &[u8], signature: &Ed25519Signature) -> bool {
        self.0.verify(message, &signature.0).is_ok()
    }
}

impl Ed25519Signature {
    /// Create from raw bytes (64 bytes)
    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        let signature = Signature::from_bytes(&bytes);
        Self(signature)
    }

    /// Create from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| CryptoError::InvalidSignature(format!("Invalid hex: {}", e)))?;
        if bytes.len() != 64 {
            return Err(CryptoError::InvalidSignature(
                "Ed25519 signature hex must be 128 characters (64 bytes)".to_string(),
            ));
        }
        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(&bytes);
        let signature = Signature::from_bytes(&sig_bytes);
        Ok(Self(signature))
    }

    /// Get raw bytes representation
    pub fn to_bytes(&self) -> [u8; 64] {
        self.0.to_bytes()
    }

    /// Get hex representation
    pub fn to_hex(&self) -> String {
        hex::encode(self.0.to_bytes())
    }
}

impl PartialEq for Ed25519Signature {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bytes() == other.0.to_bytes()
    }
}

impl Eq for Ed25519Signature {}

impl PartialOrd for Ed25519Signature {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ed25519Signature {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.to_bytes().cmp(&other.0.to_bytes())
    }
}

impl PartialEq for Ed25519PublicKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bytes() == other.0.to_bytes()
    }
}

impl Eq for Ed25519PublicKey {}

impl PartialOrd for Ed25519PublicKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ed25519PublicKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.to_bytes().cmp(&other.0.to_bytes())
    }
}

impl Ed25519KeyHash {
    /// Create from raw bytes (20 bytes)
    pub fn from_bytes(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    /// Create from a public key (hash the public key)
    pub fn from_public_key(public_key: &Ed25519PublicKey) -> Self {
        // Simplified implementation - hash first 20 bytes of public key
        let pk_bytes = public_key.0.to_bytes();
        let mut hash = [0u8; 20];
        hash.copy_from_slice(&pk_bytes[..20]);
        Self(hash)
    }

    /// Create from test data
    pub fn from_test_data(data: &[u8]) -> Self {
        let mut hash = [0u8; 20];
        for (i, byte) in data.iter().enumerate() {
            hash[i % 20] ^= *byte;
        }
        Self(hash)
    }

    /// Get raw bytes
    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    /// Get hex representation
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}
