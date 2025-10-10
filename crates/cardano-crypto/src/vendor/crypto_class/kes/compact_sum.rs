use std::marker::PhantomData;

use crate::vendor::crypto_class::direct_serialise::{DirectDeserialise, DirectResult, DirectSerialise};
use crate::vendor::crypto_class::kes::compact_single::OptimizedKesSignature;
use crate::vendor::crypto_class::kes::hash::KesHashAlgorithm;
use crate::vendor::crypto_class::kes::{KesAlgorithm, KesError, KesMError, Period};
use crate::vendor::crypto_class::mlocked_bytes::MLockedBytes;
use crate::vendor::crypto_class::seed::Seed;

/// CompactSumKES is an optimized version of SumKES that stores fewer verification keys.
///
/// The key insight is that in a Merkle tree structure, each branch node only needs
/// to store ONE verification key instead of TWO. The signature embeds the "off-side"
/// verification key, and we can reconstruct the "on-side" key from the signature.
///
/// For example, in a tree with current period at leaf E:
/// ```text
///       (A)
///      /   \
///   (B)     (C)
///   / \     / \
/// (D) (E) (F) (G)
///      ^
///  0   1   2   3
/// ```
///
/// The signature for E contains its DSIGN key. The signature for B contains E's
/// signature and D's VerKey. The signature for A contains B's signature and C's VerKey.
///
/// This reduces storage from depth*2 keys to just depth keys.
pub struct CompactSumKes<D, H>(PhantomData<(D, H)>)
where
    D: KesAlgorithm,
    D::Signature: OptimizedKesSignature,
    H: KesHashAlgorithm;

/// Signing key for CompactSumKES.
pub struct CompactSumSigningKey<D, H>
where
    D: KesAlgorithm,
    D::Signature: OptimizedKesSignature,
    H: KesHashAlgorithm,
{
    pub(crate) sk: D::SigningKey,
    pub(crate) r1_seed: Option<MLockedBytes>,
    pub(crate) vk0: D::VerificationKey,
    pub(crate) vk1: D::VerificationKey,
    _phantom: PhantomData<H>,
}

/// Signature for CompactSumKES - only stores the "other" verification key.
pub struct CompactSumSignature<D, H>
where
    D: KesAlgorithm,
    D::Signature: OptimizedKesSignature,
    H: KesHashAlgorithm,
{
    /// Signature from the active subtree (contains embedded vk)
    pub(crate) sigma: D::Signature,
    /// Verification key for the inactive subtree
    pub(crate) vk_other: D::VerificationKey,
    _phantom: PhantomData<H>,
}

/// Helper trait used to recover the verification key associated with a compact subtree
/// for any supported KES algorithm. CompactSum verification stitches these keys back
/// together when recomputing higher-level verification hashes.
pub trait CompactKesComponents: KesAlgorithm {
    fn active_verification_key_from_signature(
        signature: &Self::Signature,
        period: Period,
    ) -> Self::VerificationKey;
}

impl<D, H> Clone for CompactSumSignature<D, H>
where
    D: KesAlgorithm,
    D::Signature: OptimizedKesSignature + Clone,
    D::VerificationKey: Clone,
    H: KesHashAlgorithm,
{
    fn clone(&self) -> Self {
        Self {
            sigma: self.sigma.clone(),
            vk_other: self.vk_other.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<D, H> OptimizedKesSignature for CompactSumSignature<D, H>
where
    D: KesAlgorithm,
    D::VerificationKey: Clone,
    D::Signature: OptimizedKesSignature,
    H: KesHashAlgorithm,
{
    type VerificationKey = <D::Signature as OptimizedKesSignature>::VerificationKey;

    fn extract_verification_key(&self) -> &Self::VerificationKey {
        self.sigma.extract_verification_key()
    }
}

impl<D, H> KesAlgorithm for CompactSumKes<D, H>
where
    D: KesAlgorithm + CompactKesComponents,
    D::VerificationKey: Clone,
    D::Signature: OptimizedKesSignature + Clone,
    H: KesHashAlgorithm,
{
    type VerificationKey = Vec<u8>; // Hash of (vk0, vk1)
    type SigningKey = CompactSumSigningKey<D, H>;
    type Signature = CompactSumSignature<D, H>;
    type Context = D::Context;

    const ALGORITHM_NAME: &'static str = D::ALGORITHM_NAME; // Could append "_compact"
    const SEED_SIZE: usize = D::SEED_SIZE;
    const VERIFICATION_KEY_SIZE: usize = H::OUTPUT_SIZE; // Now parameterized!
    const SIGNING_KEY_SIZE: usize =
        D::SIGNING_KEY_SIZE + D::SEED_SIZE + 2 * D::VERIFICATION_KEY_SIZE;
    // Compact signature: constituent signature + only ONE verification key
    const SIGNATURE_SIZE: usize = D::SIGNATURE_SIZE + D::VERIFICATION_KEY_SIZE;

    fn total_periods() -> Period {
        2 * D::total_periods()
    }

    fn derive_verification_key(
        signing_key: &Self::SigningKey,
    ) -> Result<Self::VerificationKey, KesMError> {
        // vk = H(vk0 || vk1)
        let vk0_bytes = D::raw_serialize_verification_key_kes(&signing_key.vk0);
        let vk1_bytes = D::raw_serialize_verification_key_kes(&signing_key.vk1);
        Ok(H::hash_concat(&vk0_bytes, &vk1_bytes))
    }

    fn sign_kes(
        context: &Self::Context,
        period: Period,
        message: &[u8],
        signing_key: &Self::SigningKey,
    ) -> Result<Self::Signature, KesMError> {
        let t_half = D::total_periods();

        let (sigma, vk_other) = if period < t_half {
            // Use left subtree, store right vk
            let sig = D::sign_kes(context, period, message, &signing_key.sk)?;
            (sig, signing_key.vk1.clone())
        } else {
            // Use right subtree, store left vk
            let sig = D::sign_kes(context, period - t_half, message, &signing_key.sk)?;
            (sig, signing_key.vk0.clone())
        };

        Ok(CompactSumSignature {
            sigma,
            vk_other,
            _phantom: PhantomData,
        })
    }

    fn verify_kes(
        context: &Self::Context,
        verification_key: &Self::VerificationKey,
        period: Period,
        message: &[u8],
        signature: &Self::Signature,
    ) -> Result<(), KesError> {
        let t_half = D::total_periods();
        let active_is_left = period < t_half;
        let child_period = if active_is_left {
            period
        } else {
            period - t_half
        };

        // Derive the active subtree verification key (owned value so we can reuse it)
        let vk_active = <D as CompactKesComponents>::active_verification_key_from_signature(
            &signature.sigma,
            child_period,
        );
        let vk_other = signature.vk_other.clone();

        // Reconstruct both vk0 and vk1
        let (vk0, vk1) = if active_is_left {
            // Active is left, other is right
            (vk_active.clone(), vk_other.clone())
        } else {
            // Active is right, other is left
            (vk_other.clone(), vk_active.clone())
        };

        // Verify that H(vk0 || vk1) matches the provided verification key
        let vk0_bytes = D::raw_serialize_verification_key_kes(&vk0);
        let vk1_bytes = D::raw_serialize_verification_key_kes(&vk1);
        let computed_vk = H::hash_concat(&vk0_bytes, &vk1_bytes);

        if &computed_vk != verification_key {
            return Err(KesError::VerificationFailed);
        }

        // Verify the signature against the active verification key
        D::verify_kes(context, &vk_active, child_period, message, &signature.sigma)
    }

    fn update_kes(
        context: &Self::Context,
        mut signing_key: Self::SigningKey,
        period: Period,
    ) -> Result<Option<Self::SigningKey>, KesMError> {
        let t_half = D::total_periods();

        if period + 1 >= 2 * t_half {
            D::forget_signing_key_kes(signing_key.sk);
            return Ok(None);
        }

        if period + 1 == t_half {
            // Transition from left to right subtree
            let r1_seed = signing_key
                .r1_seed
                .take()
                .ok_or(KesMError::Kes(KesError::KeyExpired))?;

            let seed = Seed::from_bytes(r1_seed.as_slice());
            let sk1 = D::gen_key_kes(&seed)?;

            D::forget_signing_key_kes(signing_key.sk);

            Ok(Some(CompactSumSigningKey {
                sk: sk1,
                r1_seed: None,
                vk0: signing_key.vk0,
                vk1: signing_key.vk1,
                _phantom: PhantomData,
            }))
        } else if period + 1 < t_half {
            // Still in left subtree
            let updated_sk = D::update_kes(context, signing_key.sk, period)?;
            match updated_sk {
                Some(sk) => Ok(Some(CompactSumSigningKey {
                    sk,
                    r1_seed: signing_key.r1_seed,
                    vk0: signing_key.vk0,
                    vk1: signing_key.vk1,
                    _phantom: PhantomData,
                })),
                None => Ok(None),
            }
        } else {
            // In right subtree
            let adjusted_period = period - t_half;
            let updated_sk = D::update_kes(context, signing_key.sk, adjusted_period)?;
            match updated_sk {
                Some(sk) => Ok(Some(CompactSumSigningKey {
                    sk,
                    r1_seed: None,
                    vk0: signing_key.vk0,
                    vk1: signing_key.vk1,
                    _phantom: PhantomData,
                })),
                None => Ok(None),
            }
        }
    }

    fn gen_key_kes_from_seed_bytes(seed: &[u8]) -> Result<Self::SigningKey, KesMError> {
        // Split seed into r0 and r1 using the hash algorithm
        let (r0_bytes, r1_bytes) = H::expand_seed(seed);

        // Generate sk_0 from r0
        let sk0 = D::gen_key_kes_from_seed_bytes(&r0_bytes)?;
        let vk0 = D::derive_verification_key(&sk0)?;

        // Generate sk_1 from r1 (only to derive vk1, then forget)
        let sk1 = D::gen_key_kes_from_seed_bytes(&r1_bytes)?;
        let vk1 = D::derive_verification_key(&sk1)?;
        D::forget_signing_key_kes(sk1);

        // Store r1 in mlocked memory
        let mut r1_mlocked = MLockedBytes::new(r1_bytes.len())?;
        r1_mlocked.as_mut_slice().copy_from_slice(&r1_bytes);

        Ok(CompactSumSigningKey {
            sk: sk0,
            r1_seed: Some(r1_mlocked),
            vk0,
            vk1,
            _phantom: PhantomData,
        })
    }

    fn raw_serialize_verification_key_kes(key: &Self::VerificationKey) -> Vec<u8> {
        key.clone()
    }

    fn raw_deserialize_verification_key_kes(bytes: &[u8]) -> Option<Self::VerificationKey> {
        if bytes.len() == Self::VERIFICATION_KEY_SIZE {
            Some(bytes.to_vec())
        } else {
            None
        }
    }

    fn raw_serialize_signature_kes(signature: &Self::Signature) -> Vec<u8> {
        let mut result = D::raw_serialize_signature_kes(&signature.sigma);
        result.extend_from_slice(&D::raw_serialize_verification_key_kes(&signature.vk_other));
        result
    }

    fn raw_deserialize_signature_kes(bytes: &[u8]) -> Option<Self::Signature> {
        if bytes.len() != Self::SIGNATURE_SIZE {
            return None;
        }

        let sig_bytes = &bytes[0..D::SIGNATURE_SIZE];
        let vk_bytes = &bytes[D::SIGNATURE_SIZE..];

        let sigma = D::raw_deserialize_signature_kes(sig_bytes)?;
        let vk_other = D::raw_deserialize_verification_key_kes(vk_bytes)?;

        Some(CompactSumSignature {
            sigma,
            vk_other,
            _phantom: PhantomData,
        })
    }

    fn forget_signing_key_kes(signing_key: Self::SigningKey) {
        D::forget_signing_key_kes(signing_key.sk);
    }
}

// Type aliases for nested CompactSum compositions
use crate::vendor::crypto_class::dsign::ed25519::Ed25519;
use crate::vendor::crypto_class::kes::compact_single::CompactSingleKes;
use crate::vendor::crypto_class::kes::hash::Blake2b256;

/// Base case: CompactSingleKES wrapping Ed25519
pub type CompactSum0Kes = CompactSingleKes<Ed25519>;

/// 2^1 = 2 periods (compact)
pub type CompactSum1Kes = CompactSumKes<CompactSum0Kes, Blake2b256>;

/// 2^2 = 4 periods (compact)
pub type CompactSum2Kes = CompactSumKes<CompactSum1Kes, Blake2b256>;

/// 2^3 = 8 periods (compact)
pub type CompactSum3Kes = CompactSumKes<CompactSum2Kes, Blake2b256>;

/// 2^4 = 16 periods (compact)
pub type CompactSum4Kes = CompactSumKes<CompactSum3Kes, Blake2b256>;

/// 2^5 = 32 periods (compact)
pub type CompactSum5Kes = CompactSumKes<CompactSum4Kes, Blake2b256>;

/// 2^6 = 64 periods (compact)
pub type CompactSum6Kes = CompactSumKes<CompactSum5Kes, Blake2b256>;

/// 2^7 = 128 periods (compact, standard Cardano KES)
pub type CompactSum7Kes = CompactSumKes<CompactSum6Kes, Blake2b256>;

impl CompactKesComponents for CompactSum0Kes {
    fn active_verification_key_from_signature(
        signature: &Self::Signature,
        _period: Period,
    ) -> Self::VerificationKey {
        signature.extract_verification_key().clone()
    }
}

impl<D, H> CompactKesComponents for CompactSumKes<D, H>
where
    D: KesAlgorithm + CompactKesComponents,
    D::VerificationKey: Clone,
    D::Signature: OptimizedKesSignature + Clone,
    H: KesHashAlgorithm,
{
    fn active_verification_key_from_signature(
        signature: &Self::Signature,
        period: Period,
    ) -> Self::VerificationKey {
        let t_half = D::total_periods();
        let active_is_left = period < t_half;
        let child_period = if active_is_left {
            period
        } else {
            period - t_half
        };

        let vk_active = <D as CompactKesComponents>::active_verification_key_from_signature(
            &signature.sigma,
            child_period,
        );
        let vk_other = signature.vk_other.clone();

        let (vk_left, vk_right) = if active_is_left {
            (vk_active.clone(), vk_other)
        } else {
            (vk_other.clone(), vk_active.clone())
        };

        let left_bytes = D::raw_serialize_verification_key_kes(&vk_left);
        let right_bytes = D::raw_serialize_verification_key_kes(&vk_right);
        H::hash_concat(&left_bytes, &right_bytes)
    }
}

// DirectSerialise implementation for CompactSumSigningKey
//
// Following the Haskell pattern, we recursively serialize:
// 1. Child signing key (sk)
// 2. MLocked seed for right subtree (r1_seed)
// 3. Verification key for left subtree (vk0)
// 4. Verification key for right subtree (vk1)
impl<D, H> DirectSerialise for CompactSumSigningKey<D, H>
where
    D: KesAlgorithm,
    D::SigningKey: DirectSerialise,
    D::VerificationKey: DirectSerialise,
    D::Signature: OptimizedKesSignature,
    H: KesHashAlgorithm,
{
    fn direct_serialise(
        &self,
        push: &mut dyn FnMut(&[u8]) -> DirectResult<()>,
    ) -> DirectResult<()> {
        // Serialize child signing key
        self.sk.direct_serialise(push)?;

        // Serialize r1_seed (mlocked seed for right subtree)
        if let Some(ref r1_seed) = self.r1_seed {
            let slice = r1_seed.as_slice();
            push(slice)?;
        } else {
            // If r1_seed is None, serialize zero bytes
            let zero_bytes = vec![0u8; D::SEED_SIZE];
            push(&zero_bytes)?;
        }

        // Serialize verification keys
        self.vk0.direct_serialise(push)?;
        self.vk1.direct_serialise(push)?;

        Ok(())
    }
}

impl<D, H> DirectDeserialise for CompactSumSigningKey<D, H>
where
    D: KesAlgorithm,
    D::SigningKey: DirectDeserialise,
    D::VerificationKey: DirectDeserialise,
    D::Signature: OptimizedKesSignature,
    H: KesHashAlgorithm,
{
    fn direct_deserialise(
        pull: &mut dyn FnMut(&mut [u8]) -> DirectResult<()>,
    ) -> DirectResult<Self> {
        // Deserialize child signing key
        let sk = D::SigningKey::direct_deserialise(pull)?;

        // Deserialize r1_seed into MLocked memory
        let mut r1_mlocked = MLockedBytes::new(D::SEED_SIZE).map_err(|_| {
            crate::vendor::crypto_class::direct_serialise::SizeCheckError {
                expected_size: D::SEED_SIZE,
                actual_size: 0,
            }
        })?;
        {
            let slice = r1_mlocked.as_mut_slice();
            pull(slice)?;
        }

        // Deserialize verification keys
        let vk0 = D::VerificationKey::direct_deserialise(pull)?;
        let vk1 = D::VerificationKey::direct_deserialise(pull)?;

        Ok(CompactSumSigningKey {
            sk,
            r1_seed: Some(r1_mlocked),
            vk0,
            vk1,
            _phantom: PhantomData,
        })
    }
}
