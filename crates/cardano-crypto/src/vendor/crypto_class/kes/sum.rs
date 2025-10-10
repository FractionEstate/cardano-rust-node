use std::marker::PhantomData;

use crate::vendor::crypto_class::direct_serialise::{
    DirectDeserialise, DirectResult, DirectSerialise,
};
use crate::vendor::crypto_class::kes::hash::KesHashAlgorithm;
use crate::vendor::crypto_class::kes::{KesAlgorithm, KesError, KesMError, Period};
use crate::vendor::crypto_class::mlocked_bytes::MLockedBytes;
use crate::vendor::crypto_class::seed::Seed;

/// SumKES composes two KES schemes to create a scheme with double the periods.
///
/// This implements the binary sum composition from Section 3.1 of the MMM paper
/// "Composition and Efficiency Tradeoffs for Forward-Secure Digital Signatures".
///
/// The signing key contains:
/// - sk_0: signing key for the first half of periods
/// - r_1: seed for generating sk_1 (second half)
/// - vk_0, vk_1: verification keys for both halves
///
/// The verification key is: H(vk_0 || vk_1) where H is the hash algorithm parameter.
pub struct SumKes<D, H>(PhantomData<(D, H)>)
where
    D: KesAlgorithm,
    H: KesHashAlgorithm;

/// Signing key for SumKES contains both constituent keys.
pub struct SumSigningKey<D, H>
where
    D: KesAlgorithm,
    H: KesHashAlgorithm,
{
    /// Current signing key (either for left or right subtree)
    pub(crate) sk: D::SigningKey,
    /// Seed for the right subtree (mlocked)
    pub(crate) r1_seed: Option<MLockedBytes>,
    /// Verification key for left subtree
    pub(crate) vk0: D::VerificationKey,
    /// Verification key for right subtree
    pub(crate) vk1: D::VerificationKey,
    _phantom: PhantomData<H>,
}

/// Signature for SumKES includes constituent signature and both verification keys.
#[derive(Clone)]
pub struct SumSignature<D, H>
where
    D: KesAlgorithm,
    H: KesHashAlgorithm,
{
    pub(crate) sigma: D::Signature,
    pub(crate) vk0: D::VerificationKey,
    pub(crate) vk1: D::VerificationKey,
    _phantom: PhantomData<H>,
}

// Implement PartialEq and Eq manually since we need them for testing
impl<D, H> PartialEq for SumSignature<D, H>
where
    D: KesAlgorithm,
    D::Signature: PartialEq,
    D::VerificationKey: PartialEq,
    H: KesHashAlgorithm,
{
    fn eq(&self, other: &Self) -> bool {
        self.sigma == other.sigma && self.vk0 == other.vk0 && self.vk1 == other.vk1
    }
}

impl<D, H> Eq for SumSignature<D, H>
where
    D: KesAlgorithm,
    D::Signature: Eq,
    D::VerificationKey: Eq,
    H: KesHashAlgorithm,
{
}

// Implement Debug for better test output
impl<D, H> std::fmt::Debug for SumSignature<D, H>
where
    D: KesAlgorithm,
    D::Signature: std::fmt::Debug,
    D::VerificationKey: std::fmt::Debug,
    H: KesHashAlgorithm,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SumSignature")
            .field("sigma", &self.sigma)
            .field("vk0", &self.vk0)
            .field("vk1", &self.vk1)
            .finish()
    }
}

impl<D, H> KesAlgorithm for SumKes<D, H>
where
    D: KesAlgorithm,
    D::VerificationKey: Clone,
    H: KesHashAlgorithm,
{
    type VerificationKey = Vec<u8>; // Hash of (vk0, vk1)
    type SigningKey = SumSigningKey<D, H>;
    type Signature = SumSignature<D, H>;
    type Context = D::Context;

    const ALGORITHM_NAME: &'static str = D::ALGORITHM_NAME; // Could append "_sum"
    const SEED_SIZE: usize = D::SEED_SIZE;
    const VERIFICATION_KEY_SIZE: usize = H::OUTPUT_SIZE; // Now parameterized by hash!
    const SIGNING_KEY_SIZE: usize =
        D::SIGNING_KEY_SIZE + D::SEED_SIZE + 2 * D::VERIFICATION_KEY_SIZE;
    const SIGNATURE_SIZE: usize = D::SIGNATURE_SIZE + 2 * D::VERIFICATION_KEY_SIZE;

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

        let sigma = if period < t_half {
            // Use left subtree (sk_0)
            D::sign_kes(context, period, message, &signing_key.sk)?
        } else {
            // Use right subtree (sk_1)
            D::sign_kes(context, period - t_half, message, &signing_key.sk)?
        };

        Ok(SumSignature {
            sigma,
            vk0: signing_key.vk0.clone(),
            vk1: signing_key.vk1.clone(),
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
        // Verify that H(vk0 || vk1) matches the provided verification key
        let vk0_bytes = D::raw_serialize_verification_key_kes(&signature.vk0);
        let vk1_bytes = D::raw_serialize_verification_key_kes(&signature.vk1);
        let computed_vk = H::hash_concat(&vk0_bytes, &vk1_bytes);

        if &computed_vk != verification_key {
            return Err(KesError::VerificationFailed);
        }

        let t_half = D::total_periods();

        if period < t_half {
            // Verify against left subtree
            D::verify_kes(context, &signature.vk0, period, message, &signature.sigma)
        } else {
            // Verify against right subtree
            D::verify_kes(
                context,
                &signature.vk1,
                period - t_half,
                message,
                &signature.sigma,
            )
        }
    }

    fn update_kes(
        context: &Self::Context,
        mut signing_key: Self::SigningKey,
        period: Period,
    ) -> Result<Option<Self::SigningKey>, KesMError> {
        let t_half = D::total_periods();

        if period + 1 >= 2 * t_half {
            // Key has expired
            D::forget_signing_key_kes(signing_key.sk);
            return Ok(None);
        }

        if period + 1 == t_half {
            // Transition from left to right subtree
            // Generate sk_1 from r1_seed
            let r1_seed = signing_key
                .r1_seed
                .take()
                .ok_or(KesMError::Kes(KesError::KeyExpired))?;

            let seed = Seed::from_bytes(r1_seed.as_slice());
            let sk1 = D::gen_key_kes(&seed)?;

            // Forget the old signing key
            D::forget_signing_key_kes(signing_key.sk);

            Ok(Some(SumSigningKey {
                sk: sk1,
                r1_seed: None, // Already used
                vk0: signing_key.vk0,
                vk1: signing_key.vk1,
                _phantom: PhantomData,
            }))
        } else if period + 1 < t_half {
            // Still in left subtree, update sk_0
            let updated_sk = D::update_kes(context, signing_key.sk, period)?;
            match updated_sk {
                Some(sk) => Ok(Some(SumSigningKey {
                    sk,
                    r1_seed: signing_key.r1_seed,
                    vk0: signing_key.vk0,
                    vk1: signing_key.vk1,
                    _phantom: PhantomData,
                })),
                None => Ok(None),
            }
        } else {
            // In right subtree, update sk_1
            let adjusted_period = period - t_half;
            let updated_sk = D::update_kes(context, signing_key.sk, adjusted_period)?;
            match updated_sk {
                Some(sk) => Ok(Some(SumSigningKey {
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
        let (r0_hash, r1_hash) = H::expand_seed(seed);
        let r0_bytes = &r0_hash[..D::SEED_SIZE.min(r0_hash.len())];
        let r1_bytes = &r1_hash[..D::SEED_SIZE.min(r1_hash.len())];

        // Generate sk_0 from r0
        let sk0 = D::gen_key_kes_from_seed_bytes(r0_bytes)?;
        let vk0 = D::derive_verification_key(&sk0)?;

        // Generate sk_1 from r1 (only to derive vk1, then forget)
        let sk1 = D::gen_key_kes_from_seed_bytes(r1_bytes)?;
        let vk1 = D::derive_verification_key(&sk1)?;
        D::forget_signing_key_kes(sk1);

        // Store r1 in mlocked memory for later
        let mut r1_mlocked = MLockedBytes::new(r1_bytes.len())?;
        r1_mlocked.as_mut_slice().copy_from_slice(r1_bytes);

        Ok(SumSigningKey {
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
        result.extend_from_slice(&D::raw_serialize_verification_key_kes(&signature.vk0));
        result.extend_from_slice(&D::raw_serialize_verification_key_kes(&signature.vk1));
        result
    }

    fn raw_deserialize_signature_kes(bytes: &[u8]) -> Option<Self::Signature> {
        if bytes.len() != Self::SIGNATURE_SIZE {
            return None;
        }

        let sig_bytes = &bytes[0..D::SIGNATURE_SIZE];
        let vk0_offset = D::SIGNATURE_SIZE;
        let vk1_offset = vk0_offset + D::VERIFICATION_KEY_SIZE;

        let sigma = D::raw_deserialize_signature_kes(sig_bytes)?;
        let vk0 = D::raw_deserialize_verification_key_kes(&bytes[vk0_offset..vk1_offset])?;
        let vk1 = D::raw_deserialize_verification_key_kes(&bytes[vk1_offset..])?;

        Some(SumSignature {
            sigma,
            vk0,
            vk1,
            _phantom: PhantomData,
        })
    }

    fn forget_signing_key_kes(signing_key: Self::SigningKey) {
        D::forget_signing_key_kes(signing_key.sk);
        // r1_seed will be dropped and zeroized automatically
    }
}

// Serde implementations for SumKES types
#[cfg(feature = "serde")]
impl<D, H> serde::Serialize for SumSignature<D, H>
where
    D: KesAlgorithm,
    D::Signature: serde::Serialize,
    D::VerificationKey: serde::Serialize,
    H: KesHashAlgorithm,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeTuple;
        let mut tuple = serializer.serialize_tuple(3)?;
        tuple.serialize_element(&self.sigma)?;
        tuple.serialize_element(&self.vk0)?;
        tuple.serialize_element(&self.vk1)?;
        tuple.end()
    }
}

#[cfg(feature = "serde")]
impl<'de, D, H> serde::Deserialize<'de> for SumSignature<D, H>
where
    D: KesAlgorithm,
    D::Signature: serde::Deserialize<'de>,
    D::VerificationKey: serde::Deserialize<'de>,
    H: KesHashAlgorithm,
{
    fn deserialize<DE>(deserializer: DE) -> Result<Self, DE::Error>
    where
        DE: serde::Deserializer<'de>,
    {
        use serde::de::{self, SeqAccess, Visitor};

        struct SumSignatureVisitor<D, H>(PhantomData<(D, H)>);

        impl<'de, D, H> Visitor<'de> for SumSignatureVisitor<D, H>
        where
            D: KesAlgorithm,
            D::Signature: serde::Deserialize<'de>,
            D::VerificationKey: serde::Deserialize<'de>,
            H: KesHashAlgorithm,
        {
            type Value = SumSignature<D, H>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a SumKES signature tuple (sigma, vk0, vk1)")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let sigma = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let vk0 = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let vk1 = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(2, &self))?;

                Ok(SumSignature {
                    sigma,
                    vk0,
                    vk1,
                    _phantom: PhantomData,
                })
            }
        }

        deserializer.deserialize_tuple(3, SumSignatureVisitor(PhantomData))
    }
}

// Type aliases for nested compositions
use crate::vendor::crypto_class::dsign::ed25519::Ed25519;
use crate::vendor::crypto_class::kes::hash::Blake2b256;
use crate::vendor::crypto_class::kes::single::SingleKes;

/// Base case: SingleKES wrapping Ed25519
pub type Sum0Kes = SingleKes<Ed25519>;

/// 2^1 = 2 periods (using Blake2b-256 to match Haskell)
pub type Sum1Kes = SumKes<Sum0Kes, Blake2b256>;

/// 2^2 = 4 periods
pub type Sum2Kes = SumKes<Sum1Kes, Blake2b256>;

/// 2^3 = 8 periods
pub type Sum3Kes = SumKes<Sum2Kes, Blake2b256>;

/// 2^4 = 16 periods
pub type Sum4Kes = SumKes<Sum3Kes, Blake2b256>;

/// 2^5 = 32 periods
pub type Sum5Kes = SumKes<Sum4Kes, Blake2b256>;

/// 2^6 = 64 periods
pub type Sum6Kes = SumKes<Sum5Kes, Blake2b256>;

/// 2^7 = 128 periods (standard Cardano KES)
pub type Sum7Kes = SumKes<Sum6Kes, Blake2b256>;

// DirectSerialise implementation for SumSigningKey
//
// Following the Haskell pattern, we recursively serialize:
// 1. Child signing key (sk)
// 2. MLocked seed for right subtree (r1_seed)
// 3. Verification key for left subtree (vk0)
// 4. Verification key for right subtree (vk1)
impl<D, H> DirectSerialise for SumSigningKey<D, H>
where
    D: KesAlgorithm,
    D::SigningKey: DirectSerialise,
    D::VerificationKey: DirectSerialise,
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
            // If r1_seed is None, we still need to serialize empty bytes
            // This should not happen in a valid key, but we handle it for safety
            // Actually, based on Haskell, if r1_seed is None it means we're in right subtree
            // and the seed has been consumed. We should serialize zeroes or error.
            // For now, let's serialize the expected number of zero bytes.
            let zero_bytes = vec![0u8; D::SEED_SIZE];
            push(&zero_bytes)?;
        }

        // Serialize verification keys
        self.vk0.direct_serialise(push)?;
        self.vk1.direct_serialise(push)?;

        Ok(())
    }
}

impl<D, H> DirectDeserialise for SumSigningKey<D, H>
where
    D: KesAlgorithm,
    D::SigningKey: DirectDeserialise,
    D::VerificationKey: DirectDeserialise,
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

        Ok(SumSigningKey {
            sk,
            r1_seed: Some(r1_mlocked),
            vk0,
            vk1,
            _phantom: PhantomData,
        })
    }
}
