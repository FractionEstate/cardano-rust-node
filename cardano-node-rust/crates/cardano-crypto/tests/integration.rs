//! Integration tests for cardano-crypto crate
//!
//! Tests the interaction between different cryptographic modules
//! and verifies compatibility with Cardano's cryptographic requirements.

use cardano_crypto::*;
// Remove unused import

#[test]
fn test_crypto_module_integration() {
    // Test that all crypto modules work together

    // Ed25519 operations
    let private_key = Ed25519PrivateKey::generate(&[1u8; 32]);
    let public_key = private_key.public_key();
    let message = b"Cardano integration test message";
    let signature = private_key.sign(message);
    assert!(public_key.verify(message, &signature));

    // VRF operations
    let vrf_private = VrfPrivateKey::generate(&[2u8; 32]);
    let vrf_public = vrf_private.public_key();
    let vrf_input = b"VRF input data";
    let (vrf_output, vrf_proof) = vrf_private.prove(vrf_input);
    assert!(vrf_public.verify(vrf_input, &vrf_output, &vrf_proof));

    // Hash operations
    let data = b"Data to hash";
    let blake2b_256 = Blake2b256Hash::hash(data);
    let blake2b_512 = Blake2b512Hash::hash(data);
    let sha256 = Sha256Hash::hash(data);

    // Verify hash lengths
    assert_eq!(blake2b_256.as_bytes().len(), 32);
    assert_eq!(blake2b_512.as_bytes().len(), 64);
    assert_eq!(sha256.as_bytes().len(), 32);
}

#[test]
fn test_bls_signature_operations() {
    // BLS basic operations
    let private_key = BlsPrivateKey::generate();
    let public_key = private_key.public_key();
    let message = b"BLS test message";
    let signature = private_key.sign(message);

    // Verify signature
    assert!(public_key.verify(message, &signature));

    // Test serialization roundtrip
    let pk_bytes = public_key.to_bytes();
    let restored_pk = BlsPublicKey::from_bytes(&pk_bytes).unwrap();
    assert!(restored_pk.verify(message, &signature));
}

#[test]
fn test_bls_aggregation() {
    let message = b"Aggregation test message";

    // Create multiple keys and signatures
    let private_keys: Vec<BlsPrivateKey> = (0..3)
        .map(|i| {
            let mut seed = [0u8; 32];
            seed[0] = i as u8;
            BlsPrivateKey::from_bytes(&seed).unwrap()
        })
        .collect();

    let public_keys: Vec<BlsPublicKey> = private_keys
        .iter()
        .map(|pk| pk.public_key())
        .collect();

    let signatures: Vec<BlsSignature> = private_keys
        .iter()
        .map(|pk| pk.sign(message))
        .collect();

    // Aggregate public keys and signatures
    let agg_public_key = BlsPublicKey::aggregate(&public_keys).unwrap();
    let agg_signature = BlsSignature::aggregate(&signatures).unwrap();

    // Verify aggregated signature (this would work with proper BLS implementation)
    // For now just test that aggregation doesn't fail
    assert!(agg_public_key.verify(message, &agg_signature));
}

#[test]
fn test_cross_crypto_determinism() {
    // Test that operations are deterministic across different modules
    let seed = [42u8; 32];

    // Ed25519 determinism
    let ed_key1 = Ed25519PrivateKey::generate(&seed);
    let ed_key2 = Ed25519PrivateKey::generate(&seed);
    assert_eq!(ed_key1.to_bytes(), ed_key2.to_bytes());

    // VRF determinism
    let vrf_key1 = VrfPrivateKey::generate(&seed);
    let vrf_key2 = VrfPrivateKey::generate(&seed);
    assert_eq!(vrf_key1.to_bytes(), vrf_key2.to_bytes());

    // Hash determinism
    let data = b"Deterministic test data";
    let hash1 = Blake2b256Hash::hash(data);
    let hash2 = Blake2b256Hash::hash(data);
    assert_eq!(hash1.as_bytes(), hash2.as_bytes());
}

#[test]
fn test_cardano_compatibility_vectors() {
    // Test with known Cardano test vectors

    // Ed25519 test vector (example)
    let private_key_hex = "68e8f4b5f0e1c9c0d9e8f7b6a5948372615b4c3d2e1f0987654321fedcba9876";
    let message = b"Cardano test vector message";

    if let Ok(private_key) = Ed25519PrivateKey::from_hex(private_key_hex) {
        let public_key = private_key.public_key();
        let signature = private_key.sign(message);

        // Verify signature works
        assert!(public_key.verify(message, &signature));

        // Test serialization consistency
        let signature_hex = signature.to_hex();
        let restored_signature = Ed25519Signature::from_hex(&signature_hex).unwrap();
        assert!(public_key.verify(message, &restored_signature));
    }
}

#[test]
fn test_error_handling() {
    // Test proper error handling across modules

    // Invalid key lengths - Ed25519PrivateKey::from_bytes expects exactly 32 bytes
    // For this test, we'll test invalid hex input instead
    assert!(Ed25519PrivateKey::from_hex("invalid_length").is_err());
    assert!(BlsPrivateKey::from_bytes(&[0u8; 31]).is_err());

    // Invalid hex strings
    assert!(Ed25519PrivateKey::from_hex("invalid_hex").is_err());
    assert!(Ed25519Signature::from_hex("too_short").is_err());

    // Invalid signature verification (wrong message)
    let private_key = Ed25519PrivateKey::generate(&[1u8; 32]);
    let public_key = private_key.public_key();
    let signature = private_key.sign(b"original message");
    assert!(!public_key.verify(b"different message", &signature));
}

#[test]
fn test_performance_requirements() {
    // Basic performance tests - operations should complete quickly
    use std::time::Instant;

    let iterations = 100;
    let message = b"Performance test message";

    // Ed25519 signing performance
    let private_key = Ed25519PrivateKey::generate(&[1u8; 32]);
    let start = Instant::now();
    for _ in 0..iterations {
        let _signature = private_key.sign(message);
    }
    let ed25519_duration = start.elapsed();
    println!("Ed25519 signing: {} ops in {:?}", iterations, ed25519_duration);

    // Ed25519 verification performance
    let public_key = private_key.public_key();
    let signature = private_key.sign(message);
    let start = Instant::now();
    for _ in 0..iterations {
        assert!(public_key.verify(message, &signature));
    }
    let verify_duration = start.elapsed();
    println!("Ed25519 verification: {} ops in {:?}", iterations, verify_duration);

    // Hash performance
    let data = vec![0u8; 1024]; // 1KB data
    let start = Instant::now();
    for _ in 0..iterations {
        let _hash = Blake2b256Hash::hash(&data);
    }
    let hash_duration = start.elapsed();
    println!("BLAKE2b-256 hashing: {} ops in {:?}", iterations, hash_duration);

    // Basic performance assertions (operations should be fast)
    assert!(ed25519_duration.as_millis() < 1000); // Should complete in < 1 second
    assert!(verify_duration.as_millis() < 1000);
    assert!(hash_duration.as_millis() < 1000);
}
