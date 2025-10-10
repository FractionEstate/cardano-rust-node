//! Integration tests for Cardano-compatible VRF
//!
//! These tests validate the complete VRF implementation against
//! official test vectors from IntersectMBO/cardano-base.

use crate::vendor::vrf_pure::{
    cardano_compat::{cardano_vrf_prove, cardano_vrf_verify, point::cardano_hash_to_curve},
    common::{expand_secret_key, secret_key_to_scalar},
};
use curve25519_dalek::edwards::CompressedEdwardsY;
use sha2::{Digest, Sha512};

#[test]
fn test_basic_prove_verify_cycle() {
    let mut sk = [0u8; 64];
    sk[32..64].copy_from_slice(&[
        0x3b, 0x6a, 0x27, 0xbc, 0xce, 0xb6, 0xa4, 0x2d, 0x62, 0xa3, 0xa8, 0xd0, 0x2a, 0x6f, 0x0d,
        0x73, 0x65, 0x32, 0x15, 0x77, 0x1d, 0xe2, 0x43, 0xa6, 0x3a, 0xc0, 0x48, 0xa1, 0x8b, 0x59,
        0xda, 0x29,
    ]);

    let message = b"test message";

    let proof = cardano_vrf_prove(&sk, message).expect("prove should succeed");

    let pk_slice = &sk[32..64];
    let pk_array: [u8; 32] = pk_slice
        .try_into()
        .expect("public key slice must be 32 bytes");
    let output = cardano_vrf_verify(&pk_array, &proof, message).expect("verify should succeed");

    assert_eq!(output.len(), 64, "Output should be 64 bytes");
}

#[test]
fn test_official_test_vector_standard_10() {
    let sk_hex = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
    let pk_hex = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
    let expected_pi_hex = "b6b4699f87d56126c9117a7da55bd0085246f4c56dbc95d20172612e9d38e8d7ca65e573a126ed88d4e30a46f80a666854d675cf3ba81de0de043c3774f061560f55edc256a787afe701677c0f602900";
    let expected_beta_hex = "5b49b554d05c0cd5a5325376b3387de59d924fd1e13ded44648ab33c21349a603f25b84ec5ed887995b33da5e3bfcb87cd2f64521c4c62cf825cffabbe5d31cc";

    let sk_bytes = hex::decode(sk_hex).expect("valid secret hex");
    let pk_bytes = hex::decode(pk_hex).expect("valid public hex");
    let expected_pi = hex::decode(expected_pi_hex).expect("valid proof hex");
    let expected_beta = hex::decode(expected_beta_hex).expect("valid beta hex");

    let mut sk = [0u8; 64];
    sk[0..32].copy_from_slice(&sk_bytes);
    sk[32..64].copy_from_slice(&pk_bytes);

    let mut pk = [0u8; 32];
    pk.copy_from_slice(&pk_bytes);

    let proof = cardano_vrf_prove(&sk, &[]).expect("prove should succeed");
    assert_eq!(
        &proof[..],
        expected_pi.as_slice(),
        "proof should match official vector"
    );

    let beta = cardano_vrf_verify(&pk, &proof, &[]).expect("verify should succeed");
    assert_eq!(
        &beta[..],
        expected_beta.as_slice(),
        "beta should match official vector"
    );
}

#[test]
fn test_official_test_vector_generated_1() {
    let sk_seed_hex = "0000000000000000000000000000000000000000000000000000000000000000";
    let pk_hex = "3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29";
    let alpha_hex = "00";
    let expected_pi_hex = "000f006e64c91f84212919fe0899970cd341206fc081fe599339c8492e2cea3299ae9de4b6ce21cda0a975f65f45b70f82b3952ba6d0dbe11a06716e67aca233c0d78f115a655aa1952ada9f3d692a0a";
    let expected_beta_hex = "9930b5dddc0938f01cf6f9746eded569ee676bd6ff3b4f19233d74b903ec53a45c5728116088b7c622b6d6c354f7125c7d09870b56ec6f1e4bf4970f607e04b2";

    let sk_seed = hex::decode(sk_seed_hex).expect("valid seed hex");
    let pk_bytes = hex::decode(pk_hex).expect("valid public hex");
    let alpha = hex::decode(alpha_hex).expect("valid alpha hex");
    let expected_pi = hex::decode(expected_pi_hex).expect("valid proof hex");
    let expected_beta = hex::decode(expected_beta_hex).expect("valid beta hex");

    let mut sk = [0u8; 64];
    sk[0..32].copy_from_slice(&sk_seed);
    sk[32..64].copy_from_slice(&pk_bytes);

    let mut pk = [0u8; 32];
    pk.copy_from_slice(&pk_bytes);

    let proof = cardano_vrf_prove(&sk, &alpha).expect("prove should succeed");
    assert_eq!(
        &proof[..],
        expected_pi.as_slice(),
        "proof should match official vector"
    );

    let beta = cardano_vrf_verify(&pk, &proof, &alpha).expect("verify should succeed");
    assert_eq!(
        &beta[..],
        expected_beta.as_slice(),
        "beta should match official vector"
    );
}

#[test]
fn test_cardano_hash_to_curve_matches_gamma_factorisation() {
    const SUITE: u8 = 0x04;
    const ONE: u8 = 0x01;

    struct Vector {
        sk_seed_hex: &'static str,
        pk_hex: &'static str,
        alpha_hex: &'static str,
        proof_hex: &'static str,
        label: &'static str,
    }

    let vectors = [
        Vector {
            sk_seed_hex: "0000000000000000000000000000000000000000000000000000000000000000",
            pk_hex: "3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29",
            alpha_hex: "00",
            proof_hex: "000f006e64c91f84212919fe0899970cd341206fc081fe599339c8492e2cea3299ae9de4b6ce21cda0a975f65f45b70f82b3952ba6d0dbe11a06716e67aca233c0d78f115a655aa1952ada9f3d692a0a",
            label: "vrf_ver03_generated_1",
        },
        Vector {
            sk_seed_hex: "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
            pk_hex: "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
            alpha_hex: "",
            proof_hex: "b6b4699f87d56126c9117a7da55bd0085246f4c56dbc95d20172612e9d38e8d7ca65e573a126ed88d4e30a46f80a666854d675cf3ba81de0de043c3774f061560f55edc256a787afe701677c0f602900",
            label: "vrf_ver03_standard_10",
        },
    ];

    for vector in vectors {
        let sk_seed = hex::decode(vector.sk_seed_hex).expect("valid seed hex");
        let pk_bytes = hex::decode(vector.pk_hex).expect("valid pk hex");
        let alpha = hex::decode(vector.alpha_hex).expect("valid alpha hex");
        let proof = hex::decode(vector.proof_hex).expect("valid proof hex");

        let mut gamma_bytes = [0u8; 32];
        gamma_bytes.copy_from_slice(&proof[0..32]);
        let gamma_point = CompressedEdwardsY(gamma_bytes)
            .decompress()
            .expect("gamma decompress");

        let mut seed = [0u8; 32];
        seed.copy_from_slice(&sk_seed);
        let expanded = expand_secret_key(&seed);
        let secret_scalar = secret_key_to_scalar(&expanded);
        let secret_scalar_inv = secret_scalar.invert();
        let expected_h = gamma_point * secret_scalar_inv;

        let mut hasher = Sha512::new();
        hasher.update(&[SUITE]);
        hasher.update(&[ONE]);
        hasher.update(&pk_bytes);
        hasher.update(&alpha);
        let r_string = hasher.finalize();

        let mut r_bytes = [0u8; 32];
        r_bytes.copy_from_slice(&r_string[0..32]);
        r_bytes[31] &= 0x7f; // Clear sign bit per Cardano reference implementation

        let actual_h = cardano_hash_to_curve(&r_bytes).expect("hash_to_curve succeeds");

        let actual_bytes = actual_h.compress().to_bytes();
        let expected_bytes = expected_h.compress().to_bytes();

        assert_eq!(
            actual_bytes, expected_bytes,
            "hash_to_curve mismatch for {}",
            vector.label
        );
    }
}

type ParseVectorResult = Result<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>), String>;

/// Parse a VRF test vector file
fn parse_test_vector(content: &str) -> ParseVectorResult {
    let mut sk = None;
    let mut pk = None;
    let mut alpha = None;
    let mut pi = None;
    let mut beta = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(pos) = line.find(':') {
            let key = line[..pos].trim();
            let value = line[pos + 1..].trim();

            match key {
                "sk" => sk = hex::decode(value).ok(),
                "pk" => pk = hex::decode(value).ok(),
                "alpha" => alpha = hex::decode(value).ok(),
                "pi" => pi = hex::decode(value).ok(),
                "beta" => beta = hex::decode(value).ok(),
                _ => {},
            }
        }
    }

    Ok((
        sk.ok_or("missing sk")?,
        pk.ok_or("missing pk")?,
        alpha.ok_or("missing alpha")?,
        pi.ok_or("missing pi")?,
        beta.ok_or("missing beta")?,
    ))
}

#[test]
fn test_official_test_vector_standard_11() {
    let content = include_str!("../../../../test-vectors/vrf_ver03_standard_11");
    let (sk_seed, pk_bytes, alpha, expected_pi, expected_beta) =
        parse_test_vector(content).expect("valid test vector");

    let mut sk = [0u8; 64];
    sk[0..32].copy_from_slice(&sk_seed);
    sk[32..64].copy_from_slice(&pk_bytes);

    let mut pk = [0u8; 32];
    pk.copy_from_slice(&pk_bytes);

    let proof = cardano_vrf_prove(&sk, &alpha).expect("prove should succeed");
    assert_eq!(
        &proof[..],
        expected_pi.as_slice(),
        "proof should match official vector standard_11"
    );

    let beta = cardano_vrf_verify(&pk, &proof, &alpha).expect("verify should succeed");
    assert_eq!(
        &beta[..],
        expected_beta.as_slice(),
        "beta should match official vector standard_11"
    );
}

#[test]
fn test_official_test_vector_standard_12() {
    let content = include_str!("../../../../test-vectors/vrf_ver03_standard_12");
    let (sk_seed, pk_bytes, alpha, expected_pi, expected_beta) =
        parse_test_vector(content).expect("valid test vector");

    let mut sk = [0u8; 64];
    sk[0..32].copy_from_slice(&sk_seed);
    sk[32..64].copy_from_slice(&pk_bytes);

    let mut pk = [0u8; 32];
    pk.copy_from_slice(&pk_bytes);

    let proof = cardano_vrf_prove(&sk, &alpha).expect("prove should succeed");
    assert_eq!(
        &proof[..],
        expected_pi.as_slice(),
        "proof should match official vector standard_12"
    );

    let beta = cardano_vrf_verify(&pk, &proof, &alpha).expect("verify should succeed");
    assert_eq!(
        &beta[..],
        expected_beta.as_slice(),
        "beta should match official vector standard_12"
    );
}

#[test]
fn test_official_test_vector_generated_2() {
    let content = include_str!("../../../../test-vectors/vrf_ver03_generated_2");
    let (sk_seed, pk_bytes, alpha, expected_pi, expected_beta) =
        parse_test_vector(content).expect("valid test vector");

    let mut sk = [0u8; 64];
    sk[0..32].copy_from_slice(&sk_seed);
    sk[32..64].copy_from_slice(&pk_bytes);

    let mut pk = [0u8; 32];
    pk.copy_from_slice(&pk_bytes);

    let proof = cardano_vrf_prove(&sk, &alpha).expect("prove should succeed");
    assert_eq!(
        &proof[..],
        expected_pi.as_slice(),
        "proof should match official vector generated_2"
    );

    let beta = cardano_vrf_verify(&pk, &proof, &alpha).expect("verify should succeed");
    assert_eq!(
        &beta[..],
        expected_beta.as_slice(),
        "beta should match official vector generated_2"
    );
}

#[test]
fn test_official_test_vector_generated_3() {
    let content = include_str!("../../../../test-vectors/vrf_ver03_generated_3");
    let (sk_seed, pk_bytes, alpha, expected_pi, expected_beta) =
        parse_test_vector(content).expect("valid test vector");

    let mut sk = [0u8; 64];
    sk[0..32].copy_from_slice(&sk_seed);
    sk[32..64].copy_from_slice(&pk_bytes);

    let mut pk = [0u8; 32];
    pk.copy_from_slice(&pk_bytes);

    let proof = cardano_vrf_prove(&sk, &alpha).expect("prove should succeed");
    assert_eq!(
        &proof[..],
        expected_pi.as_slice(),
        "proof should match official vector generated_3"
    );

    let beta = cardano_vrf_verify(&pk, &proof, &alpha).expect("verify should succeed");
    assert_eq!(
        &beta[..],
        expected_beta.as_slice(),
        "beta should match official vector generated_3"
    );
}

#[test]
fn test_official_test_vector_generated_4() {
    let content = include_str!("../../../../test-vectors/vrf_ver03_generated_4");
    let (sk_seed, pk_bytes, alpha, expected_pi, expected_beta) =
        parse_test_vector(content).expect("valid test vector");

    let mut sk = [0u8; 64];
    sk[0..32].copy_from_slice(&sk_seed);
    sk[32..64].copy_from_slice(&pk_bytes);

    let mut pk = [0u8; 32];
    pk.copy_from_slice(&pk_bytes);

    let proof = cardano_vrf_prove(&sk, &alpha).expect("prove should succeed");
    assert_eq!(
        &proof[..],
        expected_pi.as_slice(),
        "proof should match official vector generated_4"
    );

    let beta = cardano_vrf_verify(&pk, &proof, &alpha).expect("verify should succeed");
    assert_eq!(
        &beta[..],
        expected_beta.as_slice(),
        "beta should match official vector generated_4"
    );
}
