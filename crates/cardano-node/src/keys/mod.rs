//! Cryptographic Key Management
//!
//! This module handles loading, parsing, and managing cryptographic keys
//! for block production including:
//! - VRF (Verifiable Random Function) keys
//! - KES (Key Evolving Signature) keys with automatic evolution
//! - Cold keys (stake pool cold keys)
//! - Operational certificates
//!
//! Supports both Cardano CLI JSON format and raw binary formats.

pub mod kes_evolution;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use cardano_crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey};
use cardano_crypto::vrf::{VrfPrivateKey, VrfPublicKey};

use crate::config::block_producer::KeyFormat;

pub use kes_evolution::{KesEvolutionConfig, KesEvolutionTracker};

/// Cardano CLI JSON key envelope (common structure for all key types)
#[derive(Debug, Clone, Deserialize, Serialize)]
struct CardanoCliKeyEnvelope {
    #[serde(rename = "type")]
    key_type: String,
    description: String,
    #[serde(rename = "cborHex")]
    cbor_hex: String,
}

/// VRF signing key with associated public key
#[derive(Debug, Clone)]
pub struct VrfSigningKey {
    pub private_key: VrfPrivateKey,
    pub public_key: VrfPublicKey,
}

/// VRF verification key
#[derive(Debug, Clone)]
pub struct VrfVerificationKey {
    pub public_key: VrfPublicKey,
}

/// KES signing key with evolution tracking
///
/// For automatic KES key evolution, see `KesEvolutionTracker`.
#[derive(Debug, Clone)]
pub struct KesSigningKey {
    // KES keys are complex and evolve over time
    // Automatic evolution is handled by KesEvolutionTracker
    pub key_data: Vec<u8>,
    pub period: u64,
}

/// KES verification key
#[derive(Debug, Clone)]
pub struct KesVerificationKey {
    pub key_data: Vec<u8>,
}

/// Cold signing key (stake pool operator key)
#[derive(Debug)]
pub struct ColdSigningKey {
    pub private_key: Ed25519PrivateKey,
}

/// Cold verification key
#[derive(Debug, Clone)]
pub struct ColdVerificationKey {
    pub public_key: Ed25519PublicKey,
}

/// Operational certificate for block producer
#[derive(Debug, Clone)]
pub struct OperationalCertificate {
    /// KES verification key hash
    pub kes_vkey_hash: Vec<u8>,
    /// Certificate issue number (counter)
    pub issue_number: u64,
    /// KES period when certificate was issued
    pub kes_period: u64,
    /// Cold key signature over the certificate
    pub signature: Vec<u8>,
    /// Raw certificate data
    pub raw_data: Vec<u8>,
}

/// Load VRF signing key from file
pub fn load_vrf_signing_key(path: &Path, format: KeyFormat) -> Result<VrfSigningKey> {
    match format {
        KeyFormat::CardanoCli => load_vrf_signing_key_cardano_cli(path),
        KeyFormat::RawHex => load_vrf_signing_key_raw_hex(path),
        KeyFormat::RawBinary => load_vrf_signing_key_raw_binary(path),
    }
}

/// Load VRF signing key from Cardano CLI JSON format
fn load_vrf_signing_key_cardano_cli(path: &Path) -> Result<VrfSigningKey> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read VRF signing key from {:?}", path))?;

    let envelope: CardanoCliKeyEnvelope = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse VRF signing key JSON from {:?}", path))?;

    // Validate key type
    if !envelope.key_type.contains("VrfSigningKey") {
        return Err(anyhow!(
            "Invalid key type: expected VrfSigningKey, got {}",
            envelope.key_type
        ));
    }

    // Decode CBOR hex
    let cbor_bytes = hex::decode(&envelope.cbor_hex)
        .with_context(|| format!("Failed to decode VRF key hex from {:?}", path))?;

    // Parse CBOR to extract the actual key bytes
    // Cardano CLI wraps keys in CBOR arrays
    let key_bytes = parse_cbor_key_bytes(&cbor_bytes)
        .with_context(|| format!("Failed to parse VRF key CBOR from {:?}", path))?;

    // VRF keys are 64 bytes in Cardano (32 bytes seed + 32 bytes extension)
    if key_bytes.len() != 64 {
        return Err(anyhow!(
            "Invalid VRF key length: expected 64 bytes, got {}",
            key_bytes.len()
        ));
    }

    // Generate VRF key from seed (first 32 bytes)
    let seed: [u8; 32] = key_bytes[0..32]
        .try_into()
        .map_err(|_| anyhow!("Failed to extract VRF seed"))?;

    let private_key = VrfPrivateKey::generate(&seed);
    let public_key = private_key.public_key();

    Ok(VrfSigningKey {
        private_key,
        public_key,
    })
}

/// Load VRF signing key from raw hex format
fn load_vrf_signing_key_raw_hex(path: &Path) -> Result<VrfSigningKey> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read VRF signing key from {:?}", path))?;

    let key_bytes = hex::decode(content.trim())
        .with_context(|| format!("Failed to decode VRF key hex from {:?}", path))?;

    if key_bytes.len() < 32 {
        return Err(anyhow!(
            "Invalid VRF key length: expected at least 32 bytes, got {}",
            key_bytes.len()
        ));
    }

    let seed: [u8; 32] = key_bytes[0..32]
        .try_into()
        .map_err(|_| anyhow!("Failed to extract VRF seed"))?;

    let private_key = VrfPrivateKey::generate(&seed);
    let public_key = private_key.public_key();

    Ok(VrfSigningKey {
        private_key,
        public_key,
    })
}

/// Load VRF signing key from raw binary format
fn load_vrf_signing_key_raw_binary(path: &Path) -> Result<VrfSigningKey> {
    let key_bytes = fs::read(path)
        .with_context(|| format!("Failed to read VRF signing key from {:?}", path))?;

    if key_bytes.len() < 32 {
        return Err(anyhow!(
            "Invalid VRF key length: expected at least 32 bytes, got {}",
            key_bytes.len()
        ));
    }

    let seed: [u8; 32] = key_bytes[0..32]
        .try_into()
        .map_err(|_| anyhow!("Failed to extract VRF seed"))?;

    let private_key = VrfPrivateKey::generate(&seed);
    let public_key = private_key.public_key();

    Ok(VrfSigningKey {
        private_key,
        public_key,
    })
}

/// Load VRF verification key from file
pub fn load_vrf_verification_key(path: &Path, format: KeyFormat) -> Result<VrfVerificationKey> {
    match format {
        KeyFormat::CardanoCli => load_vrf_verification_key_cardano_cli(path),
        KeyFormat::RawHex => load_vrf_verification_key_raw_hex(path),
        KeyFormat::RawBinary => load_vrf_verification_key_raw_binary(path),
    }
}

/// Load VRF verification key from Cardano CLI JSON format
fn load_vrf_verification_key_cardano_cli(path: &Path) -> Result<VrfVerificationKey> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read VRF verification key from {:?}", path))?;

    let envelope: CardanoCliKeyEnvelope = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse VRF verification key JSON from {:?}", path))?;

    if !envelope.key_type.contains("VrfVerificationKey") {
        return Err(anyhow!(
            "Invalid key type: expected VrfVerificationKey, got {}",
            envelope.key_type
        ));
    }

    let cbor_bytes = hex::decode(&envelope.cbor_hex)
        .with_context(|| format!("Failed to decode VRF verification key hex from {:?}", path))?;

    let key_bytes = parse_cbor_key_bytes(&cbor_bytes)
        .with_context(|| format!("Failed to parse VRF verification key CBOR from {:?}", path))?;

    // VRF public key is 32 bytes
    if key_bytes.len() != 32 {
        return Err(anyhow!(
            "Invalid VRF verification key length: expected 32 bytes, got {}",
            key_bytes.len()
        ));
    }

    let public_key_bytes: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| anyhow!("Failed to convert VRF public key bytes"))?;

    let public_key = VrfPublicKey::from_bytes(&public_key_bytes)?;

    Ok(VrfVerificationKey { public_key })
}

/// Load VRF verification key from raw hex format
fn load_vrf_verification_key_raw_hex(path: &Path) -> Result<VrfVerificationKey> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read VRF verification key from {:?}", path))?;

    let key_bytes = hex::decode(content.trim())
        .with_context(|| format!("Failed to decode VRF verification key hex from {:?}", path))?;

    if key_bytes.len() != 32 {
        return Err(anyhow!(
            "Invalid VRF verification key length: expected 32 bytes, got {}",
            key_bytes.len()
        ));
    }

    let public_key_bytes: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| anyhow!("Failed to convert VRF public key bytes"))?;

    let public_key = VrfPublicKey::from_bytes(&public_key_bytes)?;

    Ok(VrfVerificationKey { public_key })
}

/// Load VRF verification key from raw binary format
fn load_vrf_verification_key_raw_binary(path: &Path) -> Result<VrfVerificationKey> {
    let key_bytes = fs::read(path)
        .with_context(|| format!("Failed to read VRF verification key from {:?}", path))?;

    if key_bytes.len() != 32 {
        return Err(anyhow!(
            "Invalid VRF verification key length: expected 32 bytes, got {}",
            key_bytes.len()
        ));
    }

    let public_key_bytes: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| anyhow!("Failed to convert VRF public key bytes"))?;

    let public_key = VrfPublicKey::from_bytes(&public_key_bytes)?;

    Ok(VrfVerificationKey { public_key })
}

/// Load KES signing key from file
pub fn load_kes_signing_key(path: &Path, format: KeyFormat, period: u64) -> Result<KesSigningKey> {
    match format {
        KeyFormat::CardanoCli => load_kes_signing_key_cardano_cli(path, period),
        KeyFormat::RawHex => load_kes_signing_key_raw_hex(path, period),
        KeyFormat::RawBinary => load_kes_signing_key_raw_binary(path, period),
    }
}

/// Load KES signing key from Cardano CLI JSON format
fn load_kes_signing_key_cardano_cli(path: &Path, period: u64) -> Result<KesSigningKey> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read KES signing key from {:?}", path))?;

    let envelope: CardanoCliKeyEnvelope = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse KES signing key JSON from {:?}", path))?;

    if !envelope.key_type.contains("KesSigningKey") {
        return Err(anyhow!(
            "Invalid key type: expected KesSigningKey, got {}",
            envelope.key_type
        ));
    }

    let cbor_bytes = hex::decode(&envelope.cbor_hex)
        .with_context(|| format!("Failed to decode KES key hex from {:?}", path))?;

    let key_bytes = parse_cbor_key_bytes(&cbor_bytes)
        .with_context(|| format!("Failed to parse KES key CBOR from {:?}", path))?;

    Ok(KesSigningKey {
        key_data: key_bytes,
        period,
    })
}

/// Load KES signing key from raw hex format
fn load_kes_signing_key_raw_hex(path: &Path, period: u64) -> Result<KesSigningKey> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read KES signing key from {:?}", path))?;

    let key_data = hex::decode(content.trim())
        .with_context(|| format!("Failed to decode KES key hex from {:?}", path))?;

    Ok(KesSigningKey { key_data, period })
}

/// Load KES signing key from raw binary format
fn load_kes_signing_key_raw_binary(path: &Path, period: u64) -> Result<KesSigningKey> {
    let key_data = fs::read(path)
        .with_context(|| format!("Failed to read KES signing key from {:?}", path))?;

    Ok(KesSigningKey { key_data, period })
}

/// Load operational certificate from file
pub fn load_operational_certificate(path: &Path) -> Result<OperationalCertificate> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read operational certificate from {:?}", path))?;

    let envelope: CardanoCliKeyEnvelope = serde_json::from_str(&content).with_context(|| {
        format!(
            "Failed to parse operational certificate JSON from {:?}",
            path
        )
    })?;

    if !envelope.key_type.contains("NodeOperationalCertificate") {
        return Err(anyhow!(
            "Invalid certificate type: expected NodeOperationalCertificate, got {}",
            envelope.key_type
        ));
    }

    let cbor_bytes = hex::decode(&envelope.cbor_hex).with_context(|| {
        format!(
            "Failed to decode operational certificate hex from {:?}",
            path
        )
    })?;

    // Parse the operational certificate structure
    // Format: [kes_vkey_hash, issue_number, kes_period, signature]
    // This follows the Cardano Conway era certificate format

    parse_operational_certificate_cbor(&cbor_bytes).with_context(|| {
        format!(
            "Failed to parse operational certificate CBOR from {:?}",
            path
        )
    })
}

/// Parse operational certificate from CBOR bytes
///
/// Cardano operational certificate format (CBOR array):
/// ```text
/// [
///   kes_vkey_hash,    // bytes (32 bytes)
///   issue_number,     // uint (certificate counter)
///   kes_period,       // uint (KES period)
///   signature         // bytes (64 bytes - Ed25519 signature)
/// ]
/// ```
fn parse_operational_certificate_cbor(cbor_data: &[u8]) -> Result<OperationalCertificate> {
    use minicbor::decode;

    let mut decoder = decode::Decoder::new(cbor_data);

    // Parse array header - expect 4 elements
    let array_len = decoder
        .array()
        .map_err(|e| anyhow!("Failed to parse certificate array: {}", e))?
        .ok_or_else(|| anyhow!("Certificate array has indefinite length"))?;

    if array_len != 4 {
        return Err(anyhow!(
            "Invalid certificate format: expected 4 elements, got {}",
            array_len
        ));
    }

    // Element 0: KES verification key hash (32 bytes)
    let kes_vkey_hash = decoder
        .bytes()
        .map_err(|e| anyhow!("Failed to parse KES vkey hash: {}", e))?
        .to_vec();

    if kes_vkey_hash.len() != 32 {
        return Err(anyhow!(
            "Invalid KES vkey hash length: expected 32 bytes, got {}",
            kes_vkey_hash.len()
        ));
    }

    // Element 1: Issue number (certificate counter)
    let issue_number = decoder
        .u64()
        .map_err(|e| anyhow!("Failed to parse issue number: {}", e))?;

    // Element 2: KES period
    let kes_period = decoder
        .u64()
        .map_err(|e| anyhow!("Failed to parse KES period: {}", e))?;

    // Element 3: Cold key signature (64 bytes - Ed25519)
    let signature = decoder
        .bytes()
        .map_err(|e| anyhow!("Failed to parse signature: {}", e))?
        .to_vec();

    if signature.len() != 64 {
        return Err(anyhow!(
            "Invalid signature length: expected 64 bytes, got {}",
            signature.len()
        ));
    }

    Ok(OperationalCertificate {
        kes_vkey_hash,
        issue_number,
        kes_period,
        signature,
        raw_data: cbor_data.to_vec(),
    })
}

/// Parse key bytes from CBOR envelope
/// Cardano CLI wraps keys in CBOR byte strings
fn parse_cbor_key_bytes(cbor_data: &[u8]) -> Result<Vec<u8>> {
    // Simple CBOR parser for byte strings
    // CBOR byte string format:
    // - 0x40-0x57: byte string of length 0-23
    // - 0x58 + 1 byte: byte string with 1-byte length
    // - 0x59 + 2 bytes: byte string with 2-byte length

    if cbor_data.is_empty() {
        return Err(anyhow!("Empty CBOR data"));
    }

    let first_byte = cbor_data[0];

    // Byte string with length 0-23
    if (0x40..=0x57).contains(&first_byte) {
        let length = (first_byte - 0x40) as usize;
        if cbor_data.len() < 1 + length {
            return Err(anyhow!("CBOR data too short"));
        }
        return Ok(cbor_data[1..1 + length].to_vec());
    }

    // Byte string with 1-byte length
    if first_byte == 0x58 {
        if cbor_data.len() < 2 {
            return Err(anyhow!("CBOR data too short for length byte"));
        }
        let length = cbor_data[1] as usize;
        if cbor_data.len() < 2 + length {
            return Err(anyhow!("CBOR data too short"));
        }
        return Ok(cbor_data[2..2 + length].to_vec());
    }

    // Byte string with 2-byte length
    if first_byte == 0x59 {
        if cbor_data.len() < 3 {
            return Err(anyhow!("CBOR data too short for length bytes"));
        }
        let length = u16::from_be_bytes([cbor_data[1], cbor_data[2]]) as usize;
        if cbor_data.len() < 3 + length {
            return Err(anyhow!("CBOR data too short"));
        }
        return Ok(cbor_data[3..3 + length].to_vec());
    }

    Err(anyhow!(
        "Unsupported CBOR format: first byte = 0x{:02x}",
        first_byte
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cbor_short_bytestring() {
        // CBOR: 0x45 (byte string length 5) + 5 bytes
        let cbor = vec![0x45, 0x01, 0x02, 0x03, 0x04, 0x05];
        let result = parse_cbor_key_bytes(&cbor).unwrap();
        assert_eq!(result, vec![0x01, 0x02, 0x03, 0x04, 0x05]);
    }

    #[test]
    fn test_parse_cbor_1byte_length() {
        // CBOR: 0x58 (byte string with 1-byte length) + length + data
        let cbor = vec![0x58, 0x03, 0xaa, 0xbb, 0xcc];
        let result = parse_cbor_key_bytes(&cbor).unwrap();
        assert_eq!(result, vec![0xaa, 0xbb, 0xcc]);
    }

    #[test]
    fn test_parse_cbor_2byte_length() {
        // CBOR: 0x59 (byte string with 2-byte length) + length + data
        let mut cbor = vec![0x59, 0x00, 0x04];
        cbor.extend_from_slice(&[0x11, 0x22, 0x33, 0x44]);
        let result = parse_cbor_key_bytes(&cbor).unwrap();
        assert_eq!(result, vec![0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn test_parse_operational_certificate() {
        use minicbor::encode;

        // Create a valid operational certificate CBOR
        let kes_vkey_hash = vec![0xaa; 32];
        let issue_number = 5u64;
        let kes_period = 42u64;
        let signature = vec![0xbb; 64];

        let mut encoder = encode::Encoder::new(Vec::new());
        encoder.array(4).unwrap();
        encoder.bytes(&kes_vkey_hash).unwrap();
        encoder.u64(issue_number).unwrap();
        encoder.u64(kes_period).unwrap();
        encoder.bytes(&signature).unwrap();

        let cbor_bytes = encoder.into_writer();

        // Parse the certificate
        let cert = parse_operational_certificate_cbor(&cbor_bytes).unwrap();

        assert_eq!(cert.kes_vkey_hash, kes_vkey_hash);
        assert_eq!(cert.issue_number, 5);
        assert_eq!(cert.kes_period, 42);
        assert_eq!(cert.signature, signature);
    }

    #[test]
    fn test_parse_operational_certificate_invalid_array_length() {
        use minicbor::encode;

        // Create a certificate with wrong number of elements
        let mut encoder = encode::Encoder::new(Vec::new());
        encoder.array(3).unwrap(); // Only 3 elements instead of 4
        encoder.bytes(&vec![0xaa; 32]).unwrap();
        encoder.u64(5).unwrap();
        encoder.u64(42).unwrap();

        let cbor_bytes = encoder.into_writer();

        // Should fail
        let result = parse_operational_certificate_cbor(&cbor_bytes);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("expected 4 elements"));
    }

    #[test]
    fn test_parse_operational_certificate_invalid_vkey_hash_length() {
        use minicbor::encode;

        // Create a certificate with wrong vkey hash length
        let mut encoder = encode::Encoder::new(Vec::new());
        encoder.array(4).unwrap();
        encoder.bytes(&vec![0xaa; 16]).unwrap(); // Only 16 bytes instead of 32
        encoder.u64(5).unwrap();
        encoder.u64(42).unwrap();
        encoder.bytes(&vec![0xbb; 64]).unwrap();

        let cbor_bytes = encoder.into_writer();

        // Should fail
        let result = parse_operational_certificate_cbor(&cbor_bytes);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid KES vkey hash length"));
    }

    #[test]
    fn test_parse_operational_certificate_invalid_signature_length() {
        use minicbor::encode;

        // Create a certificate with wrong signature length
        let mut encoder = encode::Encoder::new(Vec::new());
        encoder.array(4).unwrap();
        encoder.bytes(&vec![0xaa; 32]).unwrap();
        encoder.u64(5).unwrap();
        encoder.u64(42).unwrap();
        encoder.bytes(&vec![0xbb; 32]).unwrap(); // Only 32 bytes instead of 64

        let cbor_bytes = encoder.into_writer();

        // Should fail
        let result = parse_operational_certificate_cbor(&cbor_bytes);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid signature length"));
    }

    #[test]
    fn test_operational_certificate_roundtrip() {
        use minicbor::encode;
        use std::io::Write;

        // Create test certificate data
        let kes_vkey_hash = vec![0x12; 32];
        let issue_number = 10u64;
        let kes_period = 100u64;
        let signature = vec![0x34; 64];

        // Encode to CBOR
        let mut encoder = encode::Encoder::new(Vec::new());
        encoder.array(4).unwrap();
        encoder.bytes(&kes_vkey_hash).unwrap();
        encoder.u64(issue_number).unwrap();
        encoder.u64(kes_period).unwrap();
        encoder.bytes(&signature).unwrap();

        let cbor_bytes = encoder.into_writer();
        let cbor_hex = hex::encode(&cbor_bytes);

        // Create TextEnvelope JSON
        let json = format!(
            r#"{{
                "type": "NodeOperationalCertificate",
                "description": "Test Operational Certificate",
                "cborHex": "{}"
            }}"#,
            cbor_hex
        );

        // Write to temporary file
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_opcert.cert");
        let mut file = std::fs::File::create(&temp_path).unwrap();
        file.write_all(json.as_bytes()).unwrap();
        drop(file);

        // Load from file
        let cert = load_operational_certificate(&temp_path).unwrap();

        // Verify all fields
        assert_eq!(cert.kes_vkey_hash, kes_vkey_hash);
        assert_eq!(cert.issue_number, 10);
        assert_eq!(cert.kes_period, 100);
        assert_eq!(cert.signature, signature);

        // Cleanup
        std::fs::remove_file(&temp_path).ok();
    }
}
