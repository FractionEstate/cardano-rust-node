use cardano_crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey, Ed25519Signature};

#[test]
fn test_ed25519_basic_functionality() {
    // Test key generation from seed
    let seed = [42u8; 32];
    let private_key = Ed25519PrivateKey::generate(&seed);
    let public_key = private_key.public_key();

    // Test signing and verification
    let message = b"test message";
    let signature = private_key.sign(message);

    assert!(public_key.verify(message, &signature));

    // Test serialization
    let private_bytes = private_key.to_bytes();
    let public_bytes = public_key.to_bytes();
    let signature_bytes = signature.to_bytes();

    // Test deserialization
    let restored_private = Ed25519PrivateKey::from_bytes(private_bytes)
        .expect("Restoring private key from bytes should succeed");
    assert_eq!(restored_private.to_bytes(), private_bytes);
    let restored_public = Ed25519PublicKey::from_bytes(public_bytes)
        .expect("Restoring public key from bytes should succeed");
    let restored_signature = Ed25519Signature::from_bytes(signature_bytes);

    // Verify restored objects work
    assert!(restored_public.verify(message, &restored_signature));

    println!("✅ Ed25519 basic functionality test passed");
}
