use std::fmt;
use std::marker::PhantomData;

use thiserror::Error;

use crate::vendor::crypto_class::mlocked_bytes::MLockedError;
use crate::vendor::crypto_class::seed::{Seed, get_bytes_from_seed_t};
use crate::vendor::crypto_class::util::SignableRepresentation;

pub mod ed25519;
pub mod ed25519_mlocked;

/// Error raised by DSIGN operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DsignError {
    #[error("signature verification failed")]
    VerificationFailed,
    #[error("{context}: wrong length, expected {expected} bytes but got {actual}")]
    WrongLength {
        context: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("{0}")]
    Message(String),
}

impl DsignError {
    /// Helper mirroring the Haskell `failSizeCheck` behaviour.
    #[must_use]
    pub fn wrong_length(context: &'static str, expected: usize, actual: usize) -> Self {
        DsignError::WrongLength {
            context,
            expected,
            actual,
        }
    }
}

/// Error raised by DSIGNM operations.
#[derive(Debug, Error)]
pub enum DsignMError {
    #[error(transparent)]
    Dsign(#[from] DsignError),
    #[error(transparent)]
    Mlocked(#[from] MLockedError),
}

/// Trait capturing the common DSIGN interface across algorithms.
pub trait DsignAlgorithm {
    /// Signing key type.
    type SigningKey;
    /// Verification key type.
    type VerificationKey;
    /// Signature type.
    type Signature;
    /// Optional context parameter mirroring the Haskell API.
    type Context;

    /// Name of the algorithm (e.g. `ed25519`).
    const ALGORITHM_NAME: &'static str;
    /// Number of seed bytes required to generate a key.
    const SEED_SIZE: usize;
    /// Size of the verification key when serialised.
    const VERIFICATION_KEY_SIZE: usize;
    /// Size of the signing key when serialised.
    const SIGNING_KEY_SIZE: usize;
    /// Size of signatures produced by the algorithm.
    const SIGNATURE_SIZE: usize;

    /// Derive the verification key from a signing key.
    fn derive_verification_key(signing_key: &Self::SigningKey) -> Self::VerificationKey;

    /// Sign a message provided as raw bytes.
    fn sign_bytes(
        context: &Self::Context,
        message: &[u8],
        signing_key: &Self::SigningKey,
    ) -> Self::Signature;

    /// Verify a signature over raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the signature is invalid for the provided message
    /// or key material.
    fn verify_bytes(
        context: &Self::Context,
        verification_key: &Self::VerificationKey,
        message: &[u8],
        signature: &Self::Signature,
    ) -> Result<(), DsignError>;

    /// Deterministically generate a signing key from the supplied seed.
    ///
    /// Mirrors the Haskell `genKeyDSIGN` behaviour by panicking when the seed
    /// does not provide enough bytes.
    ///
    /// # Panics
    ///
    /// Panics if the supplied [`Seed`] cannot provide
    /// [`DsignAlgorithm::SEED_SIZE`] bytes.
    #[must_use]
    fn gen_key(seed: &Seed) -> Self::SigningKey {
        let (material, _) = get_bytes_from_seed_t(Self::SEED_SIZE, seed.clone());
        Self::gen_key_from_seed_bytes(&material)
    }

    /// Construct a signing key from raw seed bytes. The slice length is
    /// guaranteed to match [`DsignAlgorithm::SEED_SIZE`].
    fn gen_key_from_seed_bytes(seed: &[u8]) -> Self::SigningKey;

    /// Serialise the verification key into raw bytes.
    fn raw_serialize_verification_key(key: &Self::VerificationKey) -> Vec<u8>;

    /// Deserialise a verification key from raw bytes.
    fn raw_deserialize_verification_key(bytes: &[u8]) -> Option<Self::VerificationKey>;

    /// Serialise the signing key into raw bytes.
    fn raw_serialize_signing_key(signing_key: &Self::SigningKey) -> Vec<u8>;

    /// Deserialise a signing key from raw bytes.
    fn raw_deserialize_signing_key(bytes: &[u8]) -> Option<Self::SigningKey>;

    /// Serialise a signature into raw bytes.
    fn raw_serialize_signature(signature: &Self::Signature) -> Vec<u8>;

    /// Deserialise a signature from raw bytes.
    fn raw_deserialize_signature(bytes: &[u8]) -> Option<Self::Signature>;
}

/// Convenience wrapper producing a [`SignedDsign`] value.
pub fn signed_dsign<A, M>(
    context: &A::Context,
    message: &M,
    signing_key: &A::SigningKey,
) -> SignedDsign<A, M>
where
    A: DsignAlgorithm,
    M: SignableRepresentation + ?Sized,
{
    let representation = message.signable_representation();
    let signature = A::sign_bytes(context, representation.as_ref(), signing_key);
    SignedDsign::new(signature)
}

/// Verify a [`SignedDsign`] value.
///
/// # Errors
///
/// Returns an error if signature verification fails.
pub fn verify_signed_dsign<A, M>(
    context: &A::Context,
    verification_key: &A::VerificationKey,
    message: &M,
    signed: &SignedDsign<A, M>,
) -> Result<(), DsignError>
where
    A: DsignAlgorithm,
    M: SignableRepresentation + ?Sized,
{
    let representation = message.signable_representation();
    A::verify_bytes(
        context,
        verification_key,
        representation.as_ref(),
        signed.signature(),
    )
}

/// Helper mirroring `failSizeCheck` from the Haskell implementation.
#[must_use]
pub fn fail_size_check(function: &'static str, expected: usize, actual: usize) -> DsignError {
    DsignError::wrong_length(function, expected, actual)
}

/// Helper returning the required seed size for algorithm `A`.
#[must_use]
pub const fn seed_size<A: DsignAlgorithm>() -> usize {
    A::SEED_SIZE
}

/// Helper returning the verification key size for algorithm `A`.
#[must_use]
pub const fn size_verification_key<A: DsignAlgorithm>() -> usize {
    A::VERIFICATION_KEY_SIZE
}

/// Helper returning the signing key size for algorithm `A`.
#[must_use]
pub const fn size_signing_key<A: DsignAlgorithm>() -> usize {
    A::SIGNING_KEY_SIZE
}

/// Helper returning the signature size for algorithm `A`.
#[must_use]
pub const fn size_signature<A: DsignAlgorithm>() -> usize {
    A::SIGNATURE_SIZE
}

/// Wrapper around a signature carrying the algorithm type and phantom message.
#[derive(Clone)]
pub struct SignedDsign<A, M>
where
    A: DsignAlgorithm,
    M: ?Sized,
{
    signature: A::Signature,
    _marker: PhantomData<fn(&M)>,
}

impl<A, M> SignedDsign<A, M>
where
    A: DsignAlgorithm,
    M: ?Sized,
{
    pub fn new(signature: A::Signature) -> Self {
        Self {
            signature,
            _marker: PhantomData,
        }
    }

    pub fn signature(&self) -> &A::Signature {
        &self.signature
    }

    pub fn into_inner(self) -> A::Signature {
        self.signature
    }
}

impl<A, M> fmt::Debug for SignedDsign<A, M>
where
    A: DsignAlgorithm,
    A::Signature: fmt::Debug,
    M: ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SignedDsign").field(&self.signature).finish()
    }
}

impl<A, M> PartialEq for SignedDsign<A, M>
where
    A: DsignAlgorithm,
    A::Signature: PartialEq,
    M: ?Sized,
{
    fn eq(&self, other: &Self) -> bool {
        self.signature == other.signature
    }
}

impl<A, M> Eq for SignedDsign<A, M>
where
    A: DsignAlgorithm,
    A::Signature: Eq,
    M: ?Sized,
{
}

/// DSIGN algorithms supporting secure memory-backed signing keys.
pub trait DsignMAlgorithm: DsignAlgorithm {
    /// Signing key stored in mlocked memory.
    type MLockedSigningKey;
    /// Seed material stored in mlocked memory.
    type SeedMaterial;

    /// Derive the verification key from an mlocked signing key.
    ///
    /// # Errors
    ///
    /// Propagates failures returned by the underlying algorithm or memory
    /// handling.
    fn derive_verification_key_m(
        signing_key: &Self::MLockedSigningKey,
    ) -> Result<Self::VerificationKey, DsignMError>;

    /// Sign raw bytes using an mlocked signing key.
    ///
    /// # Errors
    ///
    /// Returns an error if signing fails.
    fn sign_bytes_m(
        context: &Self::Context,
        message: &[u8],
        signing_key: &Self::MLockedSigningKey,
    ) -> Result<Self::Signature, DsignError>;

    /// Generate a signing key from an mlocked seed.
    ///
    /// # Errors
    ///
    /// Returns an error if key generation fails or the seed material is
    /// invalid.
    fn gen_key_m(seed: &Self::SeedMaterial) -> Result<Self::MLockedSigningKey, DsignMError>;

    /// Clone an mlocked signing key.
    ///
    /// # Errors
    ///
    /// Returns an error if cloning fails.
    fn clone_key_m(
        signing_key: &Self::MLockedSigningKey,
    ) -> Result<Self::MLockedSigningKey, DsignMError>;

    /// Extract the seed material from an mlocked signing key.
    ///
    /// # Errors
    ///
    /// Returns an error if the seed cannot be recovered.
    fn get_seed_m(signing_key: &Self::MLockedSigningKey)
    -> Result<Self::SeedMaterial, DsignMError>;

    /// Securely forget an mlocked signing key by consuming it.
    fn forget_signing_key_m(signing_key: Self::MLockedSigningKey);
}

/// Convenience wrapper for signing using an mlocked key.
///
/// # Errors
///
/// Propagates failures from the underlying signing routine.
pub fn signed_dsign_m<A, M>(
    context: &A::Context,
    message: &M,
    signing_key: &A::MLockedSigningKey,
) -> Result<SignedDsign<A, M>, DsignError>
where
    A: DsignMAlgorithm,
    M: SignableRepresentation + ?Sized,
{
    let representation = message.signable_representation();
    let signature = A::sign_bytes_m(context, representation.as_ref(), signing_key)?;
    Ok(SignedDsign::new(signature))
}

/// Trait exposing unsound serialisation for mlocked signing keys.
pub trait UnsoundDsignMAlgorithm: DsignMAlgorithm {
    /// Serialise an mlocked signing key into raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if serialisation fails.
    fn raw_serialize_signing_key_m(
        signing_key: &Self::MLockedSigningKey,
    ) -> Result<Vec<u8>, DsignMError>;

    /// Deserialise an mlocked signing key from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the bytes do not represent a valid signing key.
    fn raw_deserialize_signing_key_m(bytes: &[u8]) -> Result<Self::MLockedSigningKey, DsignMError>;
}
