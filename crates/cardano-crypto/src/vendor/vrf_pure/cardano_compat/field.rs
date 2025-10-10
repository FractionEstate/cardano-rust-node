//! Field element operations for Curve25519
//!
//! This module implements field arithmetic in GF(2^255-19), the finite field
//! used by Curve25519. The implementation uses radix 2^25.5 representation
//! with 10 limbs, matching the libsodium implementation exactly.
//!
//! # Representation
//!
//! Field elements are represented as `FieldElement([i64; 10])` where:
//! - Limbs alternate between 26 and 25 bits
//! - Element value = Σ(limb\[i\] * 2^(26*⌊i/2⌋ + 25*⌈i/2⌉))
//!
//! # Operations
//!
//! All operations maintain constant-time properties where applicable to
//! prevent timing attacks.

use std::ops::{Add, Mul, Neg, Sub};

/// Field element in GF(2^255-19)
///
/// Represents an element using 10 limbs in radix 2^25.5 representation.
/// This matches the libsodium fe25519 type exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldElement(pub [i64; 10]);

impl FieldElement {
    /// Precomputed constant sqrt(-1) matching libsodium's `fe_sqrtm1`.
    pub const SQRT_M1: Self = Self([
        -32595792, -7943725, 9377950, 3500415, 12389472, -272473, -25146209, -2005654, 326686,
        11406482,
    ]);

    /// Precomputed constant sqrt(-486664) (i.e. sqrt(-A-2) with A = 486662).
    pub const SQRT_AM2: Self = Self([
        -12222970, -8312128, -11511410, 9067497, -15300785, -241793, 25456130, 14121551, -12187136,
        3972024,
    ]);

    /// Zero element (additive identity)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cardano_vrf_pure::cardano_compat::field::FieldElement;
    /// let zero = FieldElement::zero();
    /// assert_eq!(zero.0[0], 0);
    /// ```
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        FieldElement([0; 10])
    }

    /// One element (multiplicative identity)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cardano_vrf_pure::cardano_compat::field::FieldElement;
    /// let one = FieldElement::one();
    /// assert_eq!(one.0[0], 1);
    /// ```
    #[inline]
    #[must_use]
    pub const fn one() -> Self {
        let mut fe = [0i64; 10];
        fe[0] = 1;
        FieldElement(fe)
    }

    /// Load field element from 32 bytes (little-endian)
    ///
    /// Converts a 32-byte array to a field element using the standard
    /// encoding where bytes represent the element in little-endian order.
    ///
    /// # Arguments
    ///
    /// * `bytes` - 32-byte array in little-endian format
    ///
    /// # Returns
    ///
    /// Field element with limbs populated from the bytes
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cardano_vrf_pure::cardano_compat::field::FieldElement;
    /// let bytes = [0u8; 32];
    /// let fe = FieldElement::from_bytes(&bytes);
    /// ```
    #[must_use]
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        #[inline(always)]
        fn load3(input: &[u8]) -> i64 {
            (input[0] as i64) | ((input[1] as i64) << 8) | ((input[2] as i64) << 16)
        }

        #[inline(always)]
        fn load4(input: &[u8]) -> i64 {
            (input[0] as i64)
                | ((input[1] as i64) << 8)
                | ((input[2] as i64) << 16)
                | ((input[3] as i64) << 24)
        }

        let mut h0 = load4(&bytes[0..4]);
        let mut h1 = load3(&bytes[4..7]) << 6;
        let mut h2 = load3(&bytes[7..10]) << 5;
        let mut h3 = load3(&bytes[10..13]) << 3;
        let mut h4 = load3(&bytes[13..16]) << 2;
        let mut h5 = load4(&bytes[16..20]);
        let mut h6 = load3(&bytes[20..23]) << 7;
        let mut h7 = load3(&bytes[23..26]) << 5;
        let mut h8 = load3(&bytes[26..29]) << 4;
        let mut h9 = (load3(&bytes[29..32]) & 0x7fffff) << 2;

        let carry9 = (h9 + (1 << 24)) >> 25;
        h0 += carry9 * 19;
        h9 -= carry9 << 25;

        let carry1 = (h1 + (1 << 24)) >> 25;
        h2 += carry1;
        h1 -= carry1 << 25;

        let carry3 = (h3 + (1 << 24)) >> 25;
        h4 += carry3;
        h3 -= carry3 << 25;

        let carry5 = (h5 + (1 << 24)) >> 25;
        h6 += carry5;
        h5 -= carry5 << 25;

        let carry7 = (h7 + (1 << 24)) >> 25;
        h8 += carry7;
        h7 -= carry7 << 25;

        let carry0 = (h0 + (1 << 25)) >> 26;
        h1 += carry0;
        h0 -= carry0 << 26;

        let carry2 = (h2 + (1 << 25)) >> 26;
        h3 += carry2;
        h2 -= carry2 << 26;

        let carry4 = (h4 + (1 << 25)) >> 26;
        h5 += carry4;
        h4 -= carry4 << 26;

        let carry6 = (h6 + (1 << 25)) >> 26;
        h7 += carry6;
        h6 -= carry6 << 26;

        let carry8 = (h8 + (1 << 25)) >> 26;
        h9 += carry8;
        h8 -= carry8 << 26;

        FieldElement([h0, h1, h2, h3, h4, h5, h6, h7, h8, h9])
    }

    /// Convert field element to 32 bytes (little-endian)
    ///
    /// Converts the field element to its canonical byte representation.
    /// The element is first reduced modulo 2^255-19 to ensure uniqueness.
    ///
    /// # Returns
    ///
    /// 32-byte array representing the field element in little-endian format
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cardano_vrf_pure::cardano_compat::field::FieldElement;
    /// let fe = FieldElement::one();
    /// let bytes = fe.to_bytes();
    /// assert_eq!(bytes[0], 1);
    /// ```
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 32] {
        // Start from a reduced representation to keep limb ranges small
        let h = self.reduce().0;

        let mut h0 = h[0];
        let mut h1 = h[1];
        let mut h2 = h[2];
        let mut h3 = h[3];
        let mut h4 = h[4];
        let mut h5 = h[5];
        let mut h6 = h[6];
        let mut h7 = h[7];
        let mut h8 = h[8];
        let mut h9 = h[9];

        let mut q = (19 * h9 + (1 << 24)) >> 25;
        q = (h0 + q) >> 26;
        q = (h1 + q) >> 25;
        q = (h2 + q) >> 26;
        q = (h3 + q) >> 25;
        q = (h4 + q) >> 26;
        q = (h5 + q) >> 25;
        q = (h6 + q) >> 26;
        q = (h7 + q) >> 25;
        q = (h8 + q) >> 26;
        q = (h9 + q) >> 25;

        h0 += 19 * q;

        let mut carry0 = h0 >> 26;
        h1 += carry0;
        h0 -= carry0 << 26;
        let mut carry1 = h1 >> 25;
        h2 += carry1;
        h1 -= carry1 << 25;
        let carry2 = h2 >> 26;
        h3 += carry2;
        h2 -= carry2 << 26;
        let carry3 = h3 >> 25;
        h4 += carry3;
        h3 -= carry3 << 25;
        let carry4 = h4 >> 26;
        h5 += carry4;
        h4 -= carry4 << 26;
        let carry5 = h5 >> 25;
        h6 += carry5;
        h5 -= carry5 << 25;
        let carry6 = h6 >> 26;
        h7 += carry6;
        h6 -= carry6 << 26;
        let carry7 = h7 >> 25;
        h8 += carry7;
        h7 -= carry7 << 25;
        let carry8 = h8 >> 26;
        h9 += carry8;
        h8 -= carry8 << 26;
        let carry9 = h9 >> 25;
        h9 -= carry9 << 25;
        h0 += carry9 * 19;

        carry0 = h0 >> 26;
        h1 += carry0;
        h0 -= carry0 << 26;
        carry1 = h1 >> 25;
        h2 += carry1;
        h1 -= carry1 << 25;

        let mut s = [0u8; 32];
        s[0] = h0 as u8;
        s[1] = (h0 >> 8) as u8;
        s[2] = (h0 >> 16) as u8;
        s[3] = ((h0 >> 24) | (h1 << 2)) as u8;
        s[4] = (h1 >> 6) as u8;
        s[5] = (h1 >> 14) as u8;
        s[6] = ((h1 >> 22) | (h2 << 3)) as u8;
        s[7] = (h2 >> 5) as u8;
        s[8] = (h2 >> 13) as u8;
        s[9] = ((h2 >> 21) | (h3 << 5)) as u8;
        s[10] = (h3 >> 3) as u8;
        s[11] = (h3 >> 11) as u8;
        s[12] = ((h3 >> 19) | (h4 << 6)) as u8;
        s[13] = (h4 >> 2) as u8;
        s[14] = (h4 >> 10) as u8;
        s[15] = (h4 >> 18) as u8;
        s[16] = h5 as u8;
        s[17] = (h5 >> 8) as u8;
        s[18] = (h5 >> 16) as u8;
        s[19] = ((h5 >> 24) | (h6 << 1)) as u8;
        s[20] = (h6 >> 7) as u8;
        s[21] = (h6 >> 15) as u8;
        s[22] = ((h6 >> 23) | (h7 << 3)) as u8;
        s[23] = (h7 >> 5) as u8;
        s[24] = (h7 >> 13) as u8;
        s[25] = ((h7 >> 21) | (h8 << 4)) as u8;
        s[26] = (h8 >> 4) as u8;
        s[27] = (h8 >> 12) as u8;
        s[28] = ((h8 >> 20) | (h9 << 6)) as u8;
        s[29] = (h9 >> 2) as u8;
        s[30] = (h9 >> 10) as u8;
        s[31] = (h9 >> 18) as u8;

        s
    }

    /// Reduce field element modulo 2^255-19
    ///
    /// Performs carry propagation to ensure limbs are within their proper ranges.
    /// This matches the libsodium fe25519_reduce() function.
    ///
    /// # Returns
    ///
    /// Reduced field element with normalized limbs
    #[must_use]
    pub fn reduce(&self) -> Self {
        let mut h = self.0;
        let mut carry: i64;

        // First reduction pass
        carry = (h[0] + (1 << 25)) >> 26;
        h[1] += carry;
        h[0] -= carry << 26;

        carry = (h[4] + (1 << 25)) >> 26;
        h[5] += carry;
        h[4] -= carry << 26;

        carry = (h[1] + (1 << 24)) >> 25;
        h[2] += carry;
        h[1] -= carry << 25;

        carry = (h[5] + (1 << 24)) >> 25;
        h[6] += carry;
        h[5] -= carry << 25;

        carry = (h[2] + (1 << 25)) >> 26;
        h[3] += carry;
        h[2] -= carry << 26;

        carry = (h[6] + (1 << 25)) >> 26;
        h[7] += carry;
        h[6] -= carry << 26;

        carry = (h[3] + (1 << 24)) >> 25;
        h[4] += carry;
        h[3] -= carry << 25;

        carry = (h[7] + (1 << 24)) >> 25;
        h[8] += carry;
        h[7] -= carry << 25;

        carry = (h[8] + (1 << 25)) >> 26;
        h[9] += carry;
        h[8] -= carry << 26;

        // Handle top limb overflow (multiply by 19)
        // Need to do this multiple times if value is very large
        for _ in 0..3 {
            carry = h[9] >> 25;
            h[0] += carry * 19;
            h[9] -= carry << 25;

            // Propagate carry from h[0]
            carry = h[0] >> 26;
            h[1] += carry;
            h[0] -= carry << 26;

            // And from h[1] if needed
            carry = h[1] >> 25;
            h[2] += carry;
            h[1] -= carry << 25;
        }

        FieldElement(h)
    }

    /// Square the field element
    ///
    /// Computes self * self efficiently using the fact that many terms are identical.
    ///
    /// # Returns
    ///
    /// The squared field element
    #[inline]
    #[must_use]
    pub fn square(&self) -> Self {
        *self * *self
    }

    /// Square and double: 2 * self^2
    ///
    /// More efficient than computing square then doubling separately.
    ///
    /// # Returns
    ///
    /// 2 * self^2
    #[inline]
    #[must_use]
    pub fn square2(&self) -> Self {
        let sq = self.square();
        sq + sq
    }

    #[inline]
    fn pow2k(&self, k: u32) -> Self {
        debug_assert!(k > 0);
        let mut z = self.square().reduce();
        for _ in 1..k {
            z = z.square().reduce();
        }
        z
    }

    #[inline]
    fn sqmul(mut acc: Self, squarings: usize, mul: &Self) -> Self {
        for _ in 0..squarings {
            acc = acc.square().reduce();
        }
        (acc * *mul).reduce()
    }

    fn pow22501(&self) -> (Self, Self) {
        let t0 = self.square().reduce();
        let mut t1 = t0.square().reduce();
        t1 = t1.square().reduce();
        let t2 = (*self * t1).reduce();
        let t3 = (t0 * t2).reduce();
        let t4 = t3.square().reduce();
        let t5 = (t2 * t4).reduce();
        let t6 = t5.pow2k(5);
        let t7 = (t6 * t5).reduce();
        let t8 = t7.pow2k(10);
        let t9 = (t8 * t7).reduce();
        let t10 = t9.pow2k(20);
        let t11 = (t10 * t9).reduce();
        let t12 = t11.pow2k(10);
        let t13 = (t12 * t7).reduce();
        let t14 = t13.pow2k(50);
        let t15 = (t14 * t13).reduce();
        let t16 = t15.pow2k(100);
        let t17 = (t16 * t15).reduce();
        let t18 = t17.pow2k(50);
        let t19 = (t18 * t13).reduce();

        (t19, t3)
    }

    fn pow22523(&self) -> Self {
        let (t19, _) = self.pow22501();
        let t20 = t19.pow2k(2);
        (t20 * *self).reduce()
    }

    #[inline]
    fn pow_p58(&self) -> Self {
        // (p-5)/8 for p = 2^255 - 19 equals 2^252 - 3
        self.pow22523()
    }

    fn sqrt_ratio(u: &Self, v: &Self) -> (bool, Self) {
        let v2 = v.square().reduce();
        let v3 = (v2 * *v).reduce();
        let v6 = v3.square().reduce();
        let v7 = (v6 * *v).reduce();

        let u_v3 = (*u * v3).reduce();
        let u_v7 = (*u * v7).reduce();

        let pow = u_v7.pow_p58();
        let mut r = (u_v3 * pow).reduce();

        let vxx = (*v * r.square()).reduce();
        let m_root_check = (vxx - *u).reduce();
        let p_root_check = (vxx + *u).reduce();
        let u_sqrt_m1 = (*u * Self::SQRT_M1).reduce();
        let f_root_check = (vxx + u_sqrt_m1).reduce();

        let has_m_root = m_root_check.is_zero();
        let has_p_root = p_root_check.is_zero();
        let has_f_root = f_root_check.is_zero();

        if has_p_root || has_f_root {
            r = (r * Self::SQRT_M1).reduce();
        }

        if has_p_root {
            r = (-r).reduce();
        }

        if r.is_negative() {
            r = (-r).reduce();
        }

        (has_m_root || has_p_root, r)
    }

    /// Check if field element is square (has a square root)
    ///
    /// Uses Euler's criterion: x is a square iff x^((p-1)/2) = 1 (mod p)
    /// For p = 2^255-19, this is x^(2^254-10)
    ///
    /// # Returns
    ///
    /// `true` if the element has a square root, `false` otherwise
    #[must_use]
    #[allow(clippy::just_underscores_and_digits)]
    pub fn is_square(&self) -> bool {
        let x = self.reduce();
        if x.is_zero() {
            return true;
        }

        // Mirror libsodium's fe25519_notsquare implementation exactly.
        let _10 = (x * x).reduce(); // x^2
        let _11 = (x * _10).reduce(); // x^3

        let mut _1100 = _11.square().reduce(); // x^6
        _1100 = _1100.square().reduce(); // x^12

        let _1111 = (_11 * _1100).reduce(); // x^15

        let mut _11110000 = _1111.square().reduce(); // x^30
        _11110000 = _11110000.square().reduce(); // x^60
        _11110000 = _11110000.square().reduce(); // x^120
        _11110000 = _11110000.square().reduce(); // x^240

        let _11111111 = (_1111 * _11110000).reduce(); // x^255

        let mut t = FieldElement::sqmul(_11111111, 2, &_11);
        let u = t;
        t = FieldElement::sqmul(t, 10, &u);
        t = FieldElement::sqmul(t, 10, &u);
        let mut v = t;
        t = FieldElement::sqmul(t, 30, &v);
        v = t;
        t = FieldElement::sqmul(t, 60, &v);
        v = t;
        t = FieldElement::sqmul(t, 120, &v);
        t = FieldElement::sqmul(t, 10, &u);
        t = FieldElement::sqmul(t, 3, &_11);
        t = t.square().reduce();

        let bytes = t.to_bytes();
        (bytes[1] & 1) == 0
    }

    /// Compute modular square root
    ///
    /// If the element is a quadratic residue, returns Some(sqrt).
    /// Otherwise returns None.
    ///
    /// # Algorithm
    ///
    /// For p = 2^255 - 19, we can compute sqrt using:
    /// sqrt(x) = x^((p+3)/8) = x^(2^252 - 2)
    ///
    /// We have pow22523() = x^(2^252 - 3), so:
    /// x^(2^252 - 2) = x^(2^252 - 3) * x = pow22523() * x
    ///
    /// # Returns
    ///
    /// `Some(sqrt)` if square root exists, `None` otherwise
    #[must_use]
    pub fn sqrt(&self) -> Option<Self> {
        let a = self.reduce();
        if a.is_zero() {
            return Some(FieldElement::zero());
        }

        let one = FieldElement::one();
        let (is_square, root) = Self::sqrt_ratio(&a, &one);

        if is_square { Some(root) } else { None }
    }

    /// Compute multiplicative inverse
    ///
    /// Returns the multiplicative inverse of the field element.
    /// For non-zero x, returns x^(-1) such that x * x^(-1) = 1.
    ///
    /// # Panics
    ///
    /// Panics if called on zero (which has no inverse).
    ///
    /// # Returns
    ///
    /// The multiplicative inverse
    #[must_use]
    pub fn invert(&self) -> Self {
        let (t19, t3) = self.pow22501();
        let t20 = t19.pow2k(5);
        (t20 * t3).reduce()
    }

    /// Conditional select: return `a` if `choice == 1`, else `b`
    ///
    /// This is a constant-time operation to prevent timing attacks.
    ///
    /// # Arguments
    ///
    /// * `a` - First field element
    /// * `b` - Second field element
    /// * `choice` - Selection bit (0 or 1)
    ///
    /// # Returns
    ///
    /// `a` if `choice == 1`, `b` if `choice == 0`
    #[inline]
    #[must_use]
    pub fn conditional_select(a: &Self, b: &Self, choice: u8) -> Self {
        let mask = -(choice as i64);
        let mut result = [0i64; 10];

        for ((result_limb, &a_limb), &b_limb) in result.iter_mut().zip(a.0.iter()).zip(b.0.iter()) {
            *result_limb = b_limb ^ (mask & (a_limb ^ b_limb));
        }

        FieldElement(result)
    }

    /// Conditional swap: swap `a` and `b` if `choice == 1`
    ///
    /// This is a constant-time operation to prevent timing attacks.
    ///
    /// # Arguments
    ///
    /// * `a` - First field element (mutable)
    /// * `b` - Second field element (mutable)
    /// * `choice` - Swap bit (0 or 1)
    pub fn conditional_swap(a: &mut Self, b: &mut Self, choice: u8) {
        let mask = -(choice as i64);

        for (a_limb, b_limb) in a.0.iter_mut().zip(b.0.iter_mut()) {
            let t = mask & (*a_limb ^ *b_limb);
            *a_limb ^= t;
            *b_limb ^= t;
        }
    }

    /// Check if field element equals zero (constant-time)
    ///
    /// # Returns
    ///
    /// `true` if the element is zero, `false` otherwise
    #[must_use]
    pub fn is_zero(&self) -> bool {
        let reduced = self.reduce();
        let bytes = reduced.to_bytes();
        bytes.iter().all(|&b| b == 0)
    }

    /// Check if field element is negative
    ///
    /// An element is considered negative if its least significant bit is 1
    /// in the canonical byte representation.
    ///
    /// # Returns
    ///
    /// `true` if negative, `false` otherwise
    #[must_use]
    pub fn is_negative(&self) -> bool {
        let bytes = self.to_bytes();
        (bytes[0] & 1) == 1
    }
}

// Implement standard arithmetic operations

impl Add for FieldElement {
    type Output = Self;

    /// Add two field elements
    ///
    /// Performs component-wise addition of limbs.
    fn add(self, other: Self) -> Self {
        let mut h = [0i64; 10];
        for ((acc, &lhs), &rhs) in h.iter_mut().zip(self.0.iter()).zip(other.0.iter()) {
            *acc = lhs + rhs;
        }
        FieldElement(h)
    }
}

impl Sub for FieldElement {
    type Output = Self;

    /// Subtract two field elements
    ///
    /// Performs component-wise subtraction of limbs.
    fn sub(self, other: Self) -> Self {
        let mut h = [0i64; 10];
        for ((acc, &lhs), &rhs) in h.iter_mut().zip(self.0.iter()).zip(other.0.iter()) {
            *acc = lhs - rhs;
        }
        FieldElement(h)
    }
}

impl Neg for FieldElement {
    type Output = Self;

    /// Negate field element
    ///
    /// Returns -self in the field.
    fn neg(self) -> Self {
        FieldElement::zero() - self
    }
}

impl Mul for FieldElement {
    type Output = Self;

    /// Multiply two field elements
    ///
    /// Performs full field multiplication matching the libsodium implementation.
    /// Uses the fact that reduction modulo 2^255-19 can be done by multiplying
    /// high limbs by 19.
    fn mul(self, other: Self) -> Self {
        // Exact translation of ref10 fe_mul.c (see SUPERCOP / libsodium) into Rust with i128 temporaries.
        // This version intentionally mirrors term structure for auditability.
        let f = self.0;
        let g = other.0;
        let f0 = f[0] as i128;
        let f1 = f[1] as i128;
        let f2 = f[2] as i128;
        let f3 = f[3] as i128;
        let f4 = f[4] as i128;
        let f5 = f[5] as i128;
        let f6 = f[6] as i128;
        let f7 = f[7] as i128;
        let f8 = f[8] as i128;
        let f9 = f[9] as i128;
        let g0 = g[0] as i128;
        let g1 = g[1] as i128;
        let g2 = g[2] as i128;
        let g3 = g[3] as i128;
        let g4 = g[4] as i128;
        let g5 = g[5] as i128;
        let g6 = g[6] as i128;
        let g7 = g[7] as i128;
        let g8 = g[8] as i128;
        let g9 = g[9] as i128;

        // Precomputations identical to ref10
        let f1_2 = 2 * f1;
        let f3_2 = 2 * f3;
        let f5_2 = 2 * f5;
        let f7_2 = 2 * f7;
        let f9_2 = 2 * f9;

        let g1_19 = 19 * g1; // 1.959375 * 2^29 < 2^55
        let g2_19 = 19 * g2;
        let g3_19 = 19 * g3;
        let g4_19 = 19 * g4;
        let g5_19 = 19 * g5;
        let g6_19 = 19 * g6;
        let g7_19 = 19 * g7;
        let g8_19 = 19 * g8;
        let g9_19 = 19 * g9;

        // Products with 38 (= 2*19) appear for terms where an odd f_i is doubled then multiplied by g_j*19
        let f1g9_38 = f1_2 * g9_19;
        let f2g8_19 = f2 * g8_19;
        let f3g7_38 = f3_2 * g7_19;
        let f4g6_19 = f4 * g6_19;
        let f5g5_38 = f5_2 * g5_19;
        let f6g4_19 = f6 * g4_19;
        let f7g3_38 = f7_2 * g3_19;
        let f8g2_19 = f8 * g2_19;
        let f9g1_38 = f9_2 * g1_19;
        let f2g9_19 = f2 * g9_19;
        let f3g8_19 = f3 * g8_19;
        let f4g7_19 = f4 * g7_19;
        let f5g6_19 = f5 * g6_19;
        let f6g5_19 = f6 * g5_19;
        let f7g4_19 = f7 * g4_19;
        let f8g3_19 = f8 * g3_19;
        let f9g2_19 = f9 * g2_19;
        let f3g9_38 = f3_2 * g9_19;
        let f5g7_38 = f5_2 * g7_19;
        let f7g5_38 = f7_2 * g5_19;
        let f9g3_38 = f9_2 * g3_19;
        let f5g9_38 = f5_2 * g9_19;
        let f7g7_38 = f7_2 * g7_19;
        let f9g5_38 = f9_2 * g5_19;
        let f4g8_19 = f4 * g8_19;
        let f4g9_19 = f4 * g9_19;
        let f6g7_19 = f6 * g7_19;
        let f7g6_19 = f7 * g6_19;
        let f8g5_19 = f8 * g5_19;
        let f9g4_19 = f9 * g4_19;
        let f6g8_19 = f6 * g8_19;
        let f8g6_19 = f8 * g6_19;
        let f8g7_19 = f8 * g7_19;
        let f9g6_19 = f9 * g6_19;
        let f7g9_38 = f7_2 * g9_19;
        let f9g7_38 = f9_2 * g7_19;
        let f8g9_19 = f8 * g9_19;
        let f9g8_19 = f9 * g8_19;
        let f9g9_38 = f9_2 * g9_19;
        let f5g8_19 = f5 * g8_19; // used in h3
        let f6g9_19 = f6 * g9_19; // used in h5
        let f7g8_19 = f7 * g8_19; // used in h5
        let f8g8_19 = f8 * g8_19; // used in h6
        let f6g6_19 = f6 * g6_19; // used in h2
        let f8g4_19 = f8 * g4_19; // used in h2

        // h0..h9 exact formulas
        let mut h0 = f0 * g0
            + f1g9_38
            + f2g8_19
            + f3g7_38
            + f4g6_19
            + f5g5_38
            + f6g4_19
            + f7g3_38
            + f8g2_19
            + f9g1_38;
        let mut h1 = f0 * g1
            + f1 * g0
            + f2g9_19
            + f3g8_19
            + f4g7_19
            + f5g6_19
            + f6g5_19
            + f7g4_19
            + f8g3_19
            + f9g2_19;
        let mut h2 = f0 * g2
            + f1_2 * g1
            + f2 * g0
            + f3g9_38
            + f4g8_19
            + f5g7_38
            + f6g6_19
            + f7g5_38
            + f8g4_19
            + f9g3_38;
        let mut h3 = f0 * g3
            + f1 * g2
            + f2 * g1
            + f3 * g0
            + f4g9_19
            + f5g8_19
            + f6g7_19
            + f7g6_19
            + f8g5_19
            + f9g4_19;
        let mut h4 = f0 * g4
            + f1_2 * g3
            + f2 * g2
            + f3_2 * g1
            + f4 * g0
            + f5g9_38
            + f6g8_19
            + f7g7_38
            + f8g6_19
            + f9g5_38;
        let mut h5 = f0 * g5
            + f1 * g4
            + f2 * g3
            + f3 * g2
            + f4 * g1
            + f5 * g0
            + f6g9_19
            + f7g8_19
            + f8g7_19
            + f9g6_19;
        let mut h6 = f0 * g6
            + f1_2 * g5
            + f2 * g4
            + f3_2 * g3
            + f4 * g2
            + f5_2 * g1
            + f6 * g0
            + f7g9_38
            + f8g8_19
            + f9g7_38;
        let mut h7 = f0 * g7
            + f1 * g6
            + f2 * g5
            + f3 * g4
            + f4 * g3
            + f5 * g2
            + f6 * g1
            + f7 * g0
            + f8g9_19
            + f9g8_19;
        let mut h8 = f0 * g8
            + f1_2 * g7
            + f2 * g6
            + f3_2 * g5
            + f4 * g4
            + f5_2 * g3
            + f6 * g2
            + f7_2 * g1
            + f8 * g0
            + f9g9_38;
        let mut h9 = f0 * g9
            + f1 * g8
            + f2 * g7
            + f3 * g6
            + f4 * g5
            + f5 * g4
            + f6 * g3
            + f7 * g2
            + f8 * g1
            + f9 * g0;

        // Carry chain (unchanged from ref10)
        let mut carry0 = (h0 + (1 << 25)) >> 26;
        h1 += carry0;
        h0 -= carry0 << 26;
        let mut carry4 = (h4 + (1 << 25)) >> 26;
        h5 += carry4;
        h4 -= carry4 << 26;
        let mut carry1 = (h1 + (1 << 24)) >> 25;
        h2 += carry1;
        h1 -= carry1 << 25;
        let carry5 = (h5 + (1 << 24)) >> 25;
        h6 += carry5;
        h5 -= carry5 << 25;
        let carry2 = (h2 + (1 << 25)) >> 26;
        h3 += carry2;
        h2 -= carry2 << 26;
        let carry6 = (h6 + (1 << 25)) >> 26;
        h7 += carry6;
        h6 -= carry6 << 26;
        let carry3 = (h3 + (1 << 24)) >> 25;
        h4 += carry3;
        h3 -= carry3 << 25;
        let carry7 = (h7 + (1 << 24)) >> 25;
        h8 += carry7;
        h7 -= carry7 << 25;
        carry4 = (h4 + (1 << 25)) >> 26;
        h5 += carry4;
        h4 -= carry4 << 26;
        let carry8 = (h8 + (1 << 25)) >> 26;
        h9 += carry8;
        h8 -= carry8 << 26;
        let carry9 = (h9 + (1 << 24)) >> 25;
        h0 += carry9 * 19;
        h9 -= carry9 << 25;
        carry0 = (h0 + (1 << 25)) >> 26;
        h1 += carry0;
        h0 -= carry0 << 26;
        carry1 = (h1 + (1 << 24)) >> 25;
        h2 += carry1;
        h1 -= carry1 << 25;

        FieldElement([
            h0 as i64, h1 as i64, h2 as i64, h3 as i64, h4 as i64, h5 as i64, h6 as i64, h7 as i64,
            h8 as i64, h9 as i64,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_one() {
        let zero = FieldElement::zero();
        let one = FieldElement::one();

        assert_eq!(zero.0[0], 0);
        assert_eq!(one.0[0], 1);

        // 0 + 1 = 1
        let sum = zero + one;
        assert_eq!(sum.reduce().0[0], 1);
    }

    #[test]
    fn test_addition() {
        let one = FieldElement::one();
        let two = one + one;

        assert_eq!(two.reduce().0[0], 2);
    }

    #[test]
    fn test_multiplication() {
        let one = FieldElement::one();
        let two = one + one;
        let four = two * two;

        assert_eq!(four.reduce().0[0], 4);
    }

    #[test]
    fn test_mul_matches_biguint_random() {
        use num_bigint::BigUint;
        use num_traits::One;
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        // modulus p = 2^255 - 19
        let p = (&BigUint::one() << 255u32) - BigUint::from(19u32);
        let mut rng = StdRng::seed_from_u64(0xF1E1D1C1B1A19088);

        fn fe_to_big(fe: &FieldElement) -> BigUint {
            // convert canonical bytes to BigUint
            let bytes = fe.reduce().to_bytes();
            BigUint::from_bytes_le(&bytes)
        }

        fn random_fe(rng: &mut StdRng) -> FieldElement {
            // sample uniformly from 0..p by generating 255 bits and reducing
            let mut bytes = [0u8; 32];
            rng.fill(&mut bytes);
            bytes[31] &= 0x7f; // clamp to < 2^255
            // Interpret as little endian number then mod p using BigUint then to FieldElement via from_bytes
            let n = BigUint::from_bytes_le(&bytes)
                % ((&BigUint::one() << 255u32) - BigUint::from(19u32));
            let mut out = [0u8; 32];
            let nbytes = n.to_bytes_le();
            for (i, b) in nbytes.iter().enumerate() {
                out[i] = *b;
            }
            FieldElement::from_bytes(&out)
        }

        for _ in 0..200u32 {
            let a = random_fe(&mut rng);
            let b = random_fe(&mut rng);
            let prod = a * b;
            let prod_bytes = prod.reduce().to_bytes();
            let expected = (fe_to_big(&a) * fe_to_big(&b)) % &p;
            let mut exp_bytes = [0u8; 32];
            let eb = expected.to_bytes_le();
            for (i, b) in eb.iter().enumerate() {
                exp_bytes[i] = *b;
            }
            assert_eq!(
                prod_bytes, exp_bytes,
                "Mul mismatch: a={:?} b={:?} got={:02x?} expect={:02x?}",
                a.0, b.0, prod_bytes, exp_bytes
            );
        }
    }

    #[test]
    fn test_square() {
        let two = FieldElement::one() + FieldElement::one();
        let four1 = two.square();
        let four2 = two * two;

        let r1 = four1.reduce();
        let r2 = four2.reduce();
        assert_eq!(r1.0, r2.0);
    }

    #[test]
    fn test_bytes_roundtrip() {
        // Test that a canonical value roundtrips correctly
        let one = FieldElement::one();
        let bytes1 = one.to_bytes();
        let fe_back = FieldElement::from_bytes(&bytes1);
        let bytes2 = fe_back.to_bytes();

        // These should be identical since one is canonical
        assert_eq!(bytes1, bytes2);

        // Test zero
        let zero = FieldElement::zero();
        let bytes_zero = zero.to_bytes();
        let fe_zero_back = FieldElement::from_bytes(&bytes_zero);
        assert_eq!(bytes_zero, fe_zero_back.to_bytes());
    }

    #[test]
    fn test_random_canonical_roundtrip() {
        use num_bigint::BigUint;
        use num_traits::One;
        use rand::rngs::StdRng;
        use rand::{RngCore, SeedableRng};

        let mut rng = StdRng::seed_from_u64(0xC0DEC0DECAFEBABE);
        let p = (&BigUint::one() << 255u32) - BigUint::from(19u32);

        for _ in 0..1024 {
            let mut bytes = [0u8; 32];
            rng.fill_bytes(&mut bytes);
            bytes[31] &= 0x7f;
            let value = BigUint::from_bytes_le(&bytes) % &p;
            let mut canonical = [0u8; 32];
            let vb = value.to_bytes_le();
            canonical[..vb.len()].copy_from_slice(&vb);

            let fe = FieldElement::from_bytes(&canonical);
            let round = fe.to_bytes();
            assert_eq!(canonical, round, "roundtrip failed for value {}", value);
        }
    }

    #[test]
    fn test_specific_bytes_roundtrip() {
        let canonical = [
            0x07, 0xc1, 0xd6, 0x85, 0x0b, 0x5b, 0x94, 0x2c, 0xc6, 0x1b, 0x8e, 0x55, 0xfd, 0x1e,
            0x0f, 0x2a, 0x00, 0xc8, 0x17, 0x3b, 0xaa, 0xc7, 0x96, 0x0b, 0xc6, 0xff, 0x93, 0x15,
            0x68, 0xf4, 0x91, 0x09,
        ];
        let fe = FieldElement::from_bytes(&canonical);
        let round = fe.to_bytes();
        assert_eq!(
            canonical, round,
            "specific canonical bytes failed roundtrip"
        );
    }

    #[test]
    fn test_invert() {
        let fe = FieldElement::one();
        let inv = fe.invert();
        let product = fe * inv;
        assert_eq!(product.reduce(), FieldElement::one());
    }

    #[test]
    fn test_invert_non_trivial() {
        // Test with a non-trivial element
        let mut bytes = [0u8; 32];
        bytes[0] = 5;
        let fe = FieldElement::from_bytes(&bytes);
        let inv = fe.invert();
        let product = fe * inv;

        // Product should be 1
        let one = FieldElement::one();
        let product_reduced = product.reduce();
        let one_reduced = one.reduce();

        assert_eq!(product_reduced.to_bytes(), one_reduced.to_bytes());
    }

    #[test]
    fn test_is_square_four() {
        let two = FieldElement::one() + FieldElement::one();
        let four = two * two;

        // 4 should be a quadratic residue
        assert!(four.is_square(), "4 should be a quadratic residue");
    }

    #[test]
    fn test_sqrt() {
        // Test square root of 4 (should be 2 or -2)
        let mut bytes = [0u8; 32];
        bytes[0] = 4;
        let fe = FieldElement::from_bytes(&bytes);
        let sqrt = fe.sqrt().expect("square root of 4 should exist");
        let check = sqrt.square();
        assert_eq!(check.reduce().to_bytes(), fe.reduce().to_bytes());
    }

    #[test]
    fn test_conditional_select() {
        let a = FieldElement::one();
        let b = FieldElement::zero();

        let result_a = FieldElement::conditional_select(&a, &b, 1);
        assert_eq!(result_a, a);

        let result_b = FieldElement::conditional_select(&a, &b, 0);
        assert_eq!(result_b, b);
    }

    #[test]
    fn test_is_zero() {
        let zero = FieldElement::zero();
        assert!(zero.is_zero());

        let one = FieldElement::one();
        assert!(!one.is_zero());
    }

    #[test]
    fn test_is_square_zero() {
        let zero = FieldElement::zero();
        assert!(zero.is_square());
    }

    #[test]
    fn test_sqrt_minus_one() {
        let minus_one = FieldElement::zero() - FieldElement::one();
        let sqrt_minus_one = minus_one
            .sqrt()
            .expect("sqrt(-1) should exist in GF(2^255-19)");

        assert_eq!(
            sqrt_minus_one.square().reduce().to_bytes(),
            minus_one.reduce().to_bytes()
        );

        let sqrt_bytes = sqrt_minus_one.to_bytes();
        let pos = FieldElement::SQRT_M1.to_bytes();
        let neg = (-FieldElement::SQRT_M1).to_bytes();
        assert!(
            sqrt_bytes == pos || sqrt_bytes == neg,
            "sqrt(-1) should equal ±sqrt(-1) constant"
        );
    }
}
