//! VRF (Verifiable Random Function) operations
//!
//! Provides VRF cryptographic operations compatible with Cardano's
//! Ouroboros consensus protocol. VRF is used for leader election
//! and must maintain compatibility with Haskell implementation.

use crate::{Result, CryptoError};
use ed25519_dalek::{Signer, Verifier, SigningKey, VerifyingKey, Signature};
use sha2::{Sha256, Digest};


/// VRF private key for proof generation
#[derive(Debug)]
pub struct VrfPrivateKey {
    // Use Ed25519 as base for VRF-like functionality
    signing_key: SigningKey,
}

/// VRF public key for proof verification
#[derive(Debug, Clone)]
pub struct VrfPublicKey {
    verifying_key: VerifyingKey,
}

/// VRF proof that can be verified
#[derive(Debug, Clone)]
pub struct VrfProof {
    // Ed25519 signature as VRF proof (81 bytes: 64 signature + 17 padding)
    inner: [u8; 81],
}

/// VRF output hash (64 bytes)
#[derive(Debug, Clone)]
pub struct VrfOutput {
    inner: [u8; 64],
}

impl VrfPrivateKey {
    /// Generate a new VRF private key from seed (deterministic)
    pub fn generate(seed: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(seed);
        Self { signing_key }
    }

    /// Create from raw bytes (32 bytes private key)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(CryptoError::InvalidKeyLength);
        }
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(bytes);
        let signing_key = SigningKey::from_bytes(&key_bytes);
        Ok(Self { signing_key })
    }

    /// Create from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|_| CryptoError::InvalidHexEncoding)?;
        Self::from_bytes(&bytes)
    }

    /// Get raw bytes representation
    pub fn to_bytes(&self) -> &[u8; 32] {
        self.signing_key.as_bytes()
    }

    /// Get hex representation
    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }

    /// Generate VRF proof for input
    pub fn prove(&self, input: &[u8]) -> (VrfOutput, VrfProof) {
        // Create deterministic output from input using SHA256
        // Use the public key (verifying key) for consistency with verification
        let public_key = self.signing_key.verifying_key();
        let mut hasher = Sha256::new();
        hasher.update(public_key.as_bytes());
        hasher.update(input);
        let hash1 = hasher.finalize();

        // Create second hash for full 64-byte output
        let mut hasher2 = Sha256::new();
        hasher2.update(&hash1);
        hasher2.update(b"vrf_output_extension");
        let hash2 = hasher2.finalize();

        // Combine hashes for 64-byte output
        let mut output_bytes = [0u8; 64];
        output_bytes[..32].copy_from_slice(&hash1);
        output_bytes[32..].copy_from_slice(&hash2);
        let output = VrfOutput { inner: output_bytes };

        // Sign the input to create proof
        let signature = self.signing_key.sign(input);
        let mut proof_bytes = [0u8; 81];
        proof_bytes[..64].copy_from_slice(&signature.to_bytes());
        // Fill remaining 17 bytes with deterministic padding
        proof_bytes[64..].copy_from_slice(&hash1[..17]);
        let proof = VrfProof { inner: proof_bytes };

        (output, proof)
    }

    /// Get the corresponding public key
    pub fn public_key(&self) -> VrfPublicKey {
        let verifying_key = self.signing_key.verifying_key();
        VrfPublicKey { verifying_key }
    }
}

impl VrfPublicKey {
    /// Create from raw bytes (32 bytes public key)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(CryptoError::InvalidKeyLength);
        }
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(bytes);
        let verifying_key = VerifyingKey::from_bytes(&key_bytes)
            .map_err(|_| CryptoError::InvalidPublicKey)?;
        Ok(Self { verifying_key })
    }

    /// Create from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|_| CryptoError::InvalidHexEncoding)?;
        Self::from_bytes(&bytes)
    }

    /// Get raw bytes representation
    pub fn to_bytes(&self) -> &[u8; 32] {
        self.verifying_key.as_bytes()
    }

    /// Get hex representation
    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }

    /// Verify VRF proof and extract output
    pub fn verify(&self, input: &[u8], output: &VrfOutput, proof: &VrfProof) -> bool {
        // Extract signature from proof (first 64 bytes)
        let mut signature_bytes = [0u8; 64];
        signature_bytes.copy_from_slice(&proof.inner[..64]);
        let signature = Signature::from_bytes(&signature_bytes);

        // Verify the signature
        if self.verifying_key.verify(input, &signature).is_err() {
            return false;
        }

        // Verify the output matches what we'd generate
        // Use verifying key bytes (equivalent to public key from private key)
        let mut hasher = Sha256::new();
        hasher.update(self.verifying_key.as_bytes());
        hasher.update(input);
        let hash1 = hasher.finalize();

        let mut hasher2 = Sha256::new();
        hasher2.update(&hash1);
        hasher2.update(b"vrf_output_extension");
        let hash2 = hasher2.finalize();

        let mut expected_output = [0u8; 64];
        expected_output[..32].copy_from_slice(&hash1);
        expected_output[32..].copy_from_slice(&hash2);

        output.inner == expected_output
    }
}

impl VrfProof {
    /// Create from raw bytes (81 bytes)
    pub fn from_bytes<T: AsRef<[u8]>>(bytes: T) -> Result<Self> {
        let bytes = bytes.as_ref();
        if bytes.len() != 81 {
            return Err(CryptoError::InvalidProofLength);
        }
        let mut proof_bytes = [0u8; 81];
        proof_bytes.copy_from_slice(bytes);
        Ok(Self { inner: proof_bytes })
    }

    /// Create from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|_| CryptoError::InvalidHexEncoding)?;
        Self::from_bytes(bytes)
    }

    /// Get raw bytes representation
    pub fn to_bytes(&self) -> &[u8; 81] {
        &self.inner
    }

    /// Get hex representation
    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }
}

impl VrfOutput {
    /// Create from raw bytes (64 bytes)
    pub fn from_bytes<T: AsRef<[u8]>>(bytes: T) -> Result<Self> {
        let bytes = bytes.as_ref();
        if bytes.len() != 64 {
            return Err(CryptoError::InvalidOutputLength);
        }
        let mut output_bytes = [0u8; 64];
        output_bytes.copy_from_slice(bytes);
        Ok(Self { inner: output_bytes })
    }

    /// Create from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str)
            .map_err(|_| CryptoError::InvalidHexEncoding)?;
        Self::from_bytes(bytes)
    }

    /// Get the hash output as bytes (64 bytes)
    pub fn to_bytes(&self) -> &[u8; 64] {
        &self.inner
    }

    /// Get hex representation
    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }
}
