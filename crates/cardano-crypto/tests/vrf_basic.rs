use cardano_crypto::vrf::{
    VrfOutput, VrfPrivateKey, VrfProof, VRF_PRIVATE_KEY_LENGTH, VRF_PROOF_LENGTH,
};

const SEED_HEX: &str = "4242424242424242424242424242424242424242424242424242424242424242";
const MESSAGE: &[u8] = b"cardano-vrf-test";
const EXPECTED_PUBLIC_KEY: &str =
    "2152f8d19b791d24453242e15f2eab6cb7cffa7b6a5ed30097960e069881db12";
const EXPECTED_OUTPUT: &str = "bad36a6b53482432f058ac35bd6622a6f628e54761fde2141bfd00da7fc366b8fd528644d56651366593571c9d0c1ea47243c5214c9bf583f221b993f891277b";
const EXPECTED_PROOF: &str = "a8f89ba6acd9bf3b34c38f9505bcde82ecba2667b962e37ac45a9251f04ed6eb75f3eac1adc1ea3c2325da4ca86e86a70a302e023a5da250d8f39d4f624f517072186c9edf74ae28965698849a57490b";

#[test]
fn vrf_prove_verify_matches_known_vector() {
    let mut seed = [0u8; 32];
    hex::decode_to_slice(SEED_HEX, &mut seed).unwrap();

    let private = VrfPrivateKey::generate(&seed);
    let public = private.public_key();
    assert_eq!(hex::encode(public.to_bytes()), EXPECTED_PUBLIC_KEY);

    let (output, proof) = private.prove(MESSAGE);
    assert_eq!(hex::encode(output.to_bytes()), EXPECTED_OUTPUT);
    assert_eq!(hex::encode(proof.to_bytes()), EXPECTED_PROOF);

    assert_eq!(private.to_bytes().len(), VRF_PRIVATE_KEY_LENGTH);
    assert_eq!(proof.to_bytes().len(), VRF_PROOF_LENGTH);

    let parsed_output = VrfOutput::from_hex(EXPECTED_OUTPUT).unwrap();
    let parsed_proof = VrfProof::from_hex(EXPECTED_PROOF).unwrap();
    assert!(public.verify(MESSAGE, &parsed_output, &parsed_proof));

    // Determinism check
    let (output2, proof2) = private.prove(MESSAGE);
    assert_eq!(output.to_bytes(), output2.to_bytes());
    assert_eq!(proof.to_bytes(), proof2.to_bytes());
}
