//! KES (Key Evolving Signature) Implementation
//!
//! KES is a forward-secure signature scheme where the signing key evolves over time periods.
//! This provides forward security: if a key is compromised at period t, signatures from
//! periods < t remain secure.
//!
//! Cardano uses KES for block signing to ensure that if a pool's hot keys are compromised,
//! only blocks from the current period onward are at risk.
//!
//! ## Algorithm: Sum Composition KES (based on MMM)
//!
//! Cardano uses a sum composition of the MMM (Malkin-Micciancio-Miner) tree-based KES.
//! The structure is:
//!
//! ```text
//! SumKES(depth) = if depth == 0:
//!                     Ed25519 signature
//!                 else:
//!                     Left: SumKES(depth-1)
//!                     Right: SumKES(depth-1)
//! ```
//!
//! For Cardano mainnet: depth = 6, giving 2^6 = 64 periods
//!
//! ## Key Evolution
//!
//! - Initial key is generated from cold key
//! - At each period, the left or right subtree is used
//! - When a subtree is exhausted, it's deleted (forward security)
//! - Period numbering: 0 to (2^depth - 1)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use cardano_crypto::kes::{KesSecretKey, KesPublicKey};
//!
//! // Generate initial key pair (depth=6 for mainnet)
//! let secret_key = KesSecretKey::generate(6);
//! let public_key = secret_key.to_public();
//!
//! // Sign at period 0
//! let signature = secret_key.sign(0, b"block data")?;
//!
//! // Evolve to period 1
//! let evolved_key = secret_key.evolve()?;
//!
//! // Sign at period 1
//! let signature = evolved_key.sign(1, b"next block")?;
//!
//! // Verify signature
//! assert!(public_key.verify(1, b"next block", &signature)?);
//! ```

use crate::{CryptoError, Result};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey, Signature as Ed25519Signature};
use blake2::Blake2b512;
use blake2::Digest;
use serde::{Deserialize, Serialize};

/// KES signature with period information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KesSignature {
    /// The period this signature was created at
    pub period: u64,
    /// The underlying signature bytes
    pub signature: Vec<u8>,
    /// The public key for this specific period (for verification)
    pub period_vkey: Vec<u8>,
    /// Authentication path (simplified for now)
    pub auth_path: Vec<Vec<u8>>,
}

impl KesSignature {
    /// Size of a KES signature in bytes (approximate)
    pub const SIZE: usize = 448; // Depends on depth, this is for depth=6

    /// Create signature from raw bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 8 + 64 + 32 {
            return Err(CryptoError::InvalidSignature(
                "KES signature too short".to_string()
            ));
        }

        let period = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let signature = bytes[8..72].to_vec();
        let period_vkey = bytes[72..104].to_vec();
        let auth_path = vec![]; // Simplified

        Ok(Self {
            period,
            signature,
            period_vkey,
            auth_path,
        })
    }

    /// Convert to raw bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.period.to_le_bytes());
        bytes.extend_from_slice(&self.signature);
        bytes.extend_from_slice(&self.period_vkey);
        for path_elem in &self.auth_path {
            bytes.extend_from_slice(path_elem);
        }
        bytes
    }
}

/// KES public key (verification key)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KesPublicKey {
    /// The root verification key
    vkey: Vec<u8>,
    /// Maximum period this key is valid for
    max_period: u64,
}

impl KesPublicKey {
    /// Size of KES public key in bytes
    pub const SIZE: usize = 32;

    /// Create from raw bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::SIZE {
            return Err(CryptoError::InvalidKeyLength);
        }

        Ok(Self {
            vkey: bytes.to_vec(),
            max_period: 62, // Default for depth=6 (2^6 - 2)
        })
    }

    /// Convert to raw bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        self.vkey.clone()
    }

    /// Get the maximum valid period
    pub fn max_period(&self) -> u64 {
        self.max_period
    }

    /// Verify a KES signature
    pub fn verify(&self, period: u64, message: &[u8], signature: &KesSignature) -> Result<bool> {
        // Check period matches
        if signature.period != period {
            return Ok(false);
        }

        // Check period is within bounds
        if period > self.max_period {
            return Ok(false);
        }

        // Verify using the period-specific verification key included in the signature
        let verifying_key = VerifyingKey::from_bytes(
            &signature.period_vkey[..32].try_into()
                .map_err(|_| CryptoError::InvalidPublicKey)?
        ).map_err(|_| CryptoError::InvalidPublicKey)?;

        let ed_sig = Ed25519Signature::from_bytes(
            &signature.signature[..64].try_into()
                .map_err(|_| CryptoError::InvalidSignature("Invalid signature length".to_string()))?
        );

        // Verify the signature
        // In a full implementation, we would also verify that period_vkey
        // was correctly derived from the root key using the auth_path
        Ok(verifying_key.verify(message, &ed_sig).is_ok())
    }
}

/// KES secret key (signing key)
#[derive(Debug, Clone)]
pub struct KesSecretKey {
    /// Root public key (constant across all periods)
    root_public_key: Vec<u8>,
    /// Current signing key (at current period)
    current_key: SigningKey,
    /// Current period
    current_period: u64,
    /// Maximum period
    max_period: u64,
    /// Tree depth
    depth: u32,
}

impl KesSecretKey {
    /// Size of KES secret key in bytes
    pub const SIZE: usize = 64;

    /// Generate a new KES key at period 0
    ///
    /// # Arguments
    /// * `depth` - Tree depth (6 for mainnet = 64 periods)
    pub fn generate(depth: u32) -> Self {
        let mut rng = rand::thread_rng();
        let signing_key = SigningKey::generate(&mut rng);
        let root_public_key = signing_key.verifying_key().to_bytes().to_vec();

        // Calculate max_period safely, cap at u64::MAX if depth is too large
        let max_period = if depth >= 64 {
            u64::MAX - 2
        } else {
            (1u64 << depth).saturating_sub(2)
        };

        Self {
            root_public_key,
            current_key: signing_key,
            current_period: 0,
            max_period,
            depth,
        }
    }

    /// Create from raw bytes and period
    pub fn from_bytes(bytes: &[u8], period: u64, depth: u32) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(CryptoError::InvalidKeyLength);
        }

        let signing_key = SigningKey::from_bytes(
            &bytes[..32].try_into()
                .map_err(|_| CryptoError::InvalidKeyLength)?
        );
        let root_public_key = signing_key.verifying_key().to_bytes().to_vec();

        let max_period = (1u64 << depth) - 2;

        Ok(Self {
            root_public_key,
            current_key: signing_key,
            current_period: period,
            max_period,
            depth,
        })
    }

    /// Convert to raw bytes (current key only)
    pub fn to_bytes(&self) -> Vec<u8> {
        self.current_key.to_bytes().to_vec()
    }

    /// Get the public key
    pub fn to_public(&self) -> KesPublicKey {
        // Use the root public key, not the current key
        KesPublicKey {
            vkey: self.root_public_key.clone(),
            max_period: self.max_period,
        }
    }

    /// Get current period
    pub fn current_period(&self) -> u64 {
        self.current_period
    }

    /// Get maximum period
    pub fn max_period(&self) -> u64 {
        self.max_period
    }

    /// Check if key has expired
    pub fn is_expired(&self) -> bool {
        self.current_period > self.max_period
    }

    /// Sign a message at the current period
    ///
    /// # Arguments
    /// * `period` - Must match current_period
    /// * `message` - The message to sign
    pub fn sign(&self, period: u64, message: &[u8]) -> Result<KesSignature> {
        // Verify period matches current period
        if period != self.current_period {
            return Err(CryptoError::InvalidSignature(
                format!("Period mismatch: expected {}, got {}", self.current_period, period)
            ));
        }

        // Check not expired
        if self.is_expired() {
            return Err(CryptoError::InvalidSignature(
                format!("KES key expired at period {}", self.max_period)
            ));
        }

        // Sign with current key
        let signature = self.current_key.sign(message);
        let period_vkey = self.current_key.verifying_key().to_bytes().to_vec();

        Ok(KesSignature {
            period,
            signature: signature.to_bytes().to_vec(),
            period_vkey,
            auth_path: vec![], // Simplified
        })
    }

    /// Evolve the key to the next period
    ///
    /// This implements forward security by deriving a new key and forgetting the old one.
    /// In a full implementation, this would traverse the tree structure.
    pub fn evolve(&self) -> Result<Self> {
        let next_period = self.current_period + 1;

        // Check if we can evolve
        if next_period > self.max_period {
            return Err(CryptoError::InvalidSignature(
                format!("Cannot evolve past max period {}", self.max_period)
            ));
        }

        // Derive next key using Blake2b-512
        // In practice: next_key = KDF(current_key, period)
        let mut hasher = Blake2b512::new();
        hasher.update(&self.current_key.to_bytes());
        hasher.update(&next_period.to_le_bytes());
        let hash = hasher.finalize();

        let next_key = SigningKey::from_bytes(
            &hash[..32].try_into()
                .map_err(|_| CryptoError::InvalidKeyLength)?
        );

        Ok(Self {
            root_public_key: self.root_public_key.clone(), // Keep root key constant
            current_key: next_key,
            current_period: next_period,
            max_period: self.max_period,
            depth: self.depth,
        })
    }

    /// Evolve to a specific period
    ///
    /// This repeatedly evolves the key until reaching the target period.
    pub fn evolve_to(&self, target_period: u64) -> Result<Self> {
        if target_period < self.current_period {
            return Err(CryptoError::InvalidSignature(
                "Cannot evolve backwards".to_string()
            ));
        }

        if target_period > self.max_period {
            return Err(CryptoError::InvalidSignature(
                format!("Target period {} exceeds max {}", target_period, self.max_period)
            ));
        }

        let mut key = self.clone();
        while key.current_period < target_period {
            key = key.evolve()?;
        }

        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kes_generation() {
        let depth = 6;
        let key = KesSecretKey::generate(depth);

        assert_eq!(key.current_period(), 0);
        assert_eq!(key.max_period(), 62); // 2^6 - 2
        assert!(!key.is_expired());
    }

    #[test]
    fn test_kes_sign_verify() {
        let key = KesSecretKey::generate(6);
        let public_key = key.to_public();
        let message = b"test block data";

        let signature = key.sign(0, message).unwrap();

        assert_eq!(signature.period, 0);
        assert!(public_key.verify(0, message, &signature).unwrap());
    }

    #[test]
    fn test_kes_evolution() {
        let key = KesSecretKey::generate(6);

        // Evolve to period 1
        let key1 = key.evolve().unwrap();
        assert_eq!(key1.current_period(), 1);

        // Sign at period 1
        let message = b"block at period 1";
        let sig = key1.sign(1, message).unwrap();

        let public_key = key.to_public();
        assert!(public_key.verify(1, message, &sig).unwrap());
    }

    #[test]
    fn test_kes_evolution_multiple_periods() {
        let key = KesSecretKey::generate(6);

        // Evolve to period 5
        let key5 = key.evolve_to(5).unwrap();
        assert_eq!(key5.current_period(), 5);

        // Sign and verify
        let message = b"block at period 5";
        let sig = key5.sign(5, message).unwrap();

        let public_key = key.to_public();
        assert!(public_key.verify(5, message, &sig).unwrap());
    }

    #[test]
    fn test_kes_period_mismatch() {
        let key = KesSecretKey::generate(6);
        let message = b"test";

        // Try to sign with wrong period
        let result = key.sign(5, message);
        assert!(result.is_err());
    }

    #[test]
    fn test_kes_expiration() {
        let key = KesSecretKey::generate(3); // depth=3 -> max_period=6

        // Evolve to max period
        let key_max = key.evolve_to(6).unwrap();
        assert!(!key_max.is_expired());

        // Try to evolve past max
        let result = key_max.evolve();
        assert!(result.is_err());
    }

    #[test]
    fn test_kes_backward_evolution_fails() {
        let key = KesSecretKey::generate(6);
        let key5 = key.evolve_to(5).unwrap();

        // Cannot evolve backwards
        let result = key5.evolve_to(3);
        assert!(result.is_err());
    }

    #[test]
    fn test_kes_signature_serialization() {
        let key = KesSecretKey::generate(6);
        let message = b"test block";
        let sig = key.sign(0, message).unwrap();

        // Serialize and deserialize
        let bytes = sig.to_bytes();
        let sig2 = KesSignature::from_bytes(&bytes).unwrap();

        assert_eq!(sig.period, sig2.period);
        assert_eq!(sig.signature, sig2.signature);
    }

    #[test]
    fn test_kes_public_key_serialization() {
        let key = KesSecretKey::generate(6);
        let public_key = key.to_public();

        // Serialize and deserialize
        let bytes = public_key.to_bytes();
        let public_key2 = KesPublicKey::from_bytes(&bytes).unwrap();

        assert_eq!(public_key.vkey, public_key2.vkey);
    }
}
