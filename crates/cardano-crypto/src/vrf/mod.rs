//! VRF (Verifiable Random Function) operations
//!
//! Provides VRF cryptographic operations compatible with Cardano's
//! Ouroboros consensus protocol. VRF is used for leader election
//! and must maintain compatibility with Haskell implementation.

mod backend;

use crate::{CryptoError, Result};
use once_cell::sync::Lazy;
use rand_core::{OsRng, RngCore};
use std::fmt;
use zeroize::Zeroize;

pub const VRF_SEED_LENGTH: usize = 32;
pub const VRF_PRIVATE_KEY_LENGTH: usize = 64;
pub const VRF_PUBLIC_KEY_LENGTH: usize = 32;
pub const VRF_PROOF_LENGTH: usize = 128; // Updated to Draft-13 batch-compatible (was 80 for Draft-03)
pub const VRF_BATCH_PROOF_LENGTH: usize = 128; // Now matches VRF_PROOF_LENGTH
pub const VRF_OUTPUT_LENGTH: usize = 64;

static PUBLIC_KEY_BYTES: Lazy<usize> = Lazy::new(backend::vrf_public_key_bytes);
static SECRET_KEY_BYTES: Lazy<usize> = Lazy::new(backend::vrf_secret_key_bytes);
static PROOF_BYTES: Lazy<usize> = Lazy::new(backend::vrf_proof_bytes);
static OUTPUT_BYTES: Lazy<usize> = Lazy::new(backend::vrf_output_bytes);
static SEED_BYTES: Lazy<usize> = Lazy::new(backend::vrf_seed_bytes);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VrfOutput {
    inner: [u8; VRF_OUTPUT_LENGTH],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VrfProof {
    inner: [u8; 128], // Fixed size for batch-compatible proofs
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VrfPublicKey {
    encoded: [u8; VRF_PUBLIC_KEY_LENGTH],
}

#[derive(Clone, PartialEq, Eq)]
pub struct VrfPrivateKey {
    secret: [u8; VRF_PRIVATE_KEY_LENGTH],
}

impl fmt::Debug for VrfPrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VrfPrivateKey").finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub struct VrfKeypair {
    pub private: VrfPrivateKey,
    pub public: VrfPublicKey,
}

impl VrfPrivateKey {
    pub fn generate(seed: &[u8; VRF_SEED_LENGTH]) -> Self {
        Self::from_seed(seed).expect("VRF key generation should succeed")
    }

    pub fn from_seed(seed: &[u8; VRF_SEED_LENGTH]) -> Result<Self> {
        ensure_lengths()?;
        ensure_backend()?;

        let mut pk = [0u8; VRF_PUBLIC_KEY_LENGTH];
        let mut sk = [0u8; VRF_PRIVATE_KEY_LENGTH];
        let code = backend::seed_keypair(&mut pk, &mut sk, seed);
        if code != 0 {
            return Err(CryptoError::VrfError("seed keypair failed".into()));
        }

        let private = Self { secret: sk };
        private.try_public_key()?; // validate secret key
        Ok(private)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        ensure_lengths()?;
        if bytes.len() != VRF_PRIVATE_KEY_LENGTH {
            return Err(CryptoError::InvalidKeyLength);
        }

        let mut secret = [0u8; VRF_PRIVATE_KEY_LENGTH];
        secret.copy_from_slice(bytes);
        let private = Self { secret };
        private.try_public_key()?;
        Ok(private)
    }

    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str).map_err(|_| CryptoError::InvalidHexEncoding)?;
        Self::from_bytes(&bytes)
    }

    pub fn to_bytes(&self) -> &[u8; VRF_PRIVATE_KEY_LENGTH] {
        &self.secret
    }

    pub fn to_seed(&self) -> Result<[u8; VRF_SEED_LENGTH]> {
        ensure_backend()?;
        let mut seed = [0u8; VRF_SEED_LENGTH];
        backend::sk_to_seed(&mut seed, &self.secret);
        Ok(seed)
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.secret)
    }

    pub fn public_key(&self) -> VrfPublicKey {
        self.try_public_key()
            .expect("VRF public key derivation should succeed")
    }

    pub fn try_public_key(&self) -> Result<VrfPublicKey> {
        ensure_backend()?;
        let mut encoded = [0u8; VRF_PUBLIC_KEY_LENGTH];
        backend::sk_to_pk(&mut encoded, &self.secret);
        VrfPublicKey::from_bytes(&encoded)
    }

    pub fn prove(&self, input: &[u8]) -> (VrfOutput, VrfProof) {
        self.try_prove(input)
            .expect("VRF proof generation should succeed")
    }

    pub fn try_prove(&self, input: &[u8]) -> Result<(VrfOutput, VrfProof)> {
        ensure_backend()?;

        let mut proof_bytes = [0u8; VRF_PROOF_LENGTH];
        let code = backend::prove(&mut proof_bytes, &self.secret, input);
        if code != 0 {
            return Err(CryptoError::VrfError("prove failed".into()));
        }

        let proof = VrfProof { inner: proof_bytes };
        let output = proof.to_hash()?;
        Ok((output, proof))
    }
}

impl Drop for VrfPrivateKey {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

impl VrfPublicKey {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        ensure_lengths()?;
        if bytes.len() != VRF_PUBLIC_KEY_LENGTH {
            return Err(CryptoError::InvalidKeyLength);
        }
        let mut encoded = [0u8; VRF_PUBLIC_KEY_LENGTH];
        encoded.copy_from_slice(bytes);
        Ok(Self { encoded })
    }

    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str).map_err(|_| CryptoError::InvalidHexEncoding)?;
        Self::from_bytes(&bytes)
    }

    pub fn to_bytes(&self) -> &[u8; VRF_PUBLIC_KEY_LENGTH] {
        &self.encoded
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.encoded)
    }

    pub fn verify(&self, input: &[u8], output: &VrfOutput, proof: &VrfProof) -> bool {
        self.try_verify(input, output, proof).unwrap_or(false)
    }

    pub fn try_verify(&self, input: &[u8], output: &VrfOutput, proof: &VrfProof) -> Result<bool> {
        ensure_backend()?;

        let mut computed = [0u8; VRF_OUTPUT_LENGTH];
        let code = backend::verify(&mut computed, &self.encoded, proof.to_bytes(), input);
        if code != 0 {
            return Ok(false);
        }

        Ok(computed == output.inner)
    }
}

impl VrfProof {
    pub fn from_bytes<T: AsRef<[u8]>>(bytes: T) -> Result<Self> {
        ensure_lengths()?;
        let bytes = bytes.as_ref();
        if bytes.len() != VRF_PROOF_LENGTH {
            return Err(CryptoError::InvalidProofLength);
        }
        let mut inner = [0u8; VRF_PROOF_LENGTH];
        inner.copy_from_slice(bytes);
        Ok(Self { inner })
    }

    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str).map_err(|_| CryptoError::InvalidHexEncoding)?;
        Self::from_bytes(bytes)
    }

    pub fn to_bytes(&self) -> &[u8; VRF_PROOF_LENGTH] {
        &self.inner
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.inner)
    }

    pub fn to_hash(&self) -> Result<VrfOutput> {
        ensure_backend()?;
        let mut hash = [0u8; VRF_OUTPUT_LENGTH];
        let code = backend::proof_to_hash(&mut hash, &self.inner);
        if code != 0 {
            return Err(CryptoError::VrfError("proof_to_hash failed".into()));
        }
        VrfOutput::from_bytes(hash)
    }
}

impl VrfOutput {
    pub fn from_bytes<T: AsRef<[u8]>>(bytes: T) -> Result<Self> {
        ensure_lengths()?;
        let bytes = bytes.as_ref();
        if bytes.len() != VRF_OUTPUT_LENGTH {
            return Err(CryptoError::InvalidOutputLength);
        }
        let mut inner = [0u8; VRF_OUTPUT_LENGTH];
        inner.copy_from_slice(bytes);
        Ok(Self { inner })
    }

    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex::decode(hex_str).map_err(|_| CryptoError::InvalidHexEncoding)?;
        Self::from_bytes(bytes)
    }

    pub fn to_bytes(&self) -> &[u8; VRF_OUTPUT_LENGTH] {
        &self.inner
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.inner)
    }
}

impl VrfKeypair {
    pub fn generate() -> Result<Self> {
        ensure_lengths()?;
        ensure_backend()?;

        let mut pk = [0u8; VRF_PUBLIC_KEY_LENGTH];
        let mut sk = [0u8; VRF_PRIVATE_KEY_LENGTH];
        let code = backend::random_keypair(&mut pk, &mut sk);
        if code != 0 {
            return Err(CryptoError::VrfError("keypair generation failed".into()));
        }

        Ok(Self {
            private: VrfPrivateKey { secret: sk },
            public: VrfPublicKey { encoded: pk },
        })
    }

    pub fn from_seed(seed: &[u8; VRF_SEED_LENGTH]) -> Result<Self> {
        let private = VrfPrivateKey::from_seed(seed)?;
        let public = private.try_public_key()?;
        Ok(Self { private, public })
    }

    pub fn random() -> Result<Self> {
        let mut seed = [0u8; VRF_SEED_LENGTH];
        OsRng.fill_bytes(&mut seed);
        Self::from_seed(&seed)
    }
}

fn ensure_backend() -> Result<()> {
    backend::ensure_backend_init()
        .map_err(|err| CryptoError::LibraryInitializationFailed(err.to_string()))
}

fn ensure_lengths() -> Result<()> {
    if *PUBLIC_KEY_BYTES != VRF_PUBLIC_KEY_LENGTH
        || *SECRET_KEY_BYTES != VRF_PRIVATE_KEY_LENGTH
        || *PROOF_BYTES != VRF_PROOF_LENGTH
        || *OUTPUT_BYTES != VRF_OUTPUT_LENGTH
        || *SEED_BYTES != VRF_SEED_LENGTH
    {
        return Err(CryptoError::VrfError(
            "compiled VRF sizes do not match expected constants".into(),
        ));
    }
    Ok(())
}
