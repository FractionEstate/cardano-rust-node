//! Edwards point operations and cofactor clearing
//!
//! This module implements operations on Edwards curve points, including
//! Cardano-specific cofactor clearing that differs from standard implementations.

use crate::vendor::vrf_pure::cardano_compat::{
    debug,
    field::FieldElement,
    montgomery::{ge25519_elligator2_faithful, ge25519_mont_to_ed_faithful},
};
use crate::vendor::vrf_pure::{VrfError, VrfResult};
use core::fmt::Write;
use curve25519_dalek::edwards::{CompressedEdwardsY, EdwardsPoint};
use num_bigint::BigUint;
use num_traits::{One, Zero};
use once_cell::sync::Lazy;
use sha2::{Digest, Sha512};

// Curve / field constants as BigUint lazily initialized
static P: Lazy<BigUint> = Lazy::new(|| {
    // 2^255 - 19
    let mut p = BigUint::one() << 255;
    p -= BigUint::from(19u8);
    p
});

static A_BIG: Lazy<BigUint> = Lazy::new(|| BigUint::from(486662u32));
static SQRT_M1_BIG: Lazy<BigUint> = Lazy::new(|| {
    // sqrt(-1) mod p (ed25519 constant) = 2^((p-1)/4) mod p
    let exp = (&*P - BigUint::one()) >> 2; // (p-1)/4
    mod_pow(&BigUint::from(2u8), &exp) // not minimal but sufficient; alternative known constant could be hard-coded
});
/// sqrt(-A-2) mod p (Cardano constant)
static SQRT_AM2_BIG: Lazy<BigUint> = Lazy::new(|| {
    let neg_a_minus_two = mod_neg(&mod_add(&A_BIG, &BigUint::from(2u8))); // -(A+2)
    sqrt_mod(&neg_a_minus_two).expect("sqrt(-A-2) must exist")
});

#[allow(dead_code)]
fn ge25519_from_hash_field(hash: &[u8; 64]) -> VrfResult<(EdwardsPoint, [u8; 32])> {
    let fe_f = fe_reduce64(hash);
    let (x_mont, y_mont, notsquare) = ge25519_elligator2_faithful(&fe_f);
    let y_sign = notsquare ^ 1;
    let mut y_adj = y_mont;
    if (y_adj.is_negative() as u8) ^ y_sign == 1 {
        y_adj = (-y_adj).reduce();
    }
    let (ed_x, ed_y) = ge25519_mont_to_ed_faithful(&x_mont, &y_adj);
    let mut ed_y_bytes = ed_y.to_bytes();
    ed_y_bytes[31] = (ed_y_bytes[31] & 0x7f) | ((ed_x.is_negative() as u8) << 7);
    let point = CompressedEdwardsY(ed_y_bytes)
        .decompress()
        .ok_or(VrfError::InvalidPoint)?;
    let cleared = cardano_clear_cofactor(&point);
    Ok((cleared, cleared.compress().to_bytes()))
}

/// Convert little-endian bytes to BigUint modulo p
fn bytes_to_biguint(bytes: &[u8; 32]) -> BigUint {
    BigUint::from_bytes_le(bytes) % &*P
}

/// Convert BigUint (already reduced) to 32-byte little-endian array
fn biguint_to_bytes(value: &BigUint) -> [u8; 32] {
    let mut bytes = value.to_bytes_le();
    bytes.resize(32, 0);
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    out
}

fn mod_add(a: &BigUint, b: &BigUint) -> BigUint {
    let mut res = a + b;
    if res >= *P {
        res -= &*P;
    }
    res
}

fn mod_sub(a: &BigUint, b: &BigUint) -> BigUint {
    if a >= b { a - b } else { &*P - (b - a) }
}

fn mod_neg(a: &BigUint) -> BigUint {
    if a.is_zero() {
        BigUint::zero()
    } else {
        &*P - (a % &*P)
    }
}

fn mod_mul(a: &BigUint, b: &BigUint) -> BigUint {
    ((a % &*P) * (b % &*P)) % &*P
}

fn mod_pow(base: &BigUint, exp: &BigUint) -> BigUint {
    base.modpow(exp, &P)
}

fn mod_inv(a: &BigUint) -> BigUint {
    if a.is_zero() {
        BigUint::zero()
    } else {
        let exp = &*P - BigUint::from(2u8);
        mod_pow(a, &exp)
    }
}

fn is_quadratic_residue(n: &BigUint) -> bool {
    if n.is_zero() {
        return true;
    }
    let exp = (&*P - BigUint::one()) >> 1;
    mod_pow(&(n % &*P), &exp) == BigUint::one()
}

fn sqrt_mod(n: &BigUint) -> Option<BigUint> {
    let n = n % &*P;
    if n.is_zero() {
        return Some(BigUint::zero());
    }
    let legendre = mod_pow(&n, &((&*P - BigUint::one()) >> 1));
    if legendre != BigUint::one() {
        return None;
    }
    let exp = (&*P + BigUint::from(3u8)) >> 3;
    let mut root = mod_pow(&n, &exp);
    if mod_mul(&root, &root) != n {
        root = mod_mul(&root, &SQRT_M1_BIG);
    }
    if mod_mul(&root, &root) == n {
        Some(root)
    } else {
        None
    }
}

fn mont_rhs(x: &BigUint) -> BigUint {
    let x_sq = mod_mul(x, x);
    let x_cu = mod_mul(&x_sq, x);
    let ax_sq = mod_mul(&A_BIG, &x_sq);
    mod_add(&mod_add(&x_cu, &ax_sq), x)
}

fn mont_to_edwards(x: &BigUint, y: &BigUint) -> (BigUint, BigUint) {
    let one = BigUint::one();
    let x_plus_one = mod_add(x, &one);
    let x_minus_one = mod_sub(x, &one);
    let denom = mod_mul(&x_plus_one, y);
    let denom_inv = if denom.is_zero() {
        BigUint::zero()
    } else {
        mod_inv(&denom)
    };
    let mut ed_x = mod_mul(&mod_mul(&SQRT_AM2_BIG, x), &denom_inv);
    ed_x = mod_mul(&ed_x, &x_plus_one);
    let mut ed_y = mod_mul(&denom_inv, y);
    ed_y = mod_mul(&ed_y, &x_minus_one);
    if denom_inv.is_zero() {
        ed_y = one;
    }
    (ed_x, ed_y)
}

// Default BigUint-based ge25519_from_hash (stable)
#[cfg(not(feature = "field-h2c-experimental"))]
fn ge25519_from_hash(hash: &[u8; 64]) -> VrfResult<(EdwardsPoint, [u8; 32])> {
    let fe_f = fe_reduce64(hash);
    let r_bytes = fe_f.to_bytes();
    let r_val = bytes_to_biguint(&r_bytes);
    let mut rr2 = mod_mul(&r_val, &r_val);
    rr2 = mod_mul(&rr2, &BigUint::from(2u8));
    let denom = mod_add(&rr2, &BigUint::one());
    let denom_inv = mod_inv(&denom);
    let mut x = mod_neg(&mod_mul(&denom_inv, &A_BIG));
    let mut gx1 = mont_rhs(&x);
    let notsquare = !is_quadratic_residue(&gx1);
    if notsquare {
        x = mod_sub(&mod_neg(&x), &A_BIG);
        gx1 = mont_rhs(&x);
    }
    let mut y = sqrt_mod(&gx1).ok_or(VrfError::InvalidPoint)?;
    let y_sign = (!notsquare) as u8;
    if (is_negative(&y) as u8) ^ y_sign == 1 {
        y = mod_neg(&y);
    }
    let y_inv = mod_inv(&y);
    let x_ed = mod_mul(&mod_mul(&SQRT_AM2_BIG, &x), &y_inv);
    let x_minus_one = mod_sub(&x, &BigUint::one());
    let x_plus_one = mod_add(&x, &BigUint::one());
    let y_ed = mod_mul(&x_minus_one, &mod_inv(&x_plus_one));
    let mut y_bytes = biguint_to_bytes(&y_ed);
    let sign_bit = is_negative(&x_ed) as u8;
    y_bytes[31] = (y_bytes[31] & 0x7f) | (sign_bit << 7);
    let point = CompressedEdwardsY(y_bytes)
        .decompress()
        .ok_or(VrfError::InvalidPoint)?;
    let cleared = cardano_clear_cofactor(&point);
    let cleared_bytes = cleared.compress().to_bytes();
    Ok((cleared, cleared_bytes))
}

#[cfg(feature = "field-h2c-experimental")]
fn ge25519_from_hash(hash: &[u8; 64]) -> VrfResult<(EdwardsPoint, [u8; 32])> {
    ge25519_from_hash_field(hash)
}

fn is_negative(fe: &BigUint) -> bool {
    (fe % &*P) % 2u8 == BigUint::one()
}

fn hash_to_curve_bigint(r: &[u8; 32], x_sign: u8) -> VrfResult<EdwardsPoint> {
    let r_val = bytes_to_biguint(r);

    let mut rr2 = mod_mul(&r_val, &r_val);
    rr2 = mod_mul(&rr2, &BigUint::from(2u8));
    let denom = mod_add(&rr2, &BigUint::one());
    let denom_inv = mod_inv(&denom);

    let mut x = mod_neg(&mod_mul(&denom_inv, &A_BIG));
    let mut gx1 = mont_rhs(&x);

    if !is_quadratic_residue(&gx1) {
        x = mod_sub(&mod_neg(&x), &A_BIG);
        gx1 = mont_rhs(&x);
    }

    let y = match sqrt_mod(&gx1) {
        Some(value) => value,
        None => {
            debug::log(|| {
                format!(
                    "hash_to_curve_bigint: sqrt_mod failed\n  r = {}\n  gx1 = {}",
                    biguint_to_hex(&r_val),
                    biguint_to_hex(&gx1),
                )
            });
            return Err(VrfError::InvalidPoint);
        },
    };
    let (mut ed_x, ed_y) = mont_to_edwards(&x, &y);

    if is_negative(&ed_x) ^ (x_sign != 0) {
        ed_x = mod_neg(&ed_x);
    }

    let mut compressed = biguint_to_bytes(&ed_y);
    let sign_bit = is_negative(&ed_x) as u8;
    compressed[31] = (compressed[31] & 0x7f) | (sign_bit << 7);
    let ed_y_bytes = compressed;

    let ed_x_bytes = biguint_to_bytes(&ed_x);

    let compressed_point = CompressedEdwardsY(compressed);
    let point = compressed_point.decompress().ok_or_else(|| {
        debug::log(|| {
            format!(
                "ge25519_from_hash: decompress failed\n  x = {}\n  y = {}\n  x_sign = {}",
                to_hex(&ed_x_bytes),
                to_hex(&ed_y_bytes),
                sign_bit,
            )
        });
        VrfError::InvalidPoint
    })?;

    // Apply cofactor clearing and re-serialize to get correct sign bit
    let cleared = cardano_clear_cofactor(&point);

    Ok(cleared)
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut out, "{:02x}", byte).expect("writing to String");
    }
    out
}

fn biguint_to_hex(value: &BigUint) -> String {
    let mut hex = value.to_str_radix(16);
    if hex.is_empty() {
        hex.push('0');
    }
    hex
}

fn fe_to_hex(fe: &FieldElement) -> String {
    to_hex(&fe.to_bytes())
}

/// Cardano-specific hash-to-curve function
///
/// Maps a uniform 32-byte value to a point on the Edwards curve using the exact
/// procedure implemented by libsodium's `cardano_ge25519_from_uniform`.
///
/// # Arguments
///
/// * `r` - 32-byte uniform random value
pub fn cardano_hash_to_curve(r: &[u8; 32]) -> VrfResult<EdwardsPoint> {
    let sign = (r[31] >> 7) & 1;
    let mut r_masked = *r;
    r_masked[31] &= 0x7f;
    hash_to_curve_bigint(&r_masked, sign)
}

/// Cardano-specific cofactor clearing
///
/// Multiplies point by cofactor (8) using Cardano's method.
#[must_use]
pub fn cardano_clear_cofactor(point: &EdwardsPoint) -> EdwardsPoint {
    use curve25519_dalek::scalar::Scalar;
    let eight = Scalar::from(8u8);
    eight * point
}

const DRAFT13_CONTEXT: &[u8] = b"ECVRF_edwards25519_XMD:SHA-512_ELL2_NU_\x04";

fn h2c_string_to_hash_sha512(ctx: &[u8], msg: &[u8], out_len: usize) -> Vec<u8> {
    const HASH_BYTES: usize = 64;
    const HASH_BLOCKBYTES: usize = 128;

    assert!(out_len <= u8::MAX as usize);

    let mut ctx_buf = ctx.to_vec();
    if ctx_buf.len() > u8::MAX as usize {
        let mut hasher = Sha512::new();
        hasher.update(b"H2C-OVERSIZE-DST-");
        hasher.update(&ctx_buf);
        ctx_buf = hasher.finalize().to_vec();
    }

    let ctx_len_u8 = ctx_buf.len() as u8;
    let empty_block = [0u8; HASH_BLOCKBYTES];
    let mut t = [0u8; 3];
    t[1] = out_len as u8;

    let mut hasher = Sha512::new();
    hasher.update(&empty_block);
    hasher.update(msg);
    hasher.update(&t);
    hasher.update(&ctx_buf);
    hasher.update(&[ctx_len_u8]);
    let u0 = hasher.finalize();

    let mut ux = [0u8; HASH_BYTES];
    let mut output = vec![0u8; out_len];
    let mut offset = 0usize;

    while offset < out_len {
        for (dst, src) in ux.iter_mut().zip(u0.iter()) {
            *dst ^= *src;
        }

        t[2] = t[2].wrapping_add(1);

        let mut round = Sha512::new();
        round.update(&ux);
        round.update(&[t[2]]);
        round.update(&ctx_buf);
        round.update(&[ctx_len_u8]);
        let hashed = round.finalize();
        ux.copy_from_slice(&hashed);

        let remaining = out_len - offset;
        let chunk = remaining.min(HASH_BYTES);
        output[offset..offset + chunk].copy_from_slice(&ux[..chunk]);
        offset += chunk;
    }

    output
}

fn fe_reduce64(hash: &[u8; 64]) -> FieldElement {
    let mut fl = [0u8; 32];
    fl.copy_from_slice(&hash[..32]);
    fl[31] &= 0x7f;

    let mut gl = [0u8; 32];
    gl.copy_from_slice(&hash[32..]);
    gl[31] &= 0x7f;

    let mut fe_f = FieldElement::from_bytes(&fl);
    let fe_g = FieldElement::from_bytes(&gl);

    let mut f_limbs = fe_f.0;
    let g_limbs = fe_g.0;

    f_limbs[0] += (i64::from(hash[31] >> 7) * 19) + (i64::from(hash[63] >> 7) * 722);
    for i in 0..f_limbs.len() {
        f_limbs[i] += 38 * g_limbs[i];
    }

    fe_f.0 = f_limbs;
    let result = fe_f.reduce();

    debug::log(|| {
        format!(
            "fe_reduce64:\n  hash = {}\n  fl = {}\n  gl = {}\n  result = {}",
            to_hex(hash),
            to_hex(&fl),
            to_hex(&gl),
            fe_to_hex(&result),
        )
    });

    result
}

// (Removed FieldElement experimental ge25519_from_hash duplicate; retained as ge25519_from_hash_field above)
/// Hash-to-curve helper for draft-13 batch-compatible VRF.
///
/// Produces the cofactor-cleared Edwards point and its compressed encoding for
/// the given verification key and message, following the Cardano libsodium
/// implementation (XMD:SHA-512 Elligator2 map) exactly.
pub fn cardano_hash_to_curve_draft13(
    pk: &[u8],
    message: &[u8],
) -> VrfResult<(EdwardsPoint, [u8; 32])> {
    if pk.len() != 32 {
        return Err(VrfError::InvalidPublicKey);
    }

    const HASH_GE_L: usize = 48;

    let mut string_to_hash = Vec::with_capacity(32 + message.len());
    string_to_hash.extend_from_slice(pk);
    string_to_hash.extend_from_slice(message);

    let h_be = h2c_string_to_hash_sha512(DRAFT13_CONTEXT, &string_to_hash, HASH_GE_L);

    let mut h = [0u8; 64];
    for j in 0..HASH_GE_L {
        h[j] = h_be[HASH_GE_L - 1 - j];
    }

    debug::log(|| {
        format!(
            "cardano_hash_to_curve_draft13::h2c \n  h_be = {}\n  h_le = {}",
            to_hex(&h_be),
            to_hex(&h[..HASH_GE_L])
        )
    });

    ge25519_from_hash(&h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use curve25519_dalek::constants::ED25519_BASEPOINT_POINT;
    use curve25519_dalek::traits::Identity;

    #[test]
    fn test_cofactor_clearing_with_basepoint() {
        let cleared = cardano_clear_cofactor(&ED25519_BASEPOINT_POINT);

        let identity = EdwardsPoint::identity();
        assert_ne!(
            cleared.compress().as_bytes(),
            identity.compress().as_bytes()
        );

        use curve25519_dalek::scalar::Scalar;
        let eight = Scalar::from(8u8);
        let expected = eight * ED25519_BASEPOINT_POINT;
        assert_eq!(cleared, expected);
    }
}
