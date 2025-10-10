//! Key Evolving Signatures (KES)
//!
//! This module ports the hierarchy of KES implementations from the Haskell
//! `cardano-crypto-class` package, preserving naming and structure to make
//! cross-referencing straightforward:
//!
//! | Haskell Module | Rust Module / Type |
//! |----------------|--------------------|
//! | `Cardano.Crypto.KES.Class` | `kes::KesAlgorithm` trait |
//! | `Cardano.Crypto.KES.Single` | `kes::single::SingleKes` |
//! | `Cardano.Crypto.KES.CompactSingle` | `kes::compact_single::CompactSingleKes` |
//! | `Cardano.Crypto.KES.Sum` | `kes::sum::{Sum0Kes..Sum7Kes}` |
//! | `Cardano.Crypto.KES.CompactSum` | `kes::compact_sum::{CompactSum0Kes..CompactSum7Kes}` |
//! | `hashVerKeyKES` (Haskell method) | `KesAlgorithm::hash_verification_key_kes` |
//!
//! # Forward security model
//!
//! A KES signing key evolves through a fixed number of discrete periods
//! (enumerated from 0). Each evolution irreversibly destroys the ability to
//! produce signatures for past periods. In practice this is achieved by:
//!
//! 1. Deriving fresh lower-level (or leaf) DSIGN keys from secret material.
//! 2. Zeroizing / overwriting intermediate secret nodes when advancing.
//! 3. Reconstructing verification key paths during verification instead of
//!    storing all subtree keys persistently (Compact variants embed the
//!    "off-path" key material inside signatures to enable this).
//!
//! The `update_kes` method returns `Ok(Some(new_key))` for a successful
//! transition and `Ok(None)` when the key has reached `total_periods()` and has
//! expired. Attempting to sign outside `[0, total_periods())` yields
//! `KesError::PeriodOutOfRange`, while using an evolved key beyond its final
//! period yields `KesError::KeyExpired`.
//!
//! # Period evolution guide
//!
//! | Family | Total Periods | Evolution Strategy |
//! |--------|---------------|--------------------|
//! | `SingleKes` / `CompactSingleKes` | 1 | Period fixed at 0; any other period rejected. |
//! | `Sum n` | 2^n | Binary tree: first half uses left subtree; after boundary switch to right subtree; internal node secrets discarded as soon as children derived. |
//! | `CompactSum n` | 2^n | Same schedule as `Sum n`, but signatures include the *off-path* verification key, letting the verifier reconstruct the full root with one fewer stored key per node. |
//!
//! Verification replays the period routing logic: it decides which leaf /
//! subtree signature should be present and reconstructs intermediate hashes
//! (or verification keys for compact variants) to compare against the root.
//!
//! # Zeroization & secure memory
//!
//! Signing keys may contain mlocked buffers. The `forget_signing_key_kes`
//! method enforces explicit destruction semantics mirroring Haskell's
//! `forgetSignKeyKES`. Internal evolution steps also overwrite obsolete secret
//! material, supporting the forward security guarantee that historical signing
//! capability cannot be restored.
//!
//! # Unsound operations
//!
//! The `UnsoundKesAlgorithm` trait exposes raw signing key (de)serialization
//! strictly for testing / vector generation. Production code should never
//! persist signing keys in raw form outside controlled secure memory contexts.
//!
//! # Metrics
//!
//! When compiled with the crate feature `kes-metrics`, lightweight relaxed
//! atomic counters (see `kes::metrics`) provide coarse-grained counts of signing
//! keys, signatures, signature bytes, and update operations to aid benchmarking
//! and regression analysis. They are zero-cost when the feature is disabled.
//!
//! # Example
//!
//! ```rust
//! use cardano_crypto_class::kes::{KesAlgorithm, SingleKes};
//! use cardano_crypto_class::dsign::ed25519::Ed25519;
//!
//! // Generate a deterministic seed (all zeros for illustration) of required length.
//! let seed = vec![0u8; SingleKes::<Ed25519>::SEED_SIZE];
//! let sk = SingleKes::<Ed25519>::gen_key_kes_from_seed_bytes(&seed).unwrap();
//! let vk = SingleKes::<Ed25519>::derive_verification_key(&sk).unwrap();
//! let msg = b"epoch-boundary";
//! let sig = SingleKes::<Ed25519>::sign_kes(&(), 0, msg, &sk).unwrap();
//! SingleKes::<Ed25519>::verify_kes(&(), &vk, 0, msg, &sig).unwrap();
//! ```
use std::fmt;
use std::marker::PhantomData;

use thiserror::Error;

use crate::vendor::crypto_class::mlocked_bytes::MLockedError;
use crate::vendor::crypto_class::seed::{Seed, get_bytes_from_seed_t};
use crate::vendor::crypto_class::util::SignableRepresentation;

pub mod compact_single;
pub mod compact_sum;
pub mod hash;
pub mod metrics;
pub mod single;
pub mod sum;
pub mod verify_hash;

// Re-export hash algorithms for convenience
pub use hash::{Blake2b224, Blake2b256, Blake2b512, KesHashAlgorithm};

// Re-export SingleKes types
pub use single::SingleKes;

// Re-export CompactSingleKes types
pub use compact_single::{CompactSingleKes, CompactSingleSig, OptimizedKesSignature};

// Re-export Sum type aliases (using Blake2b256)
pub use sum::{Sum0Kes, Sum1Kes, Sum2Kes, Sum3Kes, Sum4Kes, Sum5Kes, Sum6Kes, Sum7Kes};

// Re-export CompactSum type aliases (using Blake2b256)
pub use compact_sum::{
    CompactSum0Kes, CompactSum1Kes, CompactSum2Kes, CompactSum3Kes, CompactSum4Kes, CompactSum5Kes,
    CompactSum6Kes, CompactSum7Kes,
};

/// The KES period. Periods are enumerated from zero.
pub type Period = u64;

/// Error raised by KES operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KesError {
    #[error("KES signature verification failed")]
    VerificationFailed,
    #[error("{context}: wrong length, expected {expected} bytes but got {actual}")]
    WrongLength {
        context: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("{0}")]
    Message(String),
    #[error("KES key evolved beyond max period")]
    KeyExpired,
    #[error("period {period} out of range [0, {max_period})")]
    PeriodOutOfRange { period: Period, max_period: Period },
}

impl KesError {
    #[must_use]
    pub fn wrong_length(context: &'static str, expected: usize, actual: usize) -> Self {
        KesError::WrongLength {
            context,
            expected,
            actual,
        }
    }
}

/// Error raised by mlocked KES operations.
#[derive(Debug, Error)]
pub enum KesMError {
    #[error(transparent)]
    Kes(#[from] KesError),
    #[error(transparent)]
    Mlocked(#[from] MLockedError),
    #[error("DSIGN error: {0}")]
    Dsign(String),
}

/// Trait capturing the common KES (Key Evolving Signature) interface.
///
/// This follows the design from "Composition and Efficiency Tradeoffs for
/// Forward-Secure Digital Signatures" by Tal Malkin, Daniele Micciancio,
/// and Sara Miner (<https://eprint.iacr.org/2001/034>).
pub trait KesAlgorithm {
    /// Verification key type.
    type VerificationKey;
    /// Signing key type (may contain mlocked memory).
    type SigningKey;
    /// Signature type.
    type Signature;
    /// Optional context parameter.
    type Context;

    /// Name of the algorithm.
    const ALGORITHM_NAME: &'static str;
    /// Number of seed bytes required.
    const SEED_SIZE: usize;
    /// Size of the verification key when serialized.
    const VERIFICATION_KEY_SIZE: usize;
    /// Size of the signing key when serialized.
    const SIGNING_KEY_SIZE: usize;
    /// Size of signatures produced.
    const SIGNATURE_SIZE: usize;

    /// Total number of periods this KES scheme supports.
    fn total_periods() -> Period;

    /// Derive the verification key from a signing key.
    ///
    /// # Errors
    ///
    /// Returns an error if the signing key cannot produce a valid verification key.
    fn derive_verification_key(
        signing_key: &Self::SigningKey,
    ) -> Result<Self::VerificationKey, KesMError>;

    /// Sign a message at a specific period.
    ///
    /// # Errors
    ///
    /// Returns an error if the signing operation fails or the key material is invalid.
    fn sign_kes(
        context: &Self::Context,
        period: Period,
        message: &[u8],
        signing_key: &Self::SigningKey,
    ) -> Result<Self::Signature, KesMError>;

    /// Verify a KES signature at a specific period.
    ///
    /// # Errors
    ///
    /// Returns an error if verification fails.
    fn verify_kes(
        context: &Self::Context,
        verification_key: &Self::VerificationKey,
        period: Period,
        message: &[u8],
        signature: &Self::Signature,
    ) -> Result<(), KesError>;

    /// Update (evolve) the signing key to the next period.
    ///
    /// Returns None if the key has expired (reached max period).
    ///
    /// # Errors
    ///
    /// Returns an error if the signing key cannot be evolved.
    fn update_kes(
        context: &Self::Context,
        signing_key: Self::SigningKey,
        period: Period,
    ) -> Result<Option<Self::SigningKey>, KesMError>;

    /// Generate a signing key from a seed.
    ///
    /// # Errors
    ///
    /// Returns an error if the derived seed bytes do not produce a valid signing key.
    ///
    /// # Panics
    ///
    /// Panics if the supplied [`Seed`] does not provide enough entropy to
    /// produce [`KesAlgorithm::SEED_SIZE`] bytes.
    fn gen_key_kes(seed: &Seed) -> Result<Self::SigningKey, KesMError> {
        let (material, _) = get_bytes_from_seed_t(Self::SEED_SIZE, seed.clone());
        Self::gen_key_kes_from_seed_bytes(&material)
    }

    /// Generate a signing key from raw seed bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the bytes do not form a valid signing key.
    fn gen_key_kes_from_seed_bytes(seed: &[u8]) -> Result<Self::SigningKey, KesMError>;

    /// Serialize the verification key.
    fn raw_serialize_verification_key_kes(key: &Self::VerificationKey) -> Vec<u8>;

    /// Deserialize a verification key.
    fn raw_deserialize_verification_key_kes(bytes: &[u8]) -> Option<Self::VerificationKey>;

    /// Serialize a signature.
    fn raw_serialize_signature_kes(signature: &Self::Signature) -> Vec<u8>;

    /// Deserialize a signature.
    fn raw_deserialize_signature_kes(bytes: &[u8]) -> Option<Self::Signature>;

    /// Securely forget/zeroize a signing key.
    fn forget_signing_key_kes(signing_key: Self::SigningKey);

    /// Hash a verification key using the specified hash algorithm.
    ///
    /// This is a convenience method that serializes the verification key and hashes it.
    /// Provides API parity with Haskell's `hashVerKeyKES` method.
    ///
    /// # Type Parameters
    /// * `H` - The hash algorithm to use (must implement `KesHashAlgorithm`)
    ///
    /// # Example
    /// ```rust
    /// use cardano_crypto_class::kes::{hash::Blake2b256, KesAlgorithm, Sum1Kes};
    /// use cardano_crypto_class::seed::Seed;
    ///
    /// // Generate a deterministic signing key and derive its verification key.
    /// let seed_bytes = vec![0u8; Sum1Kes::SEED_SIZE];
    /// let seed = Seed::from_bytes(seed_bytes);
    /// let signing_key = Sum1Kes::gen_key_kes(&seed).expect("signing key generation");
    /// let verification_key =
    ///     Sum1Kes::derive_verification_key(&signing_key).expect("verification key derivation");
    ///
    /// // Hash the verification key using Blake2b256.
    /// let digest = Sum1Kes::hash_verification_key_kes::<Blake2b256>(&verification_key);
    /// assert_eq!(digest.len(), 32);
    /// ```
    fn hash_verification_key_kes<H: hash::KesHashAlgorithm>(
        verification_key: &Self::VerificationKey,
    ) -> Vec<u8> {
        let serialized = Self::raw_serialize_verification_key_kes(verification_key);
        H::hash(&serialized)
    }
}

/// Trait for unsound KES operations (exposing signing key serialization).
pub trait UnsoundKesAlgorithm: KesAlgorithm {
    /// Serialize a signing key (UNSOUND - use only for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the signing key cannot be serialized.
    fn raw_serialize_signing_key_kes(signing_key: &Self::SigningKey) -> Result<Vec<u8>, KesMError>;

    /// Deserialize a signing key (UNSOUND - use only for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if the bytes do not represent a valid signing key.
    fn raw_deserialize_signing_key_kes(bytes: &[u8]) -> Result<Self::SigningKey, KesMError>;
}

/// Wrapper around a KES signature carrying algorithm and message types.
#[derive(Clone)]
pub struct SignedKes<A, M>
where
    A: KesAlgorithm,
    M: ?Sized,
{
    signature: A::Signature,
    period: Period,
    _marker: PhantomData<fn(&M)>,
}

impl<A, M> SignedKes<A, M>
where
    A: KesAlgorithm,
    M: ?Sized,
{
    pub fn new(signature: A::Signature, period: Period) -> Self {
        Self {
            signature,
            period,
            _marker: PhantomData,
        }
    }

    pub fn signature(&self) -> &A::Signature {
        &self.signature
    }

    pub fn period(&self) -> Period {
        self.period
    }

    pub fn into_inner(self) -> (A::Signature, Period) {
        (self.signature, self.period)
    }
}

impl<A, M> fmt::Debug for SignedKes<A, M>
where
    A: KesAlgorithm,
    A::Signature: fmt::Debug,
    M: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignedKes")
            .field("signature", &self.signature)
            .field("period", &self.period)
            .finish()
    }
}

/// Convenience function to create a signed KES value.
///
/// # Errors
///
/// Propagates failures from the underlying signing routine.
pub fn signed_kes<A, M>(
    context: &A::Context,
    period: Period,
    message: &M,
    signing_key: &A::SigningKey,
) -> Result<SignedKes<A, M>, KesMError>
where
    A: KesAlgorithm,
    M: SignableRepresentation + ?Sized,
{
    let representation = message.signable_representation();
    let signature = A::sign_kes(context, period, representation.as_ref(), signing_key)?;
    Ok(SignedKes::new(signature, period))
}

/// Verify a signed KES value.
///
/// # Errors
///
/// Returns an error if verification fails.
pub fn verify_signed_kes<A, M>(
    context: &A::Context,
    verification_key: &A::VerificationKey,
    message: &M,
    signed: &SignedKes<A, M>,
) -> Result<(), KesError>
where
    A: KesAlgorithm,
    M: SignableRepresentation + ?Sized,
{
    let representation = message.signable_representation();
    A::verify_kes(
        context,
        verification_key,
        signed.period(),
        representation.as_ref(),
        signed.signature(),
    )
}

/// Helper functions
#[must_use]
pub const fn seed_size_kes<A: KesAlgorithm>() -> usize {
    A::SEED_SIZE
}

#[must_use]
pub const fn size_verification_key_kes<A: KesAlgorithm>() -> usize {
    A::VERIFICATION_KEY_SIZE
}

#[must_use]
pub const fn size_signing_key_kes<A: KesAlgorithm>() -> usize {
    A::SIGNING_KEY_SIZE
}

#[must_use]
pub const fn size_signature_kes<A: KesAlgorithm>() -> usize {
    A::SIGNATURE_SIZE
}

#[must_use]
pub fn total_periods_kes<A: KesAlgorithm>() -> Period {
    A::total_periods()
}
