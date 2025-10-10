//! Common utilities and types used across VRF implementations

use curve25519_dalek::{
    edwards::{CompressedEdwardsY, EdwardsPoint},
    scalar::Scalar,
};
use sha2::{Digest, Sha512};
use subtle::ConstantTimeEq;

use crate::vendor::vrf_pure::{VrfError, VrfResult};

/// Suite identifier for VRF draft-03
/// Note: Cardano uses 0x04 (ECVRF-ED25519-SHA512-ELL2) for both draft-03 and draft-13
pub const SUITE_DRAFT03: u8 = 0x04;

/// Suite identifier for VRF draft-13
/// Note: Cardano uses the same suite ID (0x04) for both draft-03 and draft-13
pub const SUITE_DRAFT13: u8 = 0x04;

/// Constant byte value 0x01 used in hash computations
pub const ONE: u8 = 0x01;
/// Constant byte value 0x02 used in hash computations
pub const TWO: u8 = 0x02;
/// Constant byte value 0x03 used in hash computations
pub const THREE: u8 = 0x03;

/// Convert bytes to Edwards point, validating the encoding
///
/// # Errors
///
/// Returns `VrfError::InvalidPoint` if the bytes do not represent a valid Edwards point.
pub fn bytes_to_point(bytes: &[u8; 32]) -> VrfResult<EdwardsPoint> {
    CompressedEdwardsY(*bytes)
        .decompress()
        .ok_or(VrfError::InvalidPoint)
}

/// Convert Edwards point to bytes
#[must_use]
pub fn point_to_bytes(point: &EdwardsPoint) -> [u8; 32] {
    point.compress().to_bytes()
}

/// Apply Elligator2 hash-to-curve mapping
///
/// Takes a 32-byte uniform input and maps it to a curve point.
/// This uses the curve25519-dalek nonspec_map_to_curve function.
///
/// Note: We use the deprecated `nonspec_map_to_curve` which implements
/// the Elligator2 mapping we need for VRF. The deprecation warning is
/// about it not being a secure hash function on its own, but in VRF
/// we're already hashing with SHA-512 before calling this.
#[allow(deprecated)]
#[must_use]
pub fn elligator2_hash_to_curve(r: &[u8; 32]) -> [u8; 32] {
    // We need to use nonspec_map_to_curve which expects to hash the input.
    // Since our input is already hashed (from SHA-512), we use it with SHA-512
    // as the secondary hash. This matches the implementation in edwards.rs.
    let point = EdwardsPoint::nonspec_map_to_curve::<Sha512>(r);
    point_to_bytes(&point)
}

/// Reduce a 64-byte hash to a scalar
///
/// # Panics
///
/// This function should not panic in practice as the conversion from `&[u8; 64]` to
/// `&[u8; 64]` is infallible, but it uses `try_into().unwrap()` internally.
#[must_use]
pub fn hash_to_scalar(hash: &[u8; 64]) -> Scalar {
    Scalar::from_bytes_mod_order_wide(hash.try_into().unwrap())
}

/// Negate a scalar (constant-time)
#[must_use]
pub fn scalar_negate(scalar: &Scalar) -> Scalar {
    -scalar
}

/// Check if bytes represent a canonical scalar encoding
#[must_use]
pub fn is_canonical_scalar(bytes: &[u8; 32]) -> bool {
    // A scalar is canonical if it's < L (the group order)
    // The top 4 bits of the last byte must be clear for canonical encoding
    if bytes[31] & 0xf0 != 0 {
        return false;
    }

    // Also verify it can be decoded to a scalar
    Scalar::from_canonical_bytes(*bytes).is_some().into()
}

/// Check if a point has small order (should be rejected)
#[must_use]
pub fn has_small_order(point: &EdwardsPoint) -> bool {
    // Check if point * 8 = identity
    // Small order points are in the 8-torsion subgroup
    point.is_small_order()
}

/// Clear cofactor by multiplying by 8
#[must_use]
pub fn clear_cofactor(point: &EdwardsPoint) -> EdwardsPoint {
    point.mul_by_cofactor()
}

/// Constant-time comparison of 16-byte arrays
#[must_use]
pub fn verify_16(a: &[u8; 16], b: &[u8; 16]) -> bool {
    a.ct_eq(b).into()
}

/// Expand a 32-byte seed to a 64-byte secret key using SHA-512
#[must_use]
pub fn expand_secret_key(seed: &[u8; 32]) -> [u8; 64] {
    let mut hasher = Sha512::new();
    hasher.update(seed);
    let hash = hasher.finalize();

    let mut result = [0u8; 64];
    result.copy_from_slice(&hash);

    // Clamp the scalar as per Ed25519 spec
    result[0] &= 248;
    result[31] &= 127;
    result[31] |= 64;

    result
}

/// Extract the scalar from an expanded secret key
#[must_use]
pub fn secret_key_to_scalar(sk: &[u8; 64]) -> Scalar {
    let mut scalar_bytes = [0u8; 32];
    scalar_bytes.copy_from_slice(&sk[0..32]);
    Scalar::from_bytes_mod_order(scalar_bytes)
}

/// Extract the public key from a secret key (last 32 bytes of 64-byte sk)
#[must_use]
pub fn secret_key_to_public(sk: &[u8; 64]) -> [u8; 32] {
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&sk[32..64]);
    pk
}

/// Generate a public key from a seed
#[must_use]
pub fn seed_to_public_key(seed: &[u8; 32]) -> [u8; 32] {
    let expanded = expand_secret_key(seed);
    let scalar = secret_key_to_scalar(&expanded);
    let point = EdwardsPoint::mul_base(&scalar);
    point_to_bytes(&point)
}

/// Expand seed to 64-byte secret key (seed || public_key)
#[must_use]
pub fn seed_to_secret_key(seed: &[u8; 32]) -> [u8; 64] {
    let pk = seed_to_public_key(seed);
    let mut sk = [0u8; 64];
    sk[0..32].copy_from_slice(seed);
    sk[32..64].copy_from_slice(&pk);
    sk
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elligator2_deterministic() {
        let input = [0u8; 32];
        let point1 = elligator2_hash_to_curve(&input);
        let point2 = elligator2_hash_to_curve(&input);
        assert_eq!(point1, point2, "Elligator2 should be deterministic");
    }

    #[test]
    fn test_scalar_negate() {
        let scalar = Scalar::from(42u64);
        let negated = scalar_negate(&scalar);
        let sum = scalar + negated;
        assert_eq!(sum, Scalar::ZERO, "Scalar + (-Scalar) should be zero");
    }

    #[test]
    fn test_seed_expansion() {
        let seed = [1u8; 32];
        let sk = expand_secret_key(&seed);

        // Check clamping
        assert_eq!(sk[0] & 0x07, 0, "Lowest 3 bits should be clear");
        assert_eq!(sk[31] & 0x80, 0, "Highest bit should be clear");
        assert_eq!(sk[31] & 0x40, 0x40, "Second highest bit should be set");
    }
}
