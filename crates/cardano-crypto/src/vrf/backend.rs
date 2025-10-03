//! VRF backend implementation using curve25519-dalek
//!
//! This module contains the low-level cryptographic operations for VRF.

#![allow(clippy::needless_borrows_for_generic_args)]

use core::ops::Neg;

use curve25519_dalek::constants::ED25519_BASEPOINT_POINT;
use curve25519_dalek::edwards::{CompressedEdwardsY, EdwardsPoint};
use curve25519_dalek::field::FieldElement;
use curve25519_dalek::scalar::Scalar;
use curve25519_dalek::traits::VartimeMultiscalarMul;
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha512};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

const VRF_PUBLIC_KEY_LENGTH: usize = 32;
const VRF_SECRET_KEY_LENGTH: usize = 64;
const VRF_SEED_LENGTH: usize = 32;
const VRF_PROOF_LENGTH: usize = 80;
const VRF_OUTPUT_LENGTH: usize = 64;

const SUITE: u8 = 0x04;
const ZERO: u8 = 0x00;
const TWO: u8 = 0x02;
const THREE: u8 = 0x03;

const DST: &[u8] = b"ECVRF_edwards25519_XMD:SHA-512_ELL2_NU_\x04";
const BLOCK_SIZE_SHA512: usize = 128;
const HASH_SIZE_SHA512: usize = 64;

const CURVE25519_A_BYTES: [u8; 32] = [
    0x06, 0x6d, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const SQRT_MINUS_A_MINUS_TWO_BYTES: [u8; 32] = [
    0x06, 0x7e, 0x45, 0xff, 0xaa, 0x04, 0x6e, 0xcc, 0x82, 0x1a, 0x7d, 0x4b, 0xd1, 0xd3, 0xa1, 0xc5,
    0x7e, 0x4f, 0xfc, 0x03, 0xdc, 0x08, 0x7b, 0xd2, 0xbb, 0x06, 0xa0, 0x60, 0xf4, 0xed, 0x26, 0x0f,
];

#[derive(Debug)]
enum BackendError {
    InvalidLengths,
    InvalidKey,
    InvalidProof,
    HashToCurve,
}

impl From<BackendError> for i32 {
    fn from(_: BackendError) -> Self {
        -1
    }
}

pub fn vrf_public_key_bytes() -> usize {
    VRF_PUBLIC_KEY_LENGTH
}

pub fn vrf_secret_key_bytes() -> usize {
    VRF_SECRET_KEY_LENGTH
}

pub fn vrf_seed_bytes() -> usize {
    VRF_SEED_LENGTH
}

pub fn vrf_proof_bytes() -> usize {
    VRF_PROOF_LENGTH
}

pub fn vrf_output_bytes() -> usize {
    VRF_OUTPUT_LENGTH
}

pub fn ensure_backend_init() -> Result<(), &'static str> {
    // Pure Rust backend does not require runtime initialisation.
    Ok(())
}

pub fn random_keypair(pk: &mut [u8], sk: &mut [u8]) -> i32 {
    if pk.len() != VRF_PUBLIC_KEY_LENGTH || sk.len() != VRF_SECRET_KEY_LENGTH {
        return BackendError::InvalidLengths.into();
    }

    let mut seed = [0u8; VRF_SEED_LENGTH];
    OsRng.fill_bytes(&mut seed);
    let result = seed_keypair(pk, sk, &seed);
    seed.zeroize();
    result
}

pub fn seed_keypair(pk: &mut [u8], sk: &mut [u8], seed: &[u8]) -> i32 {
    if pk.len() != VRF_PUBLIC_KEY_LENGTH
        || sk.len() != VRF_SECRET_KEY_LENGTH
        || seed.len() != VRF_SEED_LENGTH
    {
        return BackendError::InvalidLengths.into();
    }

    let mut seed_array = [0u8; VRF_SEED_LENGTH];
    seed_array.copy_from_slice(seed);

    let mut expanded = expand_secret(&seed_array);
    let public = (ED25519_BASEPOINT_POINT * expanded.scalar)
        .compress()
        .to_bytes();

    pk.copy_from_slice(&public);
    sk[..VRF_SEED_LENGTH].copy_from_slice(&seed_array);
    sk[VRF_SEED_LENGTH..].copy_from_slice(&public);

    // Zeroize expanded secret explicitly
    expanded.scalar.zeroize();
    seed_array.zeroize();

    0
}

pub fn sk_to_pk(pk: &mut [u8], sk: &[u8]) {
    if pk.len() == VRF_PUBLIC_KEY_LENGTH && sk.len() == VRF_SECRET_KEY_LENGTH {
        pk.copy_from_slice(&sk[VRF_SEED_LENGTH..]);
    }
}

pub fn sk_to_seed(seed: &mut [u8], sk: &[u8]) {
    if seed.len() == VRF_SEED_LENGTH && sk.len() == VRF_SECRET_KEY_LENGTH {
        seed.copy_from_slice(&sk[..VRF_SEED_LENGTH]);
    }
}

pub fn prove(proof: &mut [u8], sk: &[u8], msg: &[u8]) -> i32 {
    if proof.len() != VRF_PROOF_LENGTH || sk.len() != VRF_SECRET_KEY_LENGTH {
        return BackendError::InvalidLengths.into();
    }

    let mut seed = [0u8; VRF_SEED_LENGTH];
    seed.copy_from_slice(&sk[..VRF_SEED_LENGTH]);
    let mut pk_bytes = [0u8; VRF_PUBLIC_KEY_LENGTH];
    pk_bytes.copy_from_slice(&sk[VRF_SEED_LENGTH..]);

    let mut expanded = expand_secret(&seed);
    let (h_point, h_bytes) = match hash_to_curve(&pk_bytes, msg) {
        Ok(value) => value,
        Err(err) => {
            expanded.zeroize();
            seed.zeroize();
            return err.into();
        }
    };

    let gamma_point = h_point * expanded.scalar;
    let gamma_bytes = gamma_point.compress().to_bytes();

    let mut nonce_state = Sha512::new();
    nonce_state.update(&expanded.nonce_prefix);
    nonce_state.update(&h_bytes);
    let nonce_digest = nonce_state.finalize();
    let mut nonce_bytes = [0u8; 64];
    nonce_bytes.copy_from_slice(&nonce_digest);
    let nonce_scalar = Scalar::from_bytes_mod_order_wide(&nonce_bytes);

    let k_b_bytes = (ED25519_BASEPOINT_POINT * nonce_scalar)
        .compress()
        .to_bytes();
    let k_h_bytes = (h_point * nonce_scalar).compress().to_bytes();

    let mut transcript = Sha512::new();
    transcript.update(&[SUITE]);
    transcript.update(&[TWO]);
    transcript.update(&pk_bytes);
    transcript.update(&h_bytes);
    transcript.update(&gamma_bytes);
    transcript.update(&k_b_bytes);
    transcript.update(&k_h_bytes);
    transcript.update(&[ZERO]);
    let challenge_digest = transcript.finalize();
    let mut challenge = [0u8; 64];
    challenge.copy_from_slice(&challenge_digest);

    let mut response_scalar_bytes = challenge;
    proof[..32].copy_from_slice(&gamma_bytes);
    proof[32..48].copy_from_slice(&challenge[..16]);
    response_scalar_bytes[16..].fill(0);
    let challenge_scalar = Scalar::from_bytes_mod_order_wide(&response_scalar_bytes);

    let response_scalar = challenge_scalar * expanded.scalar + nonce_scalar;
    proof[48..].copy_from_slice(&response_scalar.to_bytes());

    nonce_bytes.zeroize();
    response_scalar_bytes.zeroize();
    expanded.zeroize();
    seed.zeroize();

    0
}

pub fn verify(output: &mut [u8], pk: &[u8], proof: &[u8], msg: &[u8]) -> i32 {
    if output.len() != VRF_OUTPUT_LENGTH
        || pk.len() != VRF_PUBLIC_KEY_LENGTH
        || proof.len() != VRF_PROOF_LENGTH
    {
        return BackendError::InvalidLengths.into();
    }

    let pk_point = match decompress_point(pk) {
        Some(point) => point,
        None => return BackendError::InvalidKey.into(),
    };

    if pk_point.is_small_order() {
        return BackendError::InvalidKey.into();
    }

    let mut gamma_bytes = [0u8; 32];
    gamma_bytes.copy_from_slice(&proof[..32]);
    let gamma_point = match decompress_point(&gamma_bytes) {
        Some(point) => point,
        None => return BackendError::InvalidProof.into(),
    };
    if gamma_point.is_small_order() {
        return BackendError::InvalidProof.into();
    }

    let mut c_bytes = [0u8; 16];
    c_bytes.copy_from_slice(&proof[32..48]);
    let mut s_bytes = [0u8; 32];
    s_bytes.copy_from_slice(&proof[48..80]);
    let s_scalar = match Scalar::from_canonical_bytes(s_bytes).into() {
        Some(s) => s,
        None => return BackendError::InvalidProof.into(),
    };

    let mut pk_bytes = [0u8; VRF_PUBLIC_KEY_LENGTH];
    pk_bytes.copy_from_slice(pk);

    let (h_point, h_bytes) = match hash_to_curve(&pk_bytes, msg) {
        Ok(value) => value,
        Err(err) => return err.into(),
    };

    let mut c_full = [0u8; 64];
    c_full[..16].copy_from_slice(&c_bytes);
    let c_scalar = Scalar::from_bytes_mod_order_wide(&c_full);
    let neg_c_scalar = c_scalar.neg();

    let u_point = EdwardsPoint::vartime_multiscalar_mul(
        [s_scalar, neg_c_scalar],
        [&ED25519_BASEPOINT_POINT, &pk_point],
    );
    let v_point =
        EdwardsPoint::vartime_multiscalar_mul([s_scalar, neg_c_scalar], [&h_point, &gamma_point]);
    let u_bytes = u_point.compress().to_bytes();
    let v_bytes = v_point.compress().to_bytes();

    let mut transcript = Sha512::new();
    transcript.update(&[SUITE]);
    transcript.update(&[TWO]);
    transcript.update(pk);
    transcript.update(&h_bytes);
    transcript.update(&gamma_bytes);
    transcript.update(&u_bytes);
    transcript.update(&v_bytes);
    transcript.update(&[ZERO]);
    let expected_challenge = transcript.finalize();

    if expected_challenge[..16].ct_eq(&c_bytes).unwrap_u8() == 0 {
        return BackendError::InvalidProof.into();
    }

    proof_to_hash(output, proof)
}

pub fn proof_to_hash(hash: &mut [u8], proof: &[u8]) -> i32 {
    if hash.len() != VRF_OUTPUT_LENGTH || proof.len() != VRF_PROOF_LENGTH {
        return BackendError::InvalidLengths.into();
    }

    let mut gamma_bytes = [0u8; 32];
    gamma_bytes.copy_from_slice(&proof[..32]);
    let gamma_point = match decompress_point(&gamma_bytes) {
        Some(point) => point,
        None => return BackendError::InvalidProof.into(),
    };

    if proof[79] & 0xf0 != 0 {
        let mut s_bytes = [0u8; 32];
        s_bytes.copy_from_slice(&proof[48..80]);
        if Scalar::from_canonical_bytes(s_bytes).is_none().into() {
            return BackendError::InvalidProof.into();
        }
    }

    let gamma_cleared = gamma_point.mul_by_cofactor();
    let gamma_cleared_bytes = gamma_cleared.compress().to_bytes();

    let mut hasher = Sha512::new();
    hasher.update(&[SUITE]);
    hasher.update(&[THREE]);
    hasher.update(&gamma_cleared_bytes);
    hasher.update(&[ZERO]);
    let digest = hasher.finalize();
    hash.copy_from_slice(&digest);

    0
}

fn decompress_point(bytes: &[u8]) -> Option<EdwardsPoint> {
    if bytes.len() != VRF_PUBLIC_KEY_LENGTH {
        return None;
    }
    let compressed = CompressedEdwardsY::from_slice(bytes).ok()?;
    compressed.decompress()
}

struct ExpandedSecret {
    scalar: Scalar,
    nonce_prefix: [u8; 32],
}

impl ExpandedSecret {
    fn zeroize(&mut self) {
        self.nonce_prefix.zeroize();
    }
}

fn expand_secret(seed: &[u8; VRF_SEED_LENGTH]) -> ExpandedSecret {
    let digest = Sha512::digest(seed);
    let mut az = [0u8; 64];
    az.copy_from_slice(&digest);

    let mut scalar_bytes = [0u8; 32];
    scalar_bytes.copy_from_slice(&az[..32]);
    clamp_scalar(&mut scalar_bytes);
    let scalar = Scalar::from_bytes_mod_order(scalar_bytes);

    let mut prefix = [0u8; 32];
    prefix.copy_from_slice(&az[32..]);

    az.zeroize();
    ExpandedSecret {
        scalar,
        nonce_prefix: prefix,
    }
}

fn hash_to_curve(
    pk: &[u8; VRF_PUBLIC_KEY_LENGTH],
    msg: &[u8],
) -> Result<(EdwardsPoint, [u8; 32]), BackendError> {
    let mut input = Vec::with_capacity(VRF_PUBLIC_KEY_LENGTH + msg.len());
    input.extend_from_slice(pk);
    input.extend_from_slice(msg);

    let uniform = expand_message_xmd_sha512(&input, DST, 48)?;
    let mut uniform64 = [0u8; 64];
    for idx in 0..uniform.len() {
        uniform64[idx] = uniform[uniform.len() - 1 - idx];
    }

    map_uniform_to_curve(&uniform64)
}

fn expand_message_xmd_sha512(
    msg: &[u8],
    dst: &[u8],
    len_in_bytes: usize,
) -> Result<Vec<u8>, BackendError> {
    if len_in_bytes == 0 || len_in_bytes > 255 * HASH_SIZE_SHA512 {
        return Err(BackendError::InvalidLengths);
    }

    let mut dst_prime = if dst.len() > 255 {
        let mut hasher = Sha512::new();
        hasher.update(b"H2C-OVERSIZE-DST-");
        hasher.update(dst);
        hasher.finalize().to_vec()
    } else {
        dst.to_vec()
    };

    if dst_prime.len() > 255 {
        return Err(BackendError::InvalidLengths);
    }

    let mut t = [0u8; 3];
    t[1] = len_in_bytes as u8;
    let ctx_len_u8 = dst_prime.len() as u8;

    let mut hasher = Sha512::new();
    hasher.update(&[0u8; BLOCK_SIZE_SHA512]);
    hasher.update(msg);
    hasher.update(&t);
    hasher.update(&dst_prime);
    hasher.update(&[ctx_len_u8]);
    let u0 = hasher.finalize();

    let ell = len_in_bytes.div_ceil(HASH_SIZE_SHA512);
    let mut out = vec![0u8; len_in_bytes];
    let mut ux = [0u8; HASH_SIZE_SHA512];

    for i in 0..ell {
        for j in 0..HASH_SIZE_SHA512 {
            ux[j] ^= u0[j];
        }
        t[2] = t[2].wrapping_add(1);
        let mut loop_hasher = Sha512::new();
        loop_hasher.update(&ux);
        loop_hasher.update(&[t[2]]);
        loop_hasher.update(&dst_prime);
        loop_hasher.update(&[ctx_len_u8]);
        ux.copy_from_slice(&loop_hasher.finalize());

        let start = i * HASH_SIZE_SHA512;
        let end = ((i + 1) * HASH_SIZE_SHA512).min(len_in_bytes);
        out[start..end].copy_from_slice(&ux[..(end - start)]);
    }

    dst_prime.zeroize();
    Ok(out)
}

fn map_uniform_to_curve(uniform: &[u8; 64]) -> Result<(EdwardsPoint, [u8; 32]), BackendError> {
    let r = fe_reduce64(uniform);
    let a = curve25519_a();
    let sqrt_const = sqrt_minus_a_minus_two();

    let (x_mont, mut y_mont, notsquare) = elligator2_map(&r, &a)?;
    let y_sign = !notsquare;
    if bool::from(y_mont.is_negative()) ^ y_sign {
        y_mont = -&y_mont;
    }

    let (x_ed, y_ed) = montgomery_to_edwards(&x_mont, &y_mont, &sqrt_const);
    let compressed = compress_edwards_coordinates(&x_ed, &y_ed);

    let mut point = CompressedEdwardsY(compressed)
        .decompress()
        .ok_or(BackendError::HashToCurve)?;
    point = point.mul_by_cofactor();
    let compressed = point.compress().to_bytes();

    Ok((point, compressed))
}

fn elligator2_map(
    r: &FieldElement,
    a: &FieldElement,
) -> Result<(FieldElement, FieldElement, bool), BackendError> {
    let one = FieldElement::ONE;
    let mut rr2 = r.square();
    rr2 = &rr2 + &rr2;
    rr2 = &rr2 + &one;

    let rr2_inv = rr2.invert();

    let mut x = &rr2_inv * a;
    x = -&x;

    let gx1 = montgomery_polynomial(&x, a);
    let (is_square, mut y) = FieldElement::sqrt_ratio_i(&gx1, &FieldElement::ONE);
    let is_square_bool = bool::from(is_square);

    if !is_square_bool {
        let neg_x = -&x;
        x = &neg_x - a;
        let gx2 = montgomery_polynomial(&x, a);
        let (is_square2, y_candidate) = FieldElement::sqrt_ratio_i(&gx2, &FieldElement::ONE);
        if !bool::from(is_square2) {
            return Err(BackendError::HashToCurve);
        }
        y = y_candidate;
        Ok((x, y, true))
    } else {
        Ok((x, y, false))
    }
}

fn montgomery_polynomial(x: &FieldElement, a: &FieldElement) -> FieldElement {
    let x2 = x.square();
    let x3 = &x2 * x;
    let ax2 = &x2 * a;
    let mut result = &x3 + x;
    result = &result + &ax2;
    result
}

fn montgomery_to_edwards(
    x: &FieldElement,
    y: &FieldElement,
    sqrt_const: &FieldElement,
) -> (FieldElement, FieldElement) {
    let one = FieldElement::ONE;
    let x_plus_one = x + &one;
    let x_minus_one = x - &one;
    let denom = &x_plus_one * y;
    let denom_is_zero = bool::from(denom.is_zero());
    let denom_inv = denom.invert();

    let mut xed = *x;
    xed = &xed * sqrt_const;
    xed = &xed * &denom_inv;
    xed = &xed * &x_plus_one;

    let mut yed = &denom_inv * y;
    yed = &yed * &x_minus_one;
    if denom_is_zero {
        yed = FieldElement::ONE;
    }

    (xed, yed)
}

fn compress_edwards_coordinates(x: &FieldElement, y: &FieldElement) -> [u8; 32] {
    let mut bytes = y.as_bytes();
    let sign_bit = x.is_negative().unwrap_u8();
    bytes[31] ^= sign_bit << 7;
    bytes
}

fn fe_reduce64(input: &[u8; 64]) -> FieldElement {
    let mut fl = [0u8; 32];
    let mut gl = [0u8; 32];
    fl.copy_from_slice(&input[..32]);
    gl.copy_from_slice(&input[32..]);
    fl[31] &= 0x7f;
    gl[31] &= 0x7f;

    let mut fe_f = FieldElement::from_bytes(&fl);
    let fe_g = FieldElement::from_bytes(&gl);

    let correction =
        field_element_from_u64(u64::from(input[31] >> 7) * 19 + u64::from(input[63] >> 7) * 722);
    let factor_38 = field_element_from_u64(38);

    fe_f = &fe_f + &correction;
    let scaled = &fe_g * &factor_38;
    fe_f = &fe_f + &scaled;

    fe_f
}

fn field_element_from_u64(value: u64) -> FieldElement {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&value.to_le_bytes());
    FieldElement::from_bytes(&bytes)
}

fn curve25519_a() -> FieldElement {
    FieldElement::from_bytes(&CURVE25519_A_BYTES)
}

fn sqrt_minus_a_minus_two() -> FieldElement {
    FieldElement::from_bytes(&SQRT_MINUS_A_MINUS_TWO_BYTES)
}

fn clamp_scalar(bytes: &mut [u8; 32]) {
    bytes[0] &= 248;
    bytes[31] &= 127;
    bytes[31] |= 64;
}
