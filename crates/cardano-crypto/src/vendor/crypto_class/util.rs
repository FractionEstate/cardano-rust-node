use std::borrow::Cow;

use hex::{FromHex, FromHexError};
use num_bigint::BigUint;
use rand_core::RngCore;
use thiserror::Error;

/// Marker trait equivalent to the Haskell `Empty` class. Implemented for all types.
pub trait Empty {}

impl<T> Empty for T {}

/// Types that can be converted into a byte representation for signing.
pub trait SignableRepresentation {
    fn signable_representation(&self) -> Cow<'_, [u8]>;
}

impl SignableRepresentation for Vec<u8> {
    fn signable_representation(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(self.as_slice())
    }
}

impl SignableRepresentation for [u8] {
    fn signable_representation(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(self)
    }
}

impl SignableRepresentation for &'_ [u8] {
    fn signable_representation(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(self)
    }
}

impl SignableRepresentation for &'_ Vec<u8> {
    fn signable_representation(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(self.as_slice())
    }
}

impl<const N: usize> SignableRepresentation for [u8; N] {
    fn signable_representation(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(self.as_slice())
    }
}

/// Draw a random `u64` from the provided RNG.
pub fn get_random_word64<R: RngCore + ?Sized>(rng: &mut R) -> u64 {
    rng.next_u64()
}

/// Read an unsigned 64-bit integer from a big-endian byte slice.
#[must_use]
pub fn read_binary_word64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0u64, |acc, &b| (acc << 8) | u64::from(b))
}

/// Read a natural number (arbitrary precision) from a big-endian byte slice.
#[must_use]
pub fn read_binary_natural(bytes: &[u8]) -> BigUint {
    BigUint::from_bytes_be(bytes)
}

/// Serialise a `u64` into its big-endian representation.
#[must_use]
pub fn write_binary_word64(value: u64) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

/// Serialise a natural number into exactly `len` bytes (big-endian), truncating
/// higher-order bytes if necessary.
#[must_use]
pub fn write_binary_natural(len: usize, value: &BigUint) -> Vec<u8> {
    if len == 0 {
        return Vec::new();
    }

    let mut le = value.to_bytes_le();
    if le.len() < len {
        le.resize(len, 0);
    } else if le.len() > len {
        le.truncate(len);
    }

    le.reverse();
    le
}

/// Split a byte slice at the specified lengths.
#[must_use]
pub fn splits_at<'a>(lengths: &[usize], bytes: &'a [u8]) -> Vec<Cow<'a, [u8]>> {
    let mut result = Vec::with_capacity(lengths.len().saturating_add(1));
    let mut remainder = bytes;

    for &len in lengths {
        if remainder.len() < len {
            return Vec::new();
        }
        let (head, tail) = remainder.split_at(len);
        result.push(Cow::Borrowed(head));
        remainder = tail;
    }

    if !remainder.is_empty() {
        result.push(Cow::Borrowed(remainder));
    }

    result
}

/// Slice helper taking `offset` and `size` as `u64`s.
#[must_use]
pub fn slice(offset: u64, size: u64, bytes: &[u8]) -> Cow<'_, [u8]> {
    let start = offset.min(bytes.len() as u64) as usize;
    let available = (bytes.len() as u64).saturating_sub(start as u64);
    let length = size.min(available) as usize;
    Cow::Borrowed(&bytes[start..start + length])
}

/// Convert bytes to a natural number (big-endian).
#[must_use]
pub fn bytes_to_natural(bytes: &[u8]) -> BigUint {
    BigUint::from_bytes_be(bytes)
}

/// Convert a natural number to bytes (big-endian) of the specified length.
#[must_use]
pub fn natural_to_bytes(len: usize, value: &BigUint) -> Vec<u8> {
    write_binary_natural(len, value)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecodeHexError {
    #[error("malformed hex: {0}")]
    Malformed(String),
    #[error("expected {expected} bytes, got {actual}")]
    LengthMismatch { expected: usize, actual: usize },
    #[error("input string contains invalid ASCII characters: {0}")]
    InvalidCharacters(String),
}

impl From<FromHexError> for DecodeHexError {
    fn from(err: FromHexError) -> Self {
        DecodeHexError::Malformed(err.to_string())
    }
}

/// Decode hexadecimal bytes ensuring the decoded len matches expectations.
///
/// # Errors
///
/// Returns an error if the bytes are not valid hexadecimal or if the decoded
/// length differs from `expected_len`.
pub fn decode_hex_byte_string(
    bytes: &[u8],
    expected_len: usize,
) -> Result<Vec<u8>, DecodeHexError> {
    let decoded = Vec::from_hex(bytes).map_err(DecodeHexError::from)?;
    if decoded.len() != expected_len {
        return Err(DecodeHexError::LengthMismatch {
            expected: expected_len,
            actual: decoded.len(),
        });
    }
    Ok(decoded)
}

/// Decode hexadecimal string with optional `0x` prefix ensuring expected length.
///
/// # Errors
///
/// Returns an error if the string contains non-ASCII characters, is not valid
/// hexadecimal, or if the decoded length differs from `expected_len`.
pub fn decode_hex_string(input: &str, expected_len: usize) -> Result<Vec<u8>, DecodeHexError> {
    let trimmed = input.strip_prefix("0x").unwrap_or(input);
    if !trimmed.is_ascii() {
        return Err(DecodeHexError::InvalidCharacters(trimmed.to_owned()));
    }
    decode_hex_byte_string(trimmed.as_bytes(), expected_len)
}

/// Convenience macro mirroring Template Haskell `decodeHexStringQ`.
#[macro_export]
macro_rules! decode_hex_string_or_panic {
    ($hex:expr, $len:expr) => {{
        match $crate::vendor::crypto_class::util::decode_hex_string($hex, $len) {
            Ok(bytes) => bytes,
            Err(err) => panic!("<decode_hex_string>: {err}"),
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use rand_core::RngCore;

    #[test]
    fn random_word64_matches_rng() {
        let mut rng = StdRng::seed_from_u64(42);
        let expected = rng.next_u64();
        let mut rng2 = StdRng::seed_from_u64(42);
        assert_eq!(get_random_word64(&mut rng2), expected);
    }

    #[test]
    fn binary_word64_roundtrip() {
        let value = 0x0102030405060708u64;
        let encoded = write_binary_word64(value);
        assert_eq!(encoded, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(read_binary_word64(&encoded), value);
    }

    #[test]
    fn binary_natural_roundtrip() {
        let value = BigUint::from(0x0102030405u64);
        let bytes = write_binary_natural(5, &value);
        assert_eq!(bytes, vec![1, 2, 3, 4, 5]);
        assert_eq!(read_binary_natural(&bytes), value);
    }

    #[test]
    fn binary_natural_truncates() {
        let value = BigUint::from(0x0102030405u64);
        let bytes = write_binary_natural(2, &value);
        assert_eq!(bytes, vec![4, 5]);
    }

    #[test]
    fn splits_at_exact() {
        let bytes = b"abcdefgh";
        let parts = splits_at(&[2, 3, 3], bytes);
        assert_eq!(
            parts.iter().map(|c| c.as_ref()).collect::<Vec<_>>(),
            vec![&b"ab"[..], &b"cde"[..], &b"fgh"[..]]
        );
    }

    #[test]
    fn splits_at_short_returns_original() {
        let bytes = b"data";
        let parts = splits_at(&[2, 5], bytes);
        assert!(parts.is_empty());
    }

    #[test]
    fn splits_at_with_remainder() {
        let bytes = b"abcdefgh";
        let parts = splits_at(&[2, 2], bytes);
        assert_eq!(
            parts.iter().map(|c| c.as_ref()).collect::<Vec<_>>(),
            vec![&b"ab"[..], &b"cd"[..], &b"efgh"[..]]
        );
    }

    #[test]
    fn slice_within_bounds() {
        let bytes = b"helloworld";
        let slice_bytes = slice(2, 4, bytes);
        assert_eq!(slice_bytes.as_ref(), b"llow");
    }

    #[test]
    fn slice_truncates_at_end() {
        let bytes = b"abc";
        let slice_bytes = slice(1, 10, bytes);
        assert_eq!(slice_bytes.as_ref(), b"bc");
    }

    #[test]
    fn decode_hex_success() {
        let result = decode_hex_string("0x0102", 2).unwrap();
        assert_eq!(result, vec![1, 2]);
    }

    #[test]
    fn decode_hex_length_error() {
        let err = decode_hex_string("0x0102", 1).unwrap_err();
        match err {
            DecodeHexError::LengthMismatch { expected, actual } => {
                assert_eq!(expected, 1);
                assert_eq!(actual, 2);
            }
            _ => panic!("unexpected error: {err}"),
        }
    }

    #[test]
    fn decode_hex_invalid_ascii() {
        let err = decode_hex_string("µ", 1).unwrap_err();
        assert!(matches!(err, DecodeHexError::InvalidCharacters(_)));
    }

    #[test]
    fn macro_panics_on_error() {
        let result = std::panic::catch_unwind(|| {
            let _ = crate::decode_hex_string_or_panic!("0x01", 2);
        });
        assert!(result.is_err());
    }
}
