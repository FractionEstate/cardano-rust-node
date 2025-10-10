//! VRF proof generation
//!
//! This module implements VRF proof generation matching Cardano's libsodium
//! implementation byte-for-byte.

use curve25519_dalek::{constants::ED25519_BASEPOINT_TABLE, scalar::Scalar};
use sha2::{Digest, Sha512};
use zeroize::Zeroizing;

use super::point::cardano_hash_to_curve;
use crate::vendor::vrf_pure::{
    VrfResult,
    common::{ONE, SUITE_DRAFT03, TWO},
};

// Suite / marker constants imported from common.rs to avoid duplication

/// Generate VRF proof using Cardano-compatible method
///
/// Produces a VRF proof that matches libsodium's output byte-for-byte.
///
/// # Arguments
///
/// * `secret_key` - 64-byte secret key (32-byte seed + 32-byte public key)
/// * `message` - Message to generate proof for
///
/// # Returns
///
/// 80-byte VRF proof consisting of:
/// - 32 bytes: Gamma (VRF output point)
/// - 16 bytes: Challenge c
/// - 32 bytes: Scalar s
///
/// # Algorithm
///
/// 1. Expand secret key using SHA-512
/// 2. Clamp scalar (az\[0\] &= 248, az\[31\] &= 127|64)
/// 3. Hash to curve: H = hash_to_curve(suite || 0x01 || pk || msg)
/// 4. Compute Gamma = x * H
/// 5. Generate nonce k from hash(az[32..] || H)
/// 6. Compute k*B and k*H
/// 7. Compute challenge c = hash(suite || 0x02 || H || Gamma || k*B || k*H)
/// 8. Compute response s = k + c*x (mod L)
/// 9. Return proof (Gamma || c[0..16] || s)
///
/// # Errors
///
/// Returns error if hash-to-curve fails or key is invalid
pub fn cardano_vrf_prove(secret_key: &[u8; 64], message: &[u8]) -> VrfResult<[u8; 80]> {
    // Step 1: Expand secret key
    let mut az = Zeroizing::new([0u8; 64]);
    let mut hasher = Sha512::new();
    hasher.update(&secret_key[0..32]);
    let hash = hasher.finalize();
    az.copy_from_slice(&hash);

    // Step 2: Clamp scalar (same as Ed25519)
    az[0] &= 248;
    az[31] &= 127;
    az[31] |= 64;

    let secret_scalar_bytes: [u8; 32] = az[0..32]
        .try_into()
        .expect("secret key slice must be 32 bytes");
    let x = Scalar::from_bytes_mod_order(secret_scalar_bytes);

    // Extract public key
    let pk = &secret_key[32..64];

    // Step 3: Compute H = hash_to_curve(suite || 0x01 || pk || message)
    let mut h_hasher = Sha512::new();
    h_hasher.update(&[SUITE_DRAFT03]);
    h_hasher.update(&[ONE]);
    h_hasher.update(pk);
    h_hasher.update(message);
    let r_string = h_hasher.finalize();

    let mut r_bytes = [0u8; 32];
    r_bytes.copy_from_slice(&r_string[0..32]);
    r_bytes[31] &= 0x7f; // Clear sign bit per Cardano reference implementation

    // CRITICAL: This must use Cardano-specific hash-to-curve
    let h_point = cardano_hash_to_curve(&r_bytes)?;
    let h_string = h_point.compress().to_bytes();

    // Step 4: Gamma = x * H
    let gamma = h_point * x;

    // Step 5: Generate nonce k
    let mut nonce_hasher = Sha512::new();
    nonce_hasher.update(&az[32..64]);
    nonce_hasher.update(&h_string);
    let nonce_hash = nonce_hasher.finalize();
    let nonce_bytes: [u8; 64] = nonce_hash
        .as_slice()
        .try_into()
        .expect("SHA-512 hash must be 64 bytes");
    let k = Scalar::from_bytes_mod_order_wide(&nonce_bytes);

    // Step 6: k*B and k*H
    let k_b: curve25519_dalek::edwards::EdwardsPoint = &k * ED25519_BASEPOINT_TABLE;
    let k_h = h_point * k;

    let gamma_bytes = gamma.compress().to_bytes();
    let k_b_bytes = k_b.compress().to_bytes();
    let k_h_bytes = k_h.compress().to_bytes();

    // Step 7: Compute challenge c
    let mut c_hasher = Sha512::new();
    c_hasher.update(&[SUITE_DRAFT03]);
    c_hasher.update(&[TWO]);
    c_hasher.update(&h_string);
    c_hasher.update(&gamma_bytes);
    c_hasher.update(&k_b_bytes);
    c_hasher.update(&k_h_bytes);
    let c_hash = c_hasher.finalize();

    // Take first 16 bytes of challenge
    let mut c_bytes = [0u8; 32];
    c_bytes[0..16].copy_from_slice(&c_hash[0..16]);
    let c = Scalar::from_bytes_mod_order(c_bytes);

    // Step 8: s = k + c*x (mod L)
    let s = k + c * x;

    // Step 9: Construct proof
    let mut proof = [0u8; 80];
    proof[0..32].copy_from_slice(&gamma_bytes);
    proof[32..48].copy_from_slice(&c_hash[0..16]);
    proof[48..80].copy_from_slice(s.as_bytes());

    Ok(proof)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prove_compiles() {
        let sk = [0u8; 64];
        let msg = b"test";

        let proof =
            cardano_vrf_prove(&sk, msg).expect("zero secret key should still produce a proof");
        assert_eq!(proof.len(), 80);
        // Gamma should not be all zeros even for a zero scalar because hash-to-curve produces
        // a valid point and the proof concatenates the serialized point.
        assert!(proof[0..32].iter().any(|&b| b != 0));
    }

    #[test]
    fn test_key_clamping() {
        // Verify scalar clamping logic
        let mut az = [0xffu8; 64];
        az[0] &= 248;
        az[31] &= 127;
        az[31] |= 64;

        assert_eq!(az[0] & 0x07, 0);
        assert_eq!(az[31] & 0x80, 0);
        assert_eq!(az[31] & 0x40, 0x40);
    }
}
