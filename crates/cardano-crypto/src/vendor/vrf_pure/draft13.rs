//! VRF implementation following IETF draft-13 (batch-compatible variant)
//!
//! This implements ECVRF-ED25519-SHA512-TAI with batch verification support

#![allow(clippy::unwrap_used)]

use curve25519_dalek::{edwards::EdwardsPoint, scalar::Scalar, traits::VartimeMultiscalarMul};
use sha2::{Digest, Sha512};
use zeroize::Zeroizing;

use crate::vendor::vrf_pure::cardano_compat::point::{cardano_clear_cofactor, cardano_hash_to_curve_draft13};
use crate::vendor::vrf_pure::common::*;
use crate::vendor::vrf_pure::{VrfError, VrfResult};

/// VRF proof size for draft-13 batch-compatible (128 bytes)
pub const PROOF_SIZE: usize = 128;

/// Public key size (32 bytes)
pub const PUBLIC_KEY_SIZE: usize = 32;

/// Secret key size (64 bytes: 32-byte seed + 32-byte public key)
pub const SECRET_KEY_SIZE: usize = 64;

/// Seed size (32 bytes)
pub const SEED_SIZE: usize = 32;

/// Output size (64 bytes)
pub const OUTPUT_SIZE: usize = 64;

/// VRF Draft-13 batch-compatible implementation
#[derive(Clone)]
pub struct VrfDraft13;

impl VrfDraft13 {
    /// Generate a VRF proof (batch-compatible)
    ///
    /// # Arguments
    /// * `secret_key` - 64-byte secret key (32-byte seed + 32-byte public key)
    /// * `message` - Message to prove
    ///
    /// # Returns
    /// 128-byte proof
    pub fn prove(
        secret_key: &[u8; SECRET_KEY_SIZE],
        message: &[u8],
    ) -> VrfResult<[u8; PROOF_SIZE]> {
        // Expand the secret key
        let mut az = Zeroizing::new([0u8; 64]);
        let mut hasher = Sha512::new();
        hasher.update(&secret_key[0..32]); // Hash the seed part
        let hash = hasher.finalize();
        az.copy_from_slice(&hash);

        // Clamp the scalar
        az[0] &= 248;
        az[31] &= 127;
        az[31] |= 64;

        let x = Scalar::from_bytes_mod_order(az[0..32].try_into().unwrap());

        // Extract public key
        let pk = &secret_key[32..64];

        let (h_point, h_string) = cardano_hash_to_curve_draft13(pk, message)?;

        // Gamma = x * H
        let gamma = h_point * x;

        // Compute nonce = hash(az[32..64] || H_string)
        let mut nonce_hasher = Sha512::new();
        nonce_hasher.update(&az[32..64]);
        nonce_hasher.update(&h_string);
        let nonce_hash = nonce_hasher.finalize();
        let k = Scalar::from_bytes_mod_order_wide(&nonce_hash.as_slice().try_into().unwrap());

        // k*B and k*H
        let k_b = EdwardsPoint::mul_base(&k);
        let k_h = h_point * k;

        let gamma_bytes = point_to_bytes(&gamma);
        let k_b_bytes = point_to_bytes(&k_b);
        let k_h_bytes = point_to_bytes(&k_h);

        // Compute challenge c = hash(suite || 0x02 || pk || H || Gamma || k*B || k*H || 0x00)
        // For batch-compatible draft-13, we still truncate to 16 bytes like draft-03
        let mut c_hasher = Sha512::new();
        c_hasher.update(&[SUITE_DRAFT13]);
        c_hasher.update(&[TWO]);
        c_hasher.update(pk); // Include public key
        c_hasher.update(&h_string);
        c_hasher.update(&gamma_bytes);
        c_hasher.update(&k_b_bytes);
        c_hasher.update(&k_h_bytes);
        c_hasher.update(&[0u8]); // ZERO byte
        let c_hash = c_hasher.finalize();

        // Take first 16 bytes of challenge (truncated, same as draft-03)
        let mut c_bytes = [0u8; 32];
        c_bytes[0..16].copy_from_slice(&c_hash[0..16]);
        // Remaining bytes are zero
        let c = Scalar::from_bytes_mod_order(c_bytes);

        // s = k + c*x (mod L)
        let s = k + c * x;

        // Construct batch-compatible proof: Gamma || k*B || k*H || s
        // Total: 32 + 32 + 32 + 32 = 128 bytes
        let mut proof = [0u8; PROOF_SIZE];
        proof[0..32].copy_from_slice(&gamma_bytes);
        proof[32..64].copy_from_slice(&k_b_bytes); // k*B for batch verification
        proof[64..96].copy_from_slice(&k_h_bytes); // k*H for batch verification
        proof[96..128].copy_from_slice(s.as_bytes());

        Ok(proof)
    }

    /// Verify a VRF proof and return the output
    ///
    /// # Arguments
    /// * `public_key` - 32-byte public key
    /// * `proof` - 128-byte proof
    /// * `message` - Message that was proven
    ///
    /// # Returns
    /// 64-byte VRF output on success
    pub fn verify(
        public_key: &[u8; PUBLIC_KEY_SIZE],
        proof: &[u8; PROOF_SIZE],
        message: &[u8],
    ) -> VrfResult<[u8; OUTPUT_SIZE]> {
        // Parse public key
        let y_point = bytes_to_point(public_key)?;

        // Check for small order
        if has_small_order(&y_point) {
            return Err(VrfError::InvalidPublicKey);
        }

        // Parse proof: Gamma || k*B || k*H || s
        let gamma_bytes: [u8; 32] = proof[0..32].try_into().unwrap();
        let k_b_bytes: [u8; 32] = proof[32..64].try_into().unwrap();
        let k_h_bytes: [u8; 32] = proof[64..96].try_into().unwrap();
        let s_bytes: [u8; 32] = proof[96..128].try_into().unwrap();

        // Validate Gamma
        let gamma = bytes_to_point(&gamma_bytes)?;

        // Validate s
        if !is_canonical_scalar(&s_bytes) {
            return Err(VrfError::InvalidProof);
        }

        let s = Scalar::from_canonical_bytes(s_bytes)
            .map(|s| s)
            .unwrap_or(Scalar::ZERO);
        if s == Scalar::ZERO && s_bytes != [0u8; 32] {
            return Err(VrfError::InvalidScalar);
        }

        let (h_point, h_string) = cardano_hash_to_curve_draft13(public_key, message)?;

        // Compute U = s*B - c*Y
        // We need to compute c first from k*B, k*H
        let mut c_hasher = Sha512::new();
        c_hasher.update(&[SUITE_DRAFT13]);
        c_hasher.update(&[TWO]);
        c_hasher.update(public_key);
        c_hasher.update(&h_string);
        c_hasher.update(&gamma_bytes);
        c_hasher.update(&k_b_bytes);
        c_hasher.update(&k_h_bytes);
        c_hasher.update(&[0u8]); // ZERO byte
        let c_hash = c_hasher.finalize();

        // Truncate challenge to 16 bytes (same as draft-03)
        let mut c_bytes = [0u8; 32];
        c_bytes[0..16].copy_from_slice(&c_hash[0..16]);
        let c = Scalar::from_bytes_mod_order(c_bytes);

        let neg_c = scalar_negate(&c);
        let u = EdwardsPoint::vartime_multiscalar_mul(
            &[s, neg_c],
            &[EdwardsPoint::mul_base(&Scalar::ONE), y_point],
        );

        // Compute V = s*H - c*Gamma
        let v = EdwardsPoint::vartime_multiscalar_mul(&[s, neg_c], &[h_point, gamma]);

        let u_bytes = point_to_bytes(&u);
        let v_bytes = point_to_bytes(&v);

        // k*B should equal U, k*H should equal V
        if k_b_bytes != u_bytes {
            return Err(VrfError::VerificationFailed);
        }
        if k_h_bytes != v_bytes {
            return Err(VrfError::VerificationFailed);
        }

        // Compute output
        Self::proof_to_hash(proof)
    }

    /// Convert a proof to VRF output hash
    ///
    /// # Arguments
    /// * `proof` - 128-byte proof
    ///
    /// # Returns
    /// 64-byte VRF output
    pub fn proof_to_hash(proof: &[u8; PROOF_SIZE]) -> VrfResult<[u8; OUTPUT_SIZE]> {
        let gamma_bytes: [u8; 32] = proof[0..32].try_into().unwrap();
        let gamma = bytes_to_point(&gamma_bytes)?;

        // Clear cofactor
        let gamma_cleared = cardano_clear_cofactor(&gamma);
        let gamma_cleared_bytes = point_to_bytes(&gamma_cleared);

        // beta = hash(suite || 0x03 || cofactor*Gamma)
        let mut hasher = Sha512::new();
        hasher.update(&[SUITE_DRAFT13]);
        hasher.update(&[THREE]);
        hasher.update(&gamma_cleared_bytes);
        hasher.update(&[0u8]);
        let beta = hasher.finalize();

        let mut output = [0u8; OUTPUT_SIZE];
        output.copy_from_slice(&beta);
        Ok(output)
    }

    /// Generate keypair from seed
    #[must_use]
    pub fn keypair_from_seed(
        seed: &[u8; SEED_SIZE],
    ) -> ([u8; SECRET_KEY_SIZE], [u8; PUBLIC_KEY_SIZE]) {
        let sk = seed_to_secret_key(seed);
        let pk = secret_key_to_public(&sk);
        (sk, pk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex::FromHex;

    #[test]
    fn test_prove_verify_roundtrip() {
        let seed = [42u8; SEED_SIZE];
        let (sk, pk) = VrfDraft13::keypair_from_seed(&seed);
        let message = b"test message";

        let proof = VrfDraft13::prove(&sk, message).expect("prove failed");
        let output = VrfDraft13::verify(&pk, &proof, message).expect("verify failed");

        assert_eq!(output.len(), OUTPUT_SIZE);
    }

    #[test]
    fn test_verify_rejects_invalid_proof() {
        let seed = [42u8; SEED_SIZE];
        let (_, pk) = VrfDraft13::keypair_from_seed(&seed);
        let message = b"test message";

        let mut bad_proof = [0u8; PROOF_SIZE];
        bad_proof[0] = 1; // Invalid proof

        assert!(VrfDraft13::verify(&pk, &bad_proof, message).is_err());
    }

    #[test]
    fn test_proof_size() {
        assert_eq!(
            PROOF_SIZE, 128,
            "Draft-13 batch-compatible proof should be 128 bytes"
        );
    }

    #[test]
    fn test_official_vector_generated_1() {
        let seed = [0u8; SEED_SIZE];
        let pk_bytes = <[u8; PUBLIC_KEY_SIZE]>::from_hex(
            "3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29",
        )
        .expect("valid hex pk");

        let mut sk = [0u8; SECRET_KEY_SIZE];
        sk[..SEED_SIZE].copy_from_slice(&seed);
        sk[SEED_SIZE..].copy_from_slice(&pk_bytes);

        let message = [0u8];

        let expected_proof = <[u8; PROOF_SIZE]>::from_hex(
            "93d70c5ed59ccb21ca9991be561756939ff9753bf85764d2a7b937d6fbf9183443cd118bee8a0f61e8bdc5403c03d6c94ead31956e98bfd6a5e02d3be5900d17a540852d586f0891caed3e3b0e0871d6a741fb0edcdb586f7f10252f79c35176474ece4936e0190b5167832c10712884ad12acdfff2e434aacb165e1f789660f",
        )
        .expect("valid hex proof");

        let expected_beta = <[u8; OUTPUT_SIZE]>::from_hex(
            "9a4d34f87003412e413ca42feba3b6158bdf11db41c2bbde98961c5865400cfdee07149b928b376db365c5d68459378b0981f1cb0510f1e0c194c4a17603d44d",
        )
        .expect("valid beta");

        let proof = VrfDraft13::prove(&sk, &message).expect("prove failed");
        assert_eq!(proof, expected_proof, "proof mismatch");

        let output = VrfDraft13::verify(&pk_bytes, &proof, &message).expect("verify failed");
        assert_eq!(output, expected_beta, "verify output mismatch");

        let beta = VrfDraft13::proof_to_hash(&proof).expect("proof_to_hash failed");
        assert_eq!(beta, expected_beta, "proof_to_hash mismatch");
    }

    #[test]
    fn test_official_vector_standard_10() {
        // IETF draft-13 example 10 (from test vector vrf_ver13_standard_10)
        let seed_hex = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
        let pk_hex = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";

        let seed = <[u8; 32]>::from_hex(seed_hex).expect("valid hex seed");
        let pk_bytes = <[u8; PUBLIC_KEY_SIZE]>::from_hex(pk_hex).expect("valid hex pk");
        let mut sk = [0u8; SECRET_KEY_SIZE];
        sk[..SEED_SIZE].copy_from_slice(&seed);
        sk[SEED_SIZE..].copy_from_slice(&pk_bytes);

        let message = b""; // empty message

        let expected_proof = <[u8; PROOF_SIZE]>::from_hex(
            "7d9c633ffeee27349264cf5c667579fc583b4bda63ab71d001f89c10003ab46f762f5c178b68f0cddcc1157918edf45ec334ac8e8286601a3256c3bbf858edd94652eba1c4612e6fce762977a59420b451e12964adbe4fbecd58a7aeff5860afcafa73589b023d14311c331a9ad15ff2fb37831e00f0acaa6d73bc9997b06501",
        )
        .expect("valid hex proof");

        let expected_beta = <[u8; OUTPUT_SIZE]>::from_hex(
            "9d574bf9b8302ec0fc1e21c3ec5368269527b87b462ce36dab2d14ccf80c53cccf6758f058c5b1c856b116388152bbe509ee3b9ecfe63d93c3b4346c1fbc6c54",
        )
        .expect("valid beta");

        let proof = VrfDraft13::prove(&sk, message).expect("prove failed");

        assert_eq!(proof, expected_proof, "proof mismatch");

        let output = VrfDraft13::verify(&pk_bytes, &proof, message).expect("verify failed");
        assert_eq!(output, expected_beta, "verify output mismatch");

        let beta = VrfDraft13::proof_to_hash(&proof).expect("proof_to_hash failed");
        assert_eq!(beta, expected_beta, "proof_to_hash mismatch");
    }
}
