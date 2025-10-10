use std::marker::PhantomData;

use crate::vendor::crypto_class::dsign::{DsignMAlgorithm, UnsoundDsignMAlgorithm};
#[cfg(feature = "kes-metrics")]
use crate::vendor::crypto_class::kes::metrics;
use crate::vendor::crypto_class::kes::{KesAlgorithm, KesError, KesMError, Period}; // instrumentation (no-op when feature disabled)

/// SingleKES wraps a DSIGNM algorithm to provide a 1-period KES.
///
/// This is the base case for KES composition. It simply delegates to the
/// underlying DSIGN algorithm and only supports period 0.
pub struct SingleKes<D: DsignMAlgorithm>(PhantomData<D>);

impl<D> KesAlgorithm for SingleKes<D>
where
    D: DsignMAlgorithm + UnsoundDsignMAlgorithm,
{
    type VerificationKey = D::VerificationKey;
    type SigningKey = D::MLockedSigningKey;
    type Signature = D::Signature;
    type Context = D::Context;

    const ALGORITHM_NAME: &'static str = D::ALGORITHM_NAME;
    const SEED_SIZE: usize = D::SEED_SIZE;
    const VERIFICATION_KEY_SIZE: usize = D::VERIFICATION_KEY_SIZE;
    const SIGNING_KEY_SIZE: usize = D::SIGNING_KEY_SIZE;
    const SIGNATURE_SIZE: usize = D::SIGNATURE_SIZE;

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
        let sig = D::sign_bytes_m(context, message, signing_key)
            .map_err(|e| KesMError::Dsign(format!("{:?}", e)))?;
        #[cfg(feature = "kes-metrics")]
        metrics::record_signature(Self::SIGNATURE_SIZE);
        Ok(sig)
    }

    fn verify_kes(
        context: &Self::Context,
        verification_key: &Self::VerificationKey,
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
        D::verify_bytes(context, verification_key, message, signature)
            .map_err(|_| KesError::VerificationFailed)
    }

    fn update_kes(
        _context: &Self::Context,
        signing_key: Self::SigningKey,
        period: Period,
    ) -> Result<Option<Self::SigningKey>, KesMError> {
        let last_period = Self::total_periods().saturating_sub(1);

        if period >= last_period {
            // Once we have signed for the final available period, the key expires.
            D::forget_signing_key_m(signing_key);
            Ok(None)
        } else {
            #[cfg(feature = "kes-metrics")]
            metrics::record_update();
            Ok(Some(signing_key))
        }
    }

    fn gen_key_kes_from_seed_bytes(seed: &[u8]) -> Result<Self::SigningKey, KesMError> {
        // Use the UnsoundDsignMAlgorithm trait which provides raw_deserialize_signing_key_m
        // This constructs an MLocked signing key directly from seed bytes
        // Note: This is marked "Unsound" because it exposes key material serialization,
        // but it's the correct way to construct keys from seed bytes
        let sk = D::raw_deserialize_signing_key_m(seed)
            .map_err(|e| KesMError::Dsign(format!("{:?}", e)))?;
        #[cfg(feature = "kes-metrics")]
        metrics::record_signing_key(Self::SIGNING_KEY_SIZE);
        Ok(sk)
    }

    fn raw_serialize_verification_key_kes(key: &Self::VerificationKey) -> Vec<u8> {
        D::raw_serialize_verification_key(key)
    }

    fn raw_deserialize_verification_key_kes(bytes: &[u8]) -> Option<Self::VerificationKey> {
        D::raw_deserialize_verification_key(bytes)
    }

    fn raw_serialize_signature_kes(signature: &Self::Signature) -> Vec<u8> {
        D::raw_serialize_signature(signature)
    }

    fn raw_deserialize_signature_kes(bytes: &[u8]) -> Option<Self::Signature> {
        D::raw_deserialize_signature(bytes)
    }

    fn forget_signing_key_kes(signing_key: Self::SigningKey) {
        D::forget_signing_key_m(signing_key);
    }
}

// Note: We don't implement UnsoundKesAlgorithm for SingleKes to maintain
// the same security properties as the Haskell implementation - signing keys
// should not be serialized.
