//! Cryptographic Key Management
//!
//! This module handles loading, parsing, and managing cryptographic keys
//! for block production including:
//! - VRF (Verifiable Random Function) keys
//! - KES (Key Evolving Signature) keys
//! - Cold keys (stake pool cold keys)
//! - Operational certificates
//!
//! Supports both Cardano CLI JSON format and raw binary formats.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use cardano_crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey};
use cardano_crypto::vrf::{VrfPrivateKey, VrfPublicKey};

use crate::config::block_producer::KeyFormat;

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
#[derive(Debug, Clone)]
pub struct KesSigningKey {
    // KES keys are complex and evolve over time
    // For now, we store the raw key material
    // TODO: Implement proper KES key evolution
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
    // This is a simplified parser - full CBOR parsing would be more robust

    // For now, we'll store the raw CBOR and extract what we need
    // TODO: Implement proper CBOR parsing for operational certificates

    Ok(OperationalCertificate {
        kes_vkey_hash: Vec::new(), // TODO: Extract from CBOR
        issue_number: 0,           // TODO: Extract from CBOR
        kes_period: 0,             // TODO: Extract from CBOR
        signature: Vec::new(),     // TODO: Extract from CBOR
        raw_data: cbor_bytes,
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
}
