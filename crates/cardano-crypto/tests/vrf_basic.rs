use cardano_crypto::vrf::{
    VrfOutput, VrfPrivateKey, VrfProof, VRF_PRIVATE_KEY_LENGTH, VRF_PROOF_LENGTH,
};

#[test]
fn vrf_prove_verify_roundtrip() {
    // Test that VRF prove/verify roundtrip works correctly
    let seed = [0x42; 32];
    let private = VrfPrivateKey::generate(&seed);
    let public = private.public_key();

    let message = b"cardano-vrf-test";
    let (output, proof) = private.prove(message);

    // Verify sizes
    assert_eq!(private.to_bytes().len(), VRF_PRIVATE_KEY_LENGTH);
    assert_eq!(proof.to_bytes().len(), VRF_PROOF_LENGTH);

    // Verify the proof
    assert!(public.verify(message, &output, &proof));

    // Test determinism - same inputs should produce same outputs
    let (output2, proof2) = private.prove(message);
    assert_eq!(output.to_bytes(), output2.to_bytes());
    assert_eq!(proof.to_bytes(), proof2.to_bytes());

    // Test that wrong message fails verification
    let wrong_message = b"wrong-message";
    assert!(!public.verify(wrong_message, &output, &proof));
}

#[test]
fn vrf_different_seeds_produce_different_outputs() {
    let message = b"test-message";

    let seed1 = [0x01; 32];
    let private1 = VrfPrivateKey::generate(&seed1);
    let (output1, _) = private1.prove(message);

    let seed2 = [0x02; 32];
    let private2 = VrfPrivateKey::generate(&seed2);
    let (output2, _) = private2.prove(message);

    // Different seeds should produce different outputs
    assert_ne!(output1.to_bytes(), output2.to_bytes());
}
