use ed25519_dalek::Signer;
use ed25519_dalek::SigningKey;

use crate::vendor::crypto_class::direct_serialise::{DirectDeserialise, DirectResult, DirectSerialise, SizeCheckError};
use crate::vendor::crypto_class::dsign::ed25519::{
    Ed25519, Ed25519Signature, Ed25519SigningKey, Ed25519VerificationKey, SECRET_COMPOUND_BYTES,
    SEED_BYTES, VERIFICATION_KEY_BYTES,
};
use crate::vendor::crypto_class::dsign::{DsignError, DsignMAlgorithm, DsignMError, UnsoundDsignMAlgorithm};
use crate::vendor::crypto_class::mlocked_bytes::{MLockedError, MLockedSizedBytes};
use crate::vendor::crypto_class::mlocked_seed::MLockedSeed;

/// Signing key stored in mlocked memory. Mirrors libsodium's 64-byte secret
/// key structure containing both seed and verification key.
pub struct Ed25519MLockedSigningKey(pub(crate) MLockedSizedBytes<SECRET_COMPOUND_BYTES>);

impl Ed25519MLockedSigningKey {
    fn from_seed(seed: &MLockedSeed<SEED_BYTES>) -> Result<Self, MLockedError> {
        let signing = Ed25519SigningKey::from_seed_bytes(seed.as_bytes());
        let mut compound = MLockedSizedBytes::<SECRET_COMPOUND_BYTES>::new()?;
        compound
            .as_mut_slice()
            .copy_from_slice(signing.compound_bytes());
        Ok(Self(compound))
    }

    fn seed_bytes(&self) -> [u8; SEED_BYTES] {
        let mut seed = [0u8; SEED_BYTES];
        seed.copy_from_slice(&self.0.as_slice()[..SEED_BYTES]);
        seed
    }

    fn verifying_bytes(&self) -> [u8; VERIFICATION_KEY_BYTES] {
        let mut vk = [0u8; VERIFICATION_KEY_BYTES];
        vk.copy_from_slice(&self.0.as_slice()[SEED_BYTES..]);
        vk
    }

    fn signing_key(&self) -> SigningKey {
        let seed = self.seed_bytes();
        SigningKey::from_bytes(&seed)
    }
}

impl DirectSerialise for Ed25519MLockedSigningKey {
    fn direct_serialise(
        &self,
        push: &mut dyn FnMut(&[u8]) -> DirectResult<()>,
    ) -> DirectResult<()> {
        let mut seed = self.seed_bytes();
        let result = push(&seed);
        seed.fill(0);
        result
    }
}

impl DirectDeserialise for Ed25519MLockedSigningKey {
    fn direct_deserialise(
        pull: &mut dyn FnMut(&mut [u8]) -> DirectResult<()>,
    ) -> DirectResult<Self> {
        let mut seed = MLockedSeed::<SEED_BYTES>::new_zeroed().map_err(|_| SizeCheckError {
            expected_size: SEED_BYTES,
            actual_size: 0,
        })?;
        {
            let slice = seed.as_mut_bytes();
            pull(slice)?;
        }
        let signing_key =
            Ed25519MLockedSigningKey::from_seed(&seed).map_err(|_| SizeCheckError {
                expected_size: SEED_BYTES,
                actual_size: SEED_BYTES,
            })?;
        seed.finalize();
        Ok(signing_key)
    }
}

impl DsignMAlgorithm for Ed25519 {
    type MLockedSigningKey = Ed25519MLockedSigningKey;
    type SeedMaterial = MLockedSeed<SEED_BYTES>;

    fn derive_verification_key_m(
        signing_key: &Self::MLockedSigningKey,
    ) -> Result<Self::VerificationKey, DsignMError> {
        let vk_bytes = signing_key.verifying_bytes();
        Ed25519VerificationKey::from_bytes(&vk_bytes)
            .ok_or_else(|| DsignError::Message("invalid ed25519 verification key".to_owned()))
            .map_err(Into::into)
    }

    fn sign_bytes_m(
        _context: &Self::Context,
        message: &[u8],
        signing_key: &Self::MLockedSigningKey,
    ) -> Result<Self::Signature, DsignError> {
        let signing = signing_key.signing_key();
        let signature = signing.sign(message);
        Ok(Ed25519Signature::from_dalek(&signature))
    }

    fn gen_key_m(seed: &Self::SeedMaterial) -> Result<Self::MLockedSigningKey, DsignMError> {
        Ok(Ed25519MLockedSigningKey::from_seed(seed)?)
    }

    fn clone_key_m(
        signing_key: &Self::MLockedSigningKey,
    ) -> Result<Self::MLockedSigningKey, DsignMError> {
        Ok(Ed25519MLockedSigningKey(signing_key.0.try_clone()?))
    }

    fn get_seed_m(
        signing_key: &Self::MLockedSigningKey,
    ) -> Result<Self::SeedMaterial, DsignMError> {
        let mut seed = MLockedSeed::<SEED_BYTES>::new_zeroed()?;
        seed.as_mut_bytes()
            .copy_from_slice(&signing_key.0.as_slice()[..SEED_BYTES]);
        Ok(seed)
    }

    fn forget_signing_key_m(signing_key: Self::MLockedSigningKey) {
        signing_key.0.finalize();
    }
}

impl UnsoundDsignMAlgorithm for Ed25519 {
    fn raw_serialize_signing_key_m(
        signing_key: &Self::MLockedSigningKey,
    ) -> Result<Vec<u8>, DsignMError> {
        Ok(signing_key.0.as_slice()[..SEED_BYTES].to_vec())
    }

    fn raw_deserialize_signing_key_m(bytes: &[u8]) -> Result<Self::MLockedSigningKey, DsignMError> {
        if bytes.len() != SEED_BYTES {
            return Err(DsignError::wrong_length(
                "raw_deserialize_signing_key_m",
                SEED_BYTES,
                bytes.len(),
            )
            .into());
        }
        let mut seed = MLockedSeed::<SEED_BYTES>::new_zeroed()?;
        seed.as_mut_bytes().copy_from_slice(bytes);
        let signing = Ed25519MLockedSigningKey::from_seed(&seed)?;
        seed.finalize();
        Ok(signing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vendor::crypto_class::dsign::{signed_dsign_m, verify_signed_dsign};
    use crate::vendor::crypto_class::mlocked_seed::MLockedSeed;

    #[test]
    fn mlocked_sign_and_verify() {
        let mut seed = MLockedSeed::<SEED_BYTES>::new_zeroed().unwrap();
        seed.as_mut_bytes().copy_from_slice(&[5u8; SEED_BYTES]);
        let signing = <Ed25519 as DsignMAlgorithm>::gen_key_m(&seed).unwrap();
        let verifying = <Ed25519 as DsignMAlgorithm>::derive_verification_key_m(&signing).unwrap();
        let message = b"cardano";
        let signed = signed_dsign_m::<Ed25519, _>(&(), message, &signing).unwrap();
        assert!(verify_signed_dsign::<Ed25519, _>(&(), &verifying, message, &signed).is_ok());
        <Ed25519 as DsignMAlgorithm>::forget_signing_key_m(signing);
        seed.finalize();
    }
}
