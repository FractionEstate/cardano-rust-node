//! Ed25519 digital signature operations
//!
//! Provides Ed25519 cryptographic operations compatible with Cardano's
//! Haskell implementation using cardano-base-rust. All signature operations
//! produce identical results to the Haskell node for consensus safety.

use crate::{CryptoError, Result};
use cardano_crypto_class::dsign::ed25519::{
    Ed25519, Ed25519Signature as CardanoEd25519Signature, Ed25519SigningKey as CardanoSigningKey,
    Ed25519VerificationKey as CardanoVerificationKey,
};
use cardano_crypto_class::dsign::DsignAlgorithm;

pub mod mod_rs {
    //! Ed25519 module exports
    pub use super::*;
}

/// Ed25519 signature wrapper for Cardano compatibility
///
/// Uses cardano-crypto-class Ed25519Signature internally (64 bytes, pinned memory)
#[derive(Debug, Clone)]
pub struct Ed25519Signature(CardanoEd25519Signature);

/// Ed25519 public key wrapper for Cardano compatibility
///
/// Uses cardano-crypto-class Ed25519VerificationKey internally (32 bytes, pinned memory)
#[derive(Debug, Clone)]
pub struct Ed25519PublicKey(CardanoVerificationKey);

/// Ed25519 private key wrapper for Cardano compatibility
///
/// Uses cardano-crypto-class Ed25519SigningKey internally (64-byte compound key with seed + public key)
#[derive(Debug)]
pub struct Ed25519PrivateKey(CardanoSigningKey);

/// Ed25519 key hash (20 bytes - BLAKE2b-160 of public key)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ed25519KeyHash([u8; 20]);

impl Ed25519PrivateKey {
    /// Generate a new Ed25519 private key from a seed (32 bytes)
    ///
    /// Uses cardano-crypto-class DsignAlgorithm to generate keys compatible with Haskell node
    pub fn generate(seed: &[u8; 32]) -> Self {
        let signing_key = Ed25519::gen_key_from_seed_bytes(seed);
        Self(signing_key)
    }

    /// Create from raw seed bytes (32 bytes)
    ///
    /// Note: cardano-crypto-class uses 32-byte seeds, not 64-byte compound keys
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self> {
        let signing_key = Ed25519::gen_key_from_seed_bytes(&bytes);
        Ok(Self(signing_key))
    }

    /// Create from hex string (must be 64 hex characters = 32 bytes)
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str).map_err(|_| CryptoError::InvalidHexEncoding)?;
        if bytes.len() != 32 {
            return Err(CryptoError::InvalidKeyLength);
        }
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&bytes);
        let signing_key = Ed25519::gen_key_from_seed_bytes(&key_bytes);
        Ok(Self(signing_key))
    }

    /// Get raw seed bytes (32 bytes, NOT the 64-byte compound key)
    pub fn to_bytes(&self) -> [u8; 32] {
        let vec = Ed25519::raw_serialize_signing_key(&self.0);
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&vec);
        bytes
    }

    /// Get hex representation of seed (64 hex characters)
    pub fn to_hex(&self) -> String {
        let bytes = Ed25519::raw_serialize_signing_key(&self.0);
        hex::encode(bytes)
    }

    /// Sign a message using cardano-crypto-class DsignAlgorithm
    ///
    /// Produces signatures identical to the Haskell node
    pub fn sign(&self, message: &[u8]) -> Ed25519Signature {
        let context = (); // Ed25519 doesn't use context
        let signature = Ed25519::sign_bytes(&context, message, &self.0);
        Ed25519Signature(signature)
    }

    /// Get the corresponding public key
    ///
    /// Uses cardano-crypto-class to derive verification key from signing key
    pub fn public_key(&self) -> Ed25519PublicKey {
        let vk = Ed25519::derive_verification_key(&self.0);
        Ed25519PublicKey(vk)
    }
}

impl Ed25519PublicKey {
    /// Create from raw bytes (32 bytes)
    ///
    /// Uses cardano-crypto-class to deserialize verification key
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self> {
        match Ed25519::raw_deserialize_verification_key(&bytes) {
            Some(key) => Ok(Self(key)),
            None => Err(CryptoError::InvalidPublicKey),
        }
    }

    /// Create from hex string (must be 64 hex characters = 32 bytes)
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str).map_err(|_| CryptoError::InvalidHexEncoding)?;
        if bytes.len() != 32 {
            return Err(CryptoError::InvalidKeyLength);
        }
        match Ed25519::raw_deserialize_verification_key(&bytes) {
            Some(key) => Ok(Self(key)),
            None => Err(CryptoError::InvalidPublicKey),
        }
    }

    /// Get raw bytes representation (32 bytes)
    pub fn to_bytes(&self) -> [u8; 32] {
        let vec = Ed25519::raw_serialize_verification_key(&self.0);
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&vec);
        bytes
    }

    /// Get hex representation (64 hex characters)
    pub fn to_hex(&self) -> String {
        let bytes = Ed25519::raw_serialize_verification_key(&self.0);
        hex::encode(bytes)
    }

    /// Verify a signature using cardano-crypto-class DsignAlgorithm
    ///
    /// Produces verification results identical to the Haskell node
    pub fn verify(&self, message: &[u8], signature: &Ed25519Signature) -> bool {
        let context = (); // Ed25519 doesn't use context
        Ed25519::verify_bytes(&context, &self.0, message, &signature.0).is_ok()
    }
}

impl Ed25519Signature {
    /// Create from raw bytes (64 bytes)
    ///
    /// Uses cardano-crypto-class to deserialize signature
    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        // Note: from_bytes in cardano-crypto-class is infallible, it just wraps the bytes
        match Ed25519::raw_deserialize_signature(&bytes) {
            Some(sig) => Self(sig),
            None => panic!("Invalid Ed25519 signature bytes"), // Should never happen with 64 bytes
        }
    }

    /// Create from hex string (must be 128 hex characters = 64 bytes)
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| CryptoError::InvalidSignature(format!("Invalid hex: {e}")))?;
        if bytes.len() != 64 {
            return Err(CryptoError::InvalidSignature(
                "Ed25519 signature hex must be 128 characters (64 bytes)".to_string(),
            ));
        }
        match Ed25519::raw_deserialize_signature(&bytes) {
            Some(sig) => Ok(Self(sig)),
            None => Err(CryptoError::InvalidSignature(
                "Failed to deserialize Ed25519 signature".to_string(),
            )),
        }
    }

    /// Get raw bytes representation (64 bytes)
    pub fn to_bytes(&self) -> [u8; 64] {
        let vec = Ed25519::raw_serialize_signature(&self.0);
        let mut bytes = [0u8; 64];
        bytes.copy_from_slice(&vec);
        bytes
    }

    /// Get hex representation (128 hex characters)
    pub fn to_hex(&self) -> String {
        let bytes = Ed25519::raw_serialize_signature(&self.0);
        hex::encode(bytes)
    }
}

impl PartialEq for Ed25519Signature {
    fn eq(&self, other: &Self) -> bool {
        let self_bytes = Ed25519::raw_serialize_signature(&self.0);
        let other_bytes = Ed25519::raw_serialize_signature(&other.0);
        self_bytes == other_bytes
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
        let self_bytes = Ed25519::raw_serialize_signature(&self.0);
        let other_bytes = Ed25519::raw_serialize_signature(&other.0);
        self_bytes.cmp(&other_bytes)
    }
}

// Serde support for Ed25519Signature
impl serde::Serialize for Ed25519Signature {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let bytes = self.to_bytes();
        serializer.serialize_bytes(&bytes)
    }
}

impl<'de> serde::Deserialize<'de> for Ed25519Signature {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = serde::Deserialize::deserialize(deserializer)?;
        if bytes.len() != 64 {
            return Err(serde::de::Error::custom(format!(
                "Invalid signature length: expected 64, got {}",
                bytes.len()
            )));
        }
        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(&bytes);
        Ok(Self::from_bytes(sig_bytes))
    }
}

impl PartialEq for Ed25519PublicKey {
    fn eq(&self, other: &Self) -> bool {
        let self_bytes = Ed25519::raw_serialize_verification_key(&self.0);
        let other_bytes = Ed25519::raw_serialize_verification_key(&other.0);
        self_bytes == other_bytes
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
        let self_bytes = Ed25519::raw_serialize_verification_key(&self.0);
        let other_bytes = Ed25519::raw_serialize_verification_key(&other.0);
        self_bytes.cmp(&other_bytes)
    }
}

impl Ed25519KeyHash {
    /// Create from raw bytes (20 bytes)
    pub fn from_bytes(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    /// Create from a public key (hash the public key with BLAKE2b-160)
    pub fn from_public_key(public_key: &Ed25519PublicKey) -> Self {
        let pk_bytes = Ed25519::raw_serialize_verification_key(&public_key.0);
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
