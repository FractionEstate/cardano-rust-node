//! VRF proof verification
//!
//! This module implements VRF proof verification matching Cardano's libsodium
//! implementation byte-for-byte.

use curve25519_dalek::{
    constants::ED25519_BASEPOINT_TABLE, edwards::CompressedEdwardsY, scalar::Scalar,
};
use sha2::{Digest, Sha512};

use super::point::{cardano_clear_cofactor, cardano_hash_to_curve};
use crate::vendor::vrf_pure::{
    VrfError, VrfResult,
    common::{ONE, SUITE_DRAFT03, THREE, TWO},
};

// Suite / marker constants imported from common.rs to avoid duplication

/// Verify VRF proof using Cardano-compatible method
///
/// Verifies a VRF proof and returns the VRF output if valid.
///
/// # Arguments
///
/// * `public_key` - 32-byte Ed25519 public key
/// * `proof` - 80-byte VRF proof (Gamma || c || s)
/// * `message` - Message that was signed
///
/// # Returns
///
/// 64-byte VRF output if proof is valid
///
/// # Algorithm
///
/// 1. Parse proof components: Gamma, c, s
/// 2. Compute H = hash_to_curve(suite || 0x01 || pk || msg)
/// 3. Verify equation: s*B = k*B + c*Y where k*B = s*B - c*Y
/// 4. Verify equation: s*H = k*H + c*Gamma where k*H = s*H - c*Gamma
/// 5. Recompute challenge c' = hash(suite || 0x02 || H || Gamma || k*B || k*H)
/// 6. Verify c' == c
/// 7. Compute output = hash(suite || 0x03 || Gamma)
///
/// # Errors
///
/// Returns error if proof is invalid, point decompression fails, or hash-to-curve fails
pub fn cardano_vrf_verify(
    public_key: &[u8; 32],
    proof: &[u8; 80],
    message: &[u8],
) -> VrfResult<[u8; 64]> {
    // Step 1: Parse proof components
    let gamma_bytes: [u8; 32] = proof[0..32]
        .try_into()
        .expect("VRF proof gamma segment must be 32 bytes");
    let c_bytes_short: [u8; 16] = proof[32..48]
        .try_into()
        .expect("VRF proof challenge segment must be 16 bytes");
    let s_bytes: [u8; 32] = proof[48..80]
        .try_into()
        .expect("VRF proof scalar segment must be 32 bytes");

    // Parse public key
    let y_point = CompressedEdwardsY(*public_key)
        .decompress()
        .ok_or(VrfError::InvalidPublicKey)?;

    // Parse Gamma
    let gamma = CompressedEdwardsY(gamma_bytes)
        .decompress()
        .ok_or(VrfError::InvalidProof)?;

    // Parse s (must be canonical)
    let s = Option::<Scalar>::from(Scalar::from_canonical_bytes(s_bytes))
        .ok_or(VrfError::InvalidScalar)?;

    // Reconstruct full challenge c
    let mut c_bytes = [0u8; 32];
    c_bytes[0..16].copy_from_slice(&c_bytes_short);
    let c = Scalar::from_bytes_mod_order(c_bytes);

    // Step 2: Compute H = hash_to_curve
    let mut h_hasher = Sha512::new();
    h_hasher.update(&[SUITE_DRAFT03]);
    h_hasher.update(&[ONE]);
    h_hasher.update(public_key);
    h_hasher.update(message);
    let r_string = h_hasher.finalize();

    let mut r_bytes = [0u8; 32];
    r_bytes.copy_from_slice(&r_string[0..32]);
    r_bytes[31] &= 0x7f; // Clear sign bit per Cardano reference implementation

    // CRITICAL: Must use Cardano-specific hash-to-curve
    let h_point = cardano_hash_to_curve(&r_bytes)?;
    let h_string = h_point.compress().to_bytes();

    // Steps 3-4: Verify equations
    // s*B = k*B + c*Y  =>  k*B = s*B - c*Y
    let s_b: curve25519_dalek::edwards::EdwardsPoint = &s * ED25519_BASEPOINT_TABLE;
    let c_y = c * y_point;
    let k_b = s_b - c_y;

    // s*H = k*H + c*Gamma  =>  k*H = s*H - c*Gamma
    let s_h = h_point * s;
    let c_gamma = c * gamma;
    let k_h = s_h - c_gamma;

    let k_b_bytes = k_b.compress().to_bytes();
    let k_h_bytes = k_h.compress().to_bytes();

    // Step 5: Recompute challenge
    let mut c_hasher = Sha512::new();
    c_hasher.update(&[SUITE_DRAFT03]);
    c_hasher.update(&[TWO]);
    c_hasher.update(&h_string);
    c_hasher.update(&gamma_bytes);
    c_hasher.update(&k_b_bytes);
    c_hasher.update(&k_h_bytes);
    let c_hash = c_hasher.finalize();

    // Step 6: Verify challenge matches
    if c_hash[0..16] != c_bytes_short {
        return Err(VrfError::VerificationFailed);
    }

    // Step 7: Compute VRF output
    let gamma_cleared = cardano_clear_cofactor(&gamma);
    let mut output_hasher = Sha512::new();
    output_hasher.update(&[SUITE_DRAFT03]);
    output_hasher.update(&[THREE]);
    output_hasher.update(&gamma_cleared.compress().to_bytes());
    let output_hash = output_hasher.finalize();

    let mut output = [0u8; 64];
    output.copy_from_slice(&output_hash);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_compiles() {
        let pk = [0u8; 32];
        let proof = [0u8; 80];
        let msg = b"test";

        // Will fail until hash_to_curve is implemented
        let result = cardano_vrf_verify(&pk, &proof, msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_challenge_reconstruction() {
        // Verify challenge bytes are correctly padded
        let c_short = [1u8; 16];
        let mut c_full = [0u8; 32];
        c_full[0..16].copy_from_slice(&c_short);

        assert_eq!(&c_full[0..16], &c_short);
        assert_eq!(&c_full[16..32], &[0u8; 16]);
    }
}
