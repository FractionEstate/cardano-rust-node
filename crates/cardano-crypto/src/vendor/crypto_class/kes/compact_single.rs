use std::marker::PhantomData;

use crate::vendor::crypto_class::dsign::{DsignMAlgorithm, UnsoundDsignMAlgorithm};
use crate::vendor::crypto_class::kes::{KesAlgorithm, KesError, KesMError, Period};

/// CompactSingleKES wraps a DSIGNM algorithm with an embedded verification key.
///
/// Unlike SingleKES, the signature includes the verification key. This allows
/// CompactSumKES to reconstruct verification keys from signatures, reducing
/// the number of keys that need to be stored in the Merkle tree.
pub struct CompactSingleKes<D: DsignMAlgorithm>(PhantomData<D>);

/// Signature type that embeds the verification key.
pub struct CompactSingleSig<D: DsignMAlgorithm> {
    pub(crate) signature: D::Signature,
    pub(crate) verification_key: D::VerificationKey,
}

impl<D> Clone for CompactSingleSig<D>
where
    D: DsignMAlgorithm,
    D::Signature: Clone,
    D::VerificationKey: Clone,
{
    fn clone(&self) -> Self {
        Self {
            signature: self.signature.clone(),
            verification_key: self.verification_key.clone(),
        }
    }
}

// Manual PartialEq and Eq implementations
impl<D: DsignMAlgorithm> PartialEq for CompactSingleSig<D>
where
    D::Signature: PartialEq,
    D::VerificationKey: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.signature == other.signature && self.verification_key == other.verification_key
    }
}

impl<D: DsignMAlgorithm> Eq for CompactSingleSig<D>
where
    D::Signature: Eq,
    D::VerificationKey: Eq,
{
}

// Manual Debug implementation for CompactSingleSig
impl<D: DsignMAlgorithm> std::fmt::Debug for CompactSingleSig<D>
where
    D::Signature: std::fmt::Debug,
    D::VerificationKey: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompactSingleSig")
            .field("signature", &self.signature)
            .field("verification_key", &self.verification_key)
            .finish()
    }
}

// Serde implementations for CompactSingleSig
#[cfg(feature = "serde")]
impl<D> serde::Serialize for CompactSingleSig<D>
where
    D: DsignMAlgorithm,
    D::Signature: serde::Serialize,
    D::VerificationKey: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeTuple;
        let mut tuple = serializer.serialize_tuple(2)?;
        tuple.serialize_element(&self.signature)?;
        tuple.serialize_element(&self.verification_key)?;
        tuple.end()
    }
}

#[cfg(feature = "serde")]
impl<'de, D> serde::Deserialize<'de> for CompactSingleSig<D>
where
    D: DsignMAlgorithm,
    D::Signature: serde::Deserialize<'de>,
    D::VerificationKey: serde::Deserialize<'de>,
{
    fn deserialize<DE>(deserializer: DE) -> Result<Self, DE::Error>
    where
        DE: serde::Deserializer<'de>,
    {
        use serde::de::{self, SeqAccess, Visitor};

        struct CompactSingleSigVisitor<D>(PhantomData<D>);

        impl<'de, D> Visitor<'de> for CompactSingleSigVisitor<D>
        where
            D: DsignMAlgorithm,
            D::Signature: serde::Deserialize<'de>,
            D::VerificationKey: serde::Deserialize<'de>,
        {
            type Value = CompactSingleSig<D>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a CompactSingleKES signature tuple (signature, vk)")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let signature = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let verification_key = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;

                Ok(CompactSingleSig {
                    signature,
                    verification_key,
                })
            }
        }

        deserializer.deserialize_tuple(2, CompactSingleSigVisitor(PhantomData))
    }
}

impl<D> KesAlgorithm for CompactSingleKes<D>
where
    D: DsignMAlgorithm + UnsoundDsignMAlgorithm,
    D::VerificationKey: Clone,
{
    type VerificationKey = D::VerificationKey;
    type SigningKey = D::MLockedSigningKey;
    type Signature = CompactSingleSig<D>;
    type Context = D::Context;

    const ALGORITHM_NAME: &'static str = D::ALGORITHM_NAME;
    const SEED_SIZE: usize = D::SEED_SIZE;
    const VERIFICATION_KEY_SIZE: usize = D::VERIFICATION_KEY_SIZE;
    const SIGNING_KEY_SIZE: usize = D::SIGNING_KEY_SIZE;
    // Signature size is DSIGN signature + verification key
    const SIGNATURE_SIZE: usize = D::SIGNATURE_SIZE + D::VERIFICATION_KEY_SIZE;

    fn total_periods() -> Period {
        1
    }

    fn derive_verification_key(
        signing_key: &Self::SigningKey,
    ) -> Result<Self::VerificationKey, KesMError> {
        D::derive_verification_key_m(signing_key).map_err(|e| KesMError::Dsign(format!("{:?}", e)))
    }

    fn sign_kes(
        context: &Self::Context,
        period: Period,
        message: &[u8],
        signing_key: &Self::SigningKey,
    ) -> Result<Self::Signature, KesMError> {
        if period != 0 {
            return Err(KesMError::Kes(KesError::PeriodOutOfRange {
                period,
                max_period: 1,
            }));
        }
        let signature = D::sign_bytes_m(context, message, signing_key)
            .map_err(|e| KesMError::Dsign(format!("{:?}", e)))?;
        let verification_key = D::derive_verification_key_m(signing_key)
            .map_err(|e| KesMError::Dsign(format!("{:?}", e)))?;
        Ok(CompactSingleSig {
            signature,
            verification_key,
        })
    }

    fn verify_kes(
        context: &Self::Context,
        _verification_key: &Self::VerificationKey,
        period: Period,
        message: &[u8],
        signature: &Self::Signature,
    ) -> Result<(), KesError> {
        if period != 0 {
            return Err(KesError::PeriodOutOfRange {
                period,
                max_period: 1,
            });
        }
        // Verify using the embedded verification key
        D::verify_bytes(
            context,
            &signature.verification_key,
            message,
            &signature.signature,
        )
        .map_err(|_| KesError::VerificationFailed)
    }

    fn update_kes(
        _context: &Self::Context,
        signing_key: Self::SigningKey,
        period: Period,
    ) -> Result<Option<Self::SigningKey>, KesMError> {
        let last_period = Self::total_periods().saturating_sub(1);

        if period >= last_period {
            D::forget_signing_key_m(signing_key);
            Ok(None)
        } else {
            Ok(Some(signing_key))
        }
    }

    fn gen_key_kes_from_seed_bytes(seed: &[u8]) -> Result<Self::SigningKey, KesMError> {
        // Use the UnsoundDsignMAlgorithm trait which provides raw_deserialize_signing_key_m
        // This constructs an MLocked signing key directly from seed bytes
        D::raw_deserialize_signing_key_m(seed).map_err(|e| KesMError::Dsign(format!("{:?}", e)))
    }

    fn raw_serialize_verification_key_kes(key: &Self::VerificationKey) -> Vec<u8> {
        D::raw_serialize_verification_key(key)
    }

    fn raw_deserialize_verification_key_kes(bytes: &[u8]) -> Option<Self::VerificationKey> {
        D::raw_deserialize_verification_key(bytes)
    }

    fn raw_serialize_signature_kes(signature: &Self::Signature) -> Vec<u8> {
        let mut result = D::raw_serialize_signature(&signature.signature);
        result.extend_from_slice(&D::raw_serialize_verification_key(
            &signature.verification_key,
        ));
        result
    }

    fn raw_deserialize_signature_kes(bytes: &[u8]) -> Option<Self::Signature> {
        if bytes.len() != Self::SIGNATURE_SIZE {
            return None;
        }
        let sig_bytes = &bytes[0..D::SIGNATURE_SIZE];
        let vk_bytes = &bytes[D::SIGNATURE_SIZE..];

        let signature = D::raw_deserialize_signature(sig_bytes)?;
        let verification_key = D::raw_deserialize_verification_key(vk_bytes)?;

        Some(CompactSingleSig {
            signature,
            verification_key,
        })
    }

    fn forget_signing_key_kes(signing_key: Self::SigningKey) {
        D::forget_signing_key_m(signing_key);
    }
}

/// Helper trait to extract the verification key from a CompactSingle signature.
pub trait OptimizedKesSignature {
    type VerificationKey;

    fn extract_verification_key(&self) -> &Self::VerificationKey;
}

impl<D: DsignMAlgorithm> OptimizedKesSignature for CompactSingleSig<D> {
    type VerificationKey = D::VerificationKey;

    fn extract_verification_key(&self) -> &Self::VerificationKey {
        &self.verification_key
    }
}
