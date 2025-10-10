//! CompactSum7KES-backed implementation for Cardano KES keys.

use crate::{CryptoError, Result};
use cardano_crypto_class::direct_serialise::{
    direct_deserialise_buf_checked, direct_serialise_buf_checked,
};
use cardano_crypto_class::kes::{CompactSum7Kes, KesAlgorithm, KesError, KesMError};
use cardano_crypto_class::seed::Seed;
use hex::{decode as hex_decode, encode as hex_encode};
use minicbor::{decode::Decoder, encode::Encoder};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const KES_SIGNING_KEY_TEXT_TYPE: &str = "KesSigningKey_ed25519_kes_2^7";
const KES_SIGNING_KEY_DESCRIPTION: &str = "KES Signing Key";
const KES_VERIFICATION_KEY_TEXT_TYPE: &str = "KesVerificationKey_ed25519_kes_2^7";
const KES_VERIFICATION_KEY_DESCRIPTION: &str = "KES Verification Key";

/// Alias for the upstream compact sum KES algorithm with 128 periods.
type KesAlgorithm128 = CompactSum7Kes;

/// TextEnvelope JSON wrapper used by `cardano-cli` for key material.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEnvelope {
    #[serde(rename = "type")]
    pub type_: String,
    pub description: String,
    pub cbor_hex: String,
}

/// KES verification key wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KesPublicKey {
    key: <KesAlgorithm128 as KesAlgorithm>::VerificationKey,
}

impl KesPublicKey {
    /// Number of bytes in a serialized verification key.
    pub const SIZE: usize = <KesAlgorithm128 as KesAlgorithm>::VERIFICATION_KEY_SIZE;

    /// Create from raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let key = <KesAlgorithm128 as KesAlgorithm>::raw_deserialize_verification_key_kes(bytes)
            .ok_or(CryptoError::InvalidPublicKey)?;
        Ok(Self { key })
    }

    /// Serialize to raw bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        <KesAlgorithm128 as KesAlgorithm>::raw_serialize_verification_key_kes(&self.key)
    }

    /// Maximum KES period supported by this verification key.
    pub const fn max_period(&self) -> u64 {
        KesSecretKey::MAX_PERIOD
    }

    /// Verify a KES signature for the provided message and period.
    pub fn verify(&self, period: u64, message: &[u8], signature: &KesSignature) -> Result<()> {
        if period != signature.period {
            return Err(CryptoError::KesPeriodMismatch);
        }

        let signature_inner = signature.as_algorithm_signature()?;

        <KesAlgorithm128 as KesAlgorithm>::verify_kes(
            &(),
            &self.key,
            period,
            message,
            &signature_inner,
        )
        .map_err(map_kes_error)?;

        Ok(())
    }

    /// Load a verification key from a `cardano-cli` TextEnvelope file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let envelope = read_text_envelope(path)?;
        if envelope.type_ != KES_VERIFICATION_KEY_TEXT_TYPE {
            return Err(CryptoError::InvalidSignature(format!(
                "unexpected TextEnvelope type: expected '{}', got '{}'",
                KES_VERIFICATION_KEY_TEXT_TYPE, envelope.type_
            )));
        }

        let key_bytes = decode_text_envelope_bytes(&envelope.cbor_hex)?;
        Self::from_bytes(&key_bytes)
    }

    /// Persist the verification key to a TextEnvelope-compatible JSON string.
    pub fn to_text_envelope_json(&self) -> Result<String> {
        let mut encoder = Encoder::new(Vec::new());
        let key_bytes = self.to_bytes();
        encoder.bytes(&key_bytes).map_err(|e| {
            CryptoError::InvalidSignature(format!(
                "failed to encode KES verification key to CBOR: {e}"
            ))
        })?;
        let cbor_hex = hex_encode(encoder.into_writer());
        let envelope = TextEnvelope {
            type_: KES_VERIFICATION_KEY_TEXT_TYPE.to_string(),
            description: KES_VERIFICATION_KEY_DESCRIPTION.to_string(),
            cbor_hex,
        };
        serde_json::to_string_pretty(&envelope).map_err(|e| {
            CryptoError::InvalidSignature(format!(
                "failed to serialize KES verification key envelope: {e}"
            ))
        })
    }
}

/// KES signing key wrapper.
pub struct KesSecretKey {
    signing_key: Option<<KesAlgorithm128 as KesAlgorithm>::SigningKey>,
    current_period: u64,
}

impl KesSecretKey {
    /// Total number of supported KES periods.
    pub fn total_periods() -> u64 {
        <KesAlgorithm128 as KesAlgorithm>::total_periods()
    }

    /// Highest valid period index (0-based) before the key expires.
    pub const MAX_PERIOD: u64 = 128 - 1; // CompactSum7Kes supports 128 periods

    /// Generate a fresh signing key at period 0 using secure random entropy.
    pub fn generate() -> Result<Self> {
        let mut seed_bytes = vec![0u8; <KesAlgorithm128 as KesAlgorithm>::SEED_SIZE];
        OsRng.try_fill_bytes(&mut seed_bytes).map_err(|e| {
            CryptoError::InvalidSignature(format!("failed to gather entropy for KES key: {e}"))
        })?;
        Self::from_seed(&seed_bytes, 0)
    }

    /// Construct a signing key from a deterministic seed and evolve it to the
    /// provided starting period. This is primarily intended for testing.
    pub fn from_seed(seed_bytes: &[u8], start_period: u64) -> Result<Self> {
        if seed_bytes.len() != <KesAlgorithm128 as KesAlgorithm>::SEED_SIZE {
            return Err(CryptoError::InvalidKeyLength);
        }

        let seed = Seed::from_bytes(seed_bytes.to_vec());
        let signing_key =
            <KesAlgorithm128 as KesAlgorithm>::gen_key_kes(&seed).map_err(map_kes_m_error)?;

        let mut key = Self {
            signing_key: Some(signing_key),
            current_period: 0,
        };

        if start_period > 0 {
            key.evolve_in_place(start_period)?;
        }

        Ok(key)
    }

    /// Load a signing key from a TextEnvelope file produced by `cardano-cli`.
    pub fn from_file<P: AsRef<Path>>(path: P, current_period: u64) -> Result<Self> {
        let envelope = read_text_envelope(path)?;
        if envelope.type_ != KES_SIGNING_KEY_TEXT_TYPE {
            return Err(CryptoError::InvalidSignature(format!(
                "unexpected TextEnvelope type: expected '{}', got '{}'",
                KES_SIGNING_KEY_TEXT_TYPE, envelope.type_
            )));
        }

        let key_bytes = decode_text_envelope_bytes(&envelope.cbor_hex)?;
        Self::from_bytes(&key_bytes, current_period)
    }

    /// Deserialize a signing key from raw bytes (expected to match
    /// `CompactSum7KES` direct-serialisation output).
    pub fn from_bytes(bytes: &[u8], current_period: u64) -> Result<Self> {
        let signing_key = direct_deserialise_buf_checked::<
            <KesAlgorithm128 as KesAlgorithm>::SigningKey,
        >(bytes)
        .map_err(|e| {
            CryptoError::InvalidSignature(format!("failed to deserialize KES signing key: {e}"))
        })?;

        let mut key = Self {
            signing_key: Some(signing_key),
            current_period: 0,
        };
        if current_period > 0 {
            key.evolve_in_place(current_period)?;
        }
        Ok(key)
    }

    /// Serialize the signing key to raw bytes, matching Haskell's direct
    /// serialisation layout.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buffer = vec![0u8; <KesAlgorithm128 as KesAlgorithm>::SIGNING_KEY_SIZE];
        let signing_key = self
            .signing_key
            .as_ref()
            .ok_or(CryptoError::KesKeyExpired)?;
        direct_serialise_buf_checked::<<KesAlgorithm128 as KesAlgorithm>::SigningKey>(
            buffer.as_mut_slice(),
            signing_key,
        )
        .map_err(|e| {
            CryptoError::InvalidSignature(format!("failed to serialize KES signing key: {e}"))
        })?;
        Ok(buffer)
    }

    /// Persist the signing key as a TextEnvelope-compatible JSON string.
    pub fn to_text_envelope_json(&self) -> Result<String> {
        let mut encoder = Encoder::new(Vec::new());
        let key_bytes = self.to_bytes()?;
        encoder.bytes(&key_bytes).map_err(|e| {
            CryptoError::InvalidSignature(format!("failed to encode KES signing key to CBOR: {e}"))
        })?;
        let cbor_hex = hex_encode(encoder.into_writer());
        let envelope = TextEnvelope {
            type_: KES_SIGNING_KEY_TEXT_TYPE.to_string(),
            description: KES_SIGNING_KEY_DESCRIPTION.to_string(),
            cbor_hex,
        };
        serde_json::to_string_pretty(&envelope).map_err(|e| {
            CryptoError::InvalidSignature(format!(
                "failed to serialize KES signing key envelope: {e}"
            ))
        })
    }

    /// Derive the corresponding verification key.
    pub fn to_public(&self) -> Result<KesPublicKey> {
        let signing_key = self
            .signing_key
            .as_ref()
            .ok_or(CryptoError::KesKeyExpired)?;
        let vkey = <KesAlgorithm128 as KesAlgorithm>::derive_verification_key(signing_key)
            .map_err(map_kes_m_error)?;
        Ok(KesPublicKey { key: vkey })
    }

    /// Return the current KES period tracked by this key.
    pub fn current_period(&self) -> u64 {
        self.current_period
    }

    /// Return the maximum supported KES period.
    pub fn max_period(&self) -> u64 {
        Self::MAX_PERIOD
    }

    /// Whether the key has already evolved past the maximum supported period.
    pub fn is_expired(&self) -> bool {
        self.current_period > Self::MAX_PERIOD
    }

    /// Advance the signing key in-place to the requested period.
    pub fn evolve_in_place(&mut self, target_period: u64) -> Result<()> {
        if target_period < self.current_period {
            return Err(CryptoError::KesPeriodMismatch);
        }
        if target_period > Self::MAX_PERIOD {
            return Err(CryptoError::KesKeyExpired);
        }

        while self.current_period < target_period {
            let key = self.signing_key.take().ok_or(CryptoError::KesKeyExpired)?;
            let updated =
                <KesAlgorithm128 as KesAlgorithm>::update_kes(&(), key, self.current_period)
                    .map_err(map_kes_m_error)?
                    .ok_or(CryptoError::KesKeyExpired)?;
            self.signing_key = Some(updated);
            self.current_period += 1;
        }

        Ok(())
    }

    /// Consume the signing key, evolving it to the requested period and
    /// returning the updated instance.
    pub fn evolve_to(mut self, target_period: u64) -> Result<Self> {
        self.evolve_in_place(target_period)?;
        Ok(self)
    }

    /// Sign a message for the provided period. The period must match the
    /// current period tracked by this key.
    pub fn sign(&self, period: u64, message: &[u8]) -> Result<KesSignature> {
        if period != self.current_period {
            return Err(CryptoError::KesPeriodMismatch);
        }
        if period > Self::MAX_PERIOD {
            return Err(CryptoError::KesKeyExpired);
        }

        let signing_key = self
            .signing_key
            .as_ref()
            .ok_or(CryptoError::KesKeyExpired)?;
        let signature =
            <KesAlgorithm128 as KesAlgorithm>::sign_kes(&(), period, message, signing_key)
                .map_err(map_kes_m_error)?;

        Ok(KesSignature::from_algorithm_signature(period, signature))
    }
}

impl Drop for KesSecretKey {
    fn drop(&mut self) {
        if let Some(signing_key) = self.signing_key.take() {
            <KesAlgorithm128 as KesAlgorithm>::forget_signing_key_kes(signing_key);
        }
        self.current_period = 0;
    }
}

/// Signed KES payload encapsulating the period and raw signature bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KesSignature {
    pub period: u64,
    bytes: Vec<u8>,
}

impl KesSignature {
    /// Size of the underlying algorithm signature (without the period prefix).
    pub const RAW_SIZE: usize = <KesAlgorithm128 as KesAlgorithm>::SIGNATURE_SIZE;

    /// Size of the serialized representation (period + raw signature bytes).
    pub const SERIALIZED_SIZE: usize = std::mem::size_of::<u64>() + Self::RAW_SIZE;

    fn from_algorithm_signature(
        period: u64,
        signature: <KesAlgorithm128 as KesAlgorithm>::Signature,
    ) -> Self {
        let bytes = <KesAlgorithm128 as KesAlgorithm>::raw_serialize_signature_kes(&signature);
        Self { period, bytes }
    }

    /// Raw signature bytes compatible with the upstream implementation.
    pub fn signature_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Serialize the signature (period prefix + raw bytes).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::SERIALIZED_SIZE);
        out.extend_from_slice(&self.period.to_le_bytes());
        out.extend_from_slice(&self.bytes);
        out
    }

    /// Deserialize a signature from bytes produced by [`KesSignature::to_bytes`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::SERIALIZED_SIZE {
            return Err(CryptoError::InvalidSignature(format!(
                "expected KES signature to be {} bytes, got {}",
                Self::SERIALIZED_SIZE,
                bytes.len()
            )));
        }

        let period = u64::from_le_bytes(bytes[0..8].try_into().expect("slice length checked"));
        let sig_bytes = bytes[8..].to_vec();

        if <KesAlgorithm128 as KesAlgorithm>::raw_deserialize_signature_kes(&sig_bytes).is_none() {
            return Err(CryptoError::InvalidSignature(
                "invalid KES signature encoding".to_string(),
            ));
        }

        Ok(Self {
            period,
            bytes: sig_bytes,
        })
    }

    pub fn as_algorithm_signature(&self) -> Result<<KesAlgorithm128 as KesAlgorithm>::Signature> {
        <KesAlgorithm128 as KesAlgorithm>::raw_deserialize_signature_kes(&self.bytes).ok_or_else(
            || CryptoError::InvalidSignature("invalid KES signature encoding".to_string()),
        )
    }
}

fn read_text_envelope<P: AsRef<Path>>(path: P) -> Result<TextEnvelope> {
    let json = fs::read_to_string(path)
        .map_err(|e| CryptoError::InvalidSignature(format!("failed to read KES key file: {e}")))?;
    serde_json::from_str(&json).map_err(|e| {
        CryptoError::InvalidSignature(format!("failed to parse TextEnvelope JSON: {e}"))
    })
}

fn decode_text_envelope_bytes(cbor_hex: &str) -> Result<Vec<u8>> {
    let cbor_bytes = hex_decode(cbor_hex)
        .map_err(|e| CryptoError::InvalidSignature(format!("invalid CBOR hex: {e}")))?;
    let mut decoder = Decoder::new(&cbor_bytes);
    let bytes = decoder
        .bytes()
        .map_err(|e| CryptoError::InvalidSignature(format!("invalid KES CBOR payload: {e}")))?
        .to_vec();
    if (decoder.position() as usize) != cbor_bytes.len() {
        return Err(CryptoError::InvalidSignature(
            "unexpected trailing data in KES TextEnvelope".to_string(),
        ));
    }
    Ok(bytes)
}

fn map_kes_error(err: KesError) -> CryptoError {
    match err {
        KesError::VerificationFailed => {
            CryptoError::InvalidSignature("KES signature verification failed".to_string())
        }
        KesError::WrongLength {
            context,
            expected,
            actual,
        } => CryptoError::InvalidSignature(format!(
            "{context}: wrong length (expected {expected}, got {actual})"
        )),
        KesError::Message(msg) => CryptoError::InvalidSignature(msg),
        KesError::KeyExpired => CryptoError::KesKeyExpired,
        KesError::PeriodOutOfRange { .. } => CryptoError::KesPeriodMismatch,
    }
}

fn map_kes_m_error(err: KesMError) -> CryptoError {
    match err {
        KesMError::Kes(inner) => map_kes_error(inner),
        KesMError::Mlocked(e) => CryptoError::InvalidSignature(format!("mlocked KES error: {e}")),
        KesMError::Dsign(e) => {
            CryptoError::InvalidSignature(format!("DSIGN error while handling KES key: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_sign_verify_evolve() {
        let mut key = KesSecretKey::generate().expect("KES key generation");
        assert_eq!(key.current_period(), 0);
        assert_eq!(key.max_period(), KesSecretKey::MAX_PERIOD);

        let public_key = key.to_public().expect("derive verification key");
        let message = b"genesis block";

        let signature = key.sign(0, message).expect("sign");
        assert_eq!(signature.period, 0);
        public_key.verify(0, message, &signature).expect("verify");

        key.evolve_in_place(1).expect("evolve to period 1");
        assert_eq!(key.current_period(), 1);
        let signature_1 = key.sign(1, message).expect("sign after evolve");
        public_key
            .verify(1, message, &signature_1)
            .expect("verify evolved signature");
    }

    #[test]
    fn serialization_roundtrip() {
        let key = KesSecretKey::generate().expect("generate");
        let bytes = key.to_bytes().expect("serialize key");
        assert_eq!(
            bytes.len(),
            <KesAlgorithm128 as KesAlgorithm>::SIGNING_KEY_SIZE
        );

        let restored =
            KesSecretKey::from_bytes(&bytes, key.current_period()).expect("deserialize key");
        assert_eq!(restored.current_period(), key.current_period());

        let msg = b"roundtrip";
        let sig = restored.sign(restored.current_period(), msg).expect("sign");
        let public_key = restored.to_public().expect("public key");
        public_key
            .verify(restored.current_period(), msg, &sig)
            .expect("verify");

        let sig_bytes = sig.to_bytes();
        let restored_sig = KesSignature::from_bytes(&sig_bytes).expect("deserialize signature");
        assert_eq!(restored_sig.period, sig.period);
        assert_eq!(restored_sig.signature_bytes(), sig.signature_bytes());
    }

    #[test]
    fn evolve_beyond_max_fails() {
        let mut key = KesSecretKey::generate().expect("generate");
        let max = KesSecretKey::MAX_PERIOD;
        key.evolve_in_place(max).expect("evolve to max");
        assert_eq!(key.current_period(), max);
        assert!(matches!(
            key.evolve_in_place(max + 1),
            Err(CryptoError::KesKeyExpired)
        ));
    }

    #[test]
    fn text_envelope_roundtrip() {
        let key = KesSecretKey::generate().expect("generate");
        let json = key.to_text_envelope_json().expect("serialize envelope");
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("kes.skey");
        fs::write(&path, json).expect("write envelope");

        let restored = KesSecretKey::from_file(&path, 0).expect("load envelope");
        assert_eq!(restored.current_period(), 0);
    }
}
