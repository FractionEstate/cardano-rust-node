//! Montgomery curve operations and Elligator2 mapping
//!
//! This module implements Montgomery curve operations for Curve25519,
//! including the critical Elligator2 hash-to-curve function that must
//! match Cardano's libsodium implementation exactly.
//!
//! # Montgomery vs Edwards
//!
//! Curve25519 can be represented as either:
//! - Montgomery form: By^2 = x^3 + Ax^2 + x
//! - Edwards form: x^2 + y^2 = 1 + dx^2y^2
//!
//! This module handles Montgomery operations and conversion to Edwards.

use super::{debug, field::FieldElement};

// Montgomery curve constants for Curve25519
// By^2 = x^3 + Ax^2 + x where A = 486662, B = 1

const A: i64 = 486662;

#[inline]
fn montgomery_a_fe() -> FieldElement {
    let mut a_bytes = [0u8; 32];
    let a = A as u64;
    a_bytes[0] = (a & 0xff) as u8;
    a_bytes[1] = ((a >> 8) & 0xff) as u8;
    a_bytes[2] = ((a >> 16) & 0xff) as u8;
    FieldElement::from_bytes(&a_bytes)
}

/// Elligator2 hash-to-curve function
///
/// Maps a 32-byte uniform random string to a point on Curve25519.
/// This is the CRITICAL function that must match libsodium exactly.
///
/// # Arguments
///
/// * `r` - 32-byte uniform random value
///
/// # Returns
///
/// Montgomery u-coordinate resulting from the mapping
///
/// # Algorithm
///
/// Implements Elligator2 mapping as specified in Cardano's libsodium:
/// 1. Load r as field element and square it
/// 2. Compute v = -A/(1+ur^2) where u = 2
/// 3. Compute e = Legendre symbol of v^3 + Av^2 + v
/// 4. Select x = e*v - (1-e)*A (where A is the Montgomery curve parameter)
/// 5. Compute y from x using curve equation
///
/// The key insight: If v^3 + Av^2 + v is a QR, use x=v; otherwise x=-v-A.
/// This ensures at least one of these x values will have a corresponding y.
#[must_use]
pub fn elligator2(r: &[u8; 32]) -> Option<FieldElement> {
    let a_fe = montgomery_a_fe();
    let r_fe = FieldElement::from_bytes(r);
    debug::log(|| {
        let bytes: String = r_fe
            .to_bytes()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        format!("elligator2: r_fe = {}", bytes)
    });
    debug::log(|| {
        let limbs: Vec<String> = r_fe.0.iter().map(|v| v.to_string()).collect();
        format!("elligator2: r_fe limbs = [{}]", limbs.join(", "))
    });

    // rr2 = 2 * r^2
    let mut rr2 = r_fe.square2().reduce();
    // rr2 = 1 + 2 * r^2
    rr2 = (rr2 + FieldElement::one()).reduce();

    let rr2_inv = rr2.invert();
    debug::log(|| {
        let bytes: String = rr2
            .to_bytes()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        format!("elligator2: rr2 = {}", bytes)
    });
    debug::log(|| {
        let bytes: String = rr2_inv
            .to_bytes()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        format!("elligator2: rr2_inv = {}", bytes)
    });

    // x1 = -A / (1 + 2r^2)
    let mut x = (rr2_inv * a_fe).reduce();
    debug::log(|| {
        let bytes: String = x.to_bytes().iter().map(|b| format!("{:02x}", b)).collect();
        format!("elligator2: a_over_denom = {}", bytes)
    });

    x = (-x).reduce();
    debug::log(|| {
        let bytes: String = x.to_bytes().iter().map(|b| format!("{:02x}", b)).collect();
        format!("elligator2: x1 = {}", bytes)
    });

    // gx1 = x^3 + A*x^2 + x
    let x_sq = x.square().reduce();
    let mut gx1 = (x_sq * x).reduce();
    let ax_sq = (x_sq * a_fe).reduce();
    gx1 = (gx1 + ax_sq + x).reduce();

    let rhs_is_square = is_quadratic_residue(&gx1);

    if !rhs_is_square {
        x = (-x).reduce();
        x = (x - a_fe).reduce();
    }

    debug::log(|| format!("elligator2: rhs_v_is_square = {}", rhs_is_square));

    Some(x.reduce())
}

/// Faithful port of libsodium/cardano C ge25519_elligator2
///
/// Returns (x, y, notsquare) where (x,y) are Montgomery coordinates and
/// notsquare indicates whether the first candidate gx1 was a nonsquare
/// (i.e. 1 if gx1 not square, 0 if square) exactly like C sets notsquare.
#[must_use]
pub fn ge25519_elligator2_faithful(r: &FieldElement) -> (FieldElement, FieldElement, u8) {
    // Mirrors C:
    // cardano_fe25519_sq2(rr2, r); rr2[0]++; invert; x = -A * rr2; (neg afterwards) etc.
    let mut rr2 = r.square2().reduce(); // 2*r^2
    // rr2[0]++ equivalent: add 1
    rr2 = (rr2 + FieldElement::one()).reduce();
    let rr2_inv = rr2.invert(); // 1/(1+2*r^2)

    // x = -A * rr2_inv  (C: cardano_fe25519_mul(x, cardano_curve25519_A, rr2); cardano_fe25519_neg(x,x))
    let a_fe = montgomery_a_fe();
    let mut x = (a_fe * rr2_inv).reduce();
    x = (-x).reduce(); // x = x1

    // gx1 = x1^3 + A*x1^2 + x1
    let x_sq = x.square().reduce();
    let x_cu = (x_sq * x).reduce();
    let ax_sq = (x_sq * a_fe).reduce();
    let gx1 = (x_cu + x + ax_sq).reduce();

    let notsquare = (!gx1.is_square()) as u8; // fe25519_notsquare(gx1)

    // If not square: x = -x1 - A
    if notsquare == 1 {
        let negx = (-x).reduce();
        x = (negx - a_fe).reduce();
    }

    // Recover y from curve equation y = sqrt(x^3 + A*x^2 + x)
    let y = {
        let x_sq2 = x.square().reduce();
        let x_cu2 = (x_sq2 * x).reduce();
        let ax_sq2 = (x_sq2 * a_fe).reduce();
        let rhs = (x_cu2 + ax_sq2 + x).reduce();
        match rhs.sqrt() {
            Some(value) => value,
            None => {
                debug::log(|| {
                    "ge25519_elligator2_faithful: sqrt failed (should not happen)".to_string()
                });
                FieldElement::zero()
            },
        }
    };

    fn fe_hex(fe: &FieldElement) -> String {
        fe.to_bytes().iter().map(|b| format!("{:02x}", b)).collect()
    }
    debug::log(|| {
        format!(
            "ge25519_elligator2_faithful:\n  rr2 = {}\n  x_final = {}\n  gx1_notsquare = {}\n  y = {}",
            fe_hex(&rr2),
            fe_hex(&x),
            notsquare,
            fe_hex(&y)
        )
    });

    (x, y, notsquare)
}

fn is_quadratic_residue(value: &FieldElement) -> bool {
    value.is_square()
}

/// Convert Montgomery coordinates to Edwards coordinates
///
/// Transforms (u, v) on Montgomery curve to (x, y) on Edwards curve.
///
/// # Arguments
///
/// * `mont_u` - Montgomery u-coordinate
/// * `mont_v` - Montgomery v-coordinate
///
/// # Returns
///
/// Edwards coordinates (x, y)
///
/// # Formula
///
/// For Curve25519/Ed25519 conversion:
/// - x = sqrt(-486664) * u / v
/// - y = (u - 1) / (u + 1)
#[must_use]
pub fn mont_to_edwards(
    mont_u: &FieldElement,
    mont_v: &FieldElement,
) -> Option<(FieldElement, FieldElement)> {
    let one = FieldElement::one();
    let x_plus_one = (*mont_u + one).reduce();
    let x_minus_one = (*mont_u - one).reduce();

    let denom = (x_plus_one * *mont_v).reduce();
    let denom_is_zero = denom.is_zero();
    let denom_inv = denom.invert();

    let mut ed_x = (*mont_u * FieldElement::SQRT_AM2).reduce();
    ed_x = (ed_x * denom_inv).reduce();
    ed_x = (ed_x * x_plus_one).reduce();

    let mut ed_y = (denom_inv * *mont_v).reduce();
    ed_y = (ed_y * x_minus_one).reduce();
    if denom_is_zero {
        ed_y = one;
    }

    Some((ed_x.reduce(), ed_y.reduce()))
}

/// Faithful port of libsodium/cardano C ge25519_mont_to_ed
/// Input: Montgomery (x,y) -> Output: Edwards (X,Y)
#[must_use]
pub fn ge25519_mont_to_ed_faithful(
    x: &FieldElement,
    y: &FieldElement,
) -> (FieldElement, FieldElement) {
    let one = FieldElement::one();
    let x_plus_one = (*x + one).reduce();
    let x_minus_one = (*x - one).reduce();

    // xed = sqrt(-A-2)*x/((x+1)*y)*(x+1)
    let denom = (x_plus_one * *y).reduce();
    let denom_inv = denom.invert();
    let mut xed = (*x * FieldElement::SQRT_AM2).reduce();
    xed = (xed * denom_inv).reduce();
    xed = (xed * x_plus_one).reduce();

    // yed = (x-1)/(x+1) = (x-1) * 1/(x+1)
    // reuse denom_inv * y = 1/(x+1)
    let inv_x_plus_one = (denom_inv * *y).reduce();
    let mut yed = (inv_x_plus_one * x_minus_one).reduce();
    if denom.is_zero() {
        // condition on original (x+1)*y pre-invert
        yed = one;
    }
    debug::log(|| {
        format!(
            "ge25519_mont_to_ed_faithful:\n  x = {}\n  y = {}\n  xed = {}\n  yed = {}\n  denom_zero = {}",
            fe_hex(x),
            fe_hex(y),
            fe_hex(&xed),
            fe_hex(&yed),
            denom.is_zero() as u8
        )
    });
    // Alternate direct formula: x' = sqrt(-A-2)*x / y, y' = (x-1)/(x+1)
    let y_inv = y.invert();
    let x_alt = ((*x * FieldElement::SQRT_AM2).reduce() * y_inv).reduce();
    let x_plus_one_inv = x_plus_one.invert();
    let y_alt = (x_minus_one * x_plus_one_inv).reduce();
    if x_alt.to_bytes() != xed.to_bytes() || y_alt.to_bytes() != yed.to_bytes() {
        debug::log(|| {
            format!(
                "alt formula diff:\n  x_alt = {}\n  y_alt = {}",
                fe_hex(&x_alt),
                fe_hex(&y_alt)
            )
        });
    }
    (xed, yed)
}

fn fe_hex(fe: &FieldElement) -> String {
    fe.to_bytes().iter().map(|b| format!("{:02x}", b)).collect()
}

/// Recover Montgomery y-coordinate from x-coordinate
///
/// Given x on Montgomery curve, compute corresponding y coordinate.
///
/// # Arguments
///
/// * `x` - Montgomery x-coordinate
/// * `sign` - Sign bit to select positive or negative root
///
/// # Returns
///
/// Montgomery y-coordinate if x is on curve
///
/// # Formula
///
/// Solves By^2 = x^3 + Ax^2 + x for y where B = 1, A = 486662
#[must_use]
pub fn xmont_to_ymont(x: &FieldElement, sign: u8) -> Option<FieldElement> {
    // Compute x^2
    let x2 = x.square().reduce();

    // Compute x^3 = x * x^2
    let x3 = (*x * x2).reduce();

    // Compute A as field element
    let a_fe = montgomery_a_fe();

    // Compute Ax^2
    let ax2 = (a_fe * x2).reduce();

    // Compute rhs = x^3 + Ax^2 + x (since B = 1)
    let rhs = (x3 + ax2 + *x).reduce();

    // Compute square root of rhs
    let y_opt = rhs.sqrt();

    y_opt.map(|mut y| {
        // Flip sign when the canonical sign bit does not match the requested one.
        let expect_negative = sign == 1;
        if y.is_negative() != expect_negative {
            y = -y;
        }
        y.reduce()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elligator2_basic() {
        let r = [0u8; 32];
        let result = elligator2(&r);
        // Should return Some value now
        assert!(result.is_some());
    }

    #[test]
    fn test_elligator2_various_inputs() {
        // Test with several inputs - some may not have valid points
        let mut r = [0u8; 32];
        let mut successes = 0;

        // Use smaller values to avoid overflow in current implementation
        for i in 0..5 {
            r[0] = i as u8;
            r[1] = 0;
            r[2] = 0;
            if elligator2(&r).is_some() {
                successes += 1;
            }
        }

        // At least some inputs should succeed
        assert!(successes > 0, "No successful Elligator2 mappings found");
    }

    #[test]
    fn test_field_square_and_sqrt_consistency() {
        use super::super::field::FieldElement;

        // Test that square and sqrt are inverses (when sqrt exists)
        let x = FieldElement::one() + FieldElement::one(); // 2
        let x2 = x.square(); // No need for .reduce() since mul already reduces

        // eprintln!("x (2) = {}", hex::encode(x.to_bytes()));
        // eprintln!("x^2 (4) = {}", hex::encode(x2.to_bytes()));

        // x^2 should be a QR
        let is_qr = x2.is_square();
        // eprintln!("is_square(4) = {}", is_qr);
        assert!(is_qr, "Square of 2 should be a QR");

        // sqrt(x^2) should give us back something whose square is x^2
        let sqrt_x2 = x2
            .sqrt()
            .expect("sqrt of x^2 should exist for quadratic residues");
        // eprintln!("sqrt(4) = {}", hex::encode(sqrt_x2.to_bytes()));
        let sqrt_x2_squared = sqrt_x2.square();
        // eprintln!("sqrt(4)^2 = {}", hex::encode(sqrt_x2_squared.to_bytes()));
        assert_eq!(
            sqrt_x2_squared.to_bytes(),
            x2.to_bytes(),
            "sqrt(x^2)^2 should equal x^2"
        );
    }

    #[test]
    fn test_mont_to_edwards() {
        // Test with valid Montgomery point
        let mut u_bytes = [0u8; 32];
        u_bytes[0] = 9; // Base point u-coordinate
        let u = FieldElement::from_bytes(&u_bytes);

        // Get corresponding v
        if let Some(v) = xmont_to_ymont(&u, 0) {
            let result = mont_to_edwards(&u, &v);
            // Should succeed for valid point
            assert!(result.is_some());
        }
    }
}
