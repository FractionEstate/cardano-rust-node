use core::cmp::Ordering;
use core::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Fixed-size packed byte array with efficient equality and XOR support.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct PackedBytes<const N: usize> {
    data: [u8; N],
}

impl<const N: usize> PackedBytes<N> {
    /// Construct packed bytes from an array.
    #[must_use]
    pub const fn new(data: [u8; N]) -> Self {
        Self { data }
    }

    /// View the packed bytes as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Mutable view of the packed bytes.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Copy the underlying bytes into a fixed array.
    #[must_use]
    pub fn to_array(&self) -> [u8; N] {
        self.data
    }

    /// Copy the underlying bytes into a `Vec`.
    #[must_use]
    pub fn to_vec(&self) -> Vec<u8> {
        self.data.to_vec()
    }
}

impl<const N: usize> fmt::Debug for PackedBytes<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PackedBytes({}, 0x", N)?;
        for byte in &self.data {
            write!(f, "{:02x}", byte)?;
        }
        write!(f, ")")
    }
}

impl<const N: usize> Ord for PackedBytes<N> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.data.cmp(&other.data)
    }
}

impl<const N: usize> PartialOrd for PackedBytes<N> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const N: usize> AsRef<[u8]> for PackedBytes<N> {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl<const N: usize> From<[u8; N]> for PackedBytes<N> {
    fn from(data: [u8; N]) -> Self {
        Self::new(data)
    }
}

impl<const N: usize> From<PackedBytes<N>> for [u8; N] {
    fn from(bytes: PackedBytes<N>) -> Self {
        bytes.data
    }
}

#[cfg(feature = "serde")]
impl<const N: usize> Serialize for PackedBytes<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self.data)
    }
}

#[cfg(feature = "serde")]
impl<'de, const N: usize> Deserialize<'de> for PackedBytes<N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor<const N: usize>;

        impl<'de, const N: usize> serde::de::Visitor<'de> for Visitor<N> {
            type Value = PackedBytes<N>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a byte array of length {N}")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                pack_pinned_bytes(v).map_err(|err| E::custom(err.to_string()))
            }

            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_bytes(&v)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut data = [0u8; N];
                let mut idx = 0usize;

                while let Some(byte) = seq.next_element::<u8>()? {
                    if idx >= N {
                        return Err(<A::Error as serde::de::Error>::invalid_length(
                            idx + 1,
                            &self,
                        ));
                    }
                    data[idx] = byte;
                    idx += 1;
                }

                if idx != N {
                    return Err(<A::Error as serde::de::Error>::invalid_length(idx, &self));
                }

                Ok(PackedBytes::new(data))
            }
        }

        deserializer.deserialize_bytes(Visitor::<N>)
    }
}

/// Pack bytes from a slice and offset, panicking if bounds are violated.
///
/// # Panics
///
/// Panics when `bytes` does not contain `N` bytes starting at `offset`.
#[must_use]
pub fn pack_bytes<const N: usize>(bytes: &[u8], offset: usize) -> PackedBytes<N> {
    pack_bytes_maybe(bytes, offset).expect("pack_bytes: slice too short")
}

/// Pack bytes from a slice and offset, returning `None` if the slice is too small.
#[must_use]
pub fn pack_bytes_maybe<const N: usize>(bytes: &[u8], offset: usize) -> Option<PackedBytes<N>> {
    bytes.get(offset..offset.checked_add(N)?).map(|segment| {
        let mut data = [0u8; N];
        data.copy_from_slice(segment);
        PackedBytes::new(data)
    })
}

/// Pack bytes from a slice ensuring the slice length matches the packed length.
///
/// # Errors
///
/// Returns an error if `bytes.len()` does not equal `N`.
pub fn pack_pinned_bytes<const N: usize>(bytes: &[u8]) -> Result<PackedBytes<N>, PackedBytesError> {
    if bytes.len() != N {
        return Err(PackedBytesError::LengthMismatch {
            expected: N,
            actual: bytes.len(),
        });
    }
    let mut data = [0u8; N];
    data.copy_from_slice(bytes);
    Ok(PackedBytes::new(data))
}

/// Unpack into a fixed-size array.
#[must_use]
pub fn unpack_bytes<const N: usize>(bytes: &PackedBytes<N>) -> [u8; N] {
    bytes.to_array()
}

/// Unpack into a `Vec<u8>`.
#[must_use]
pub fn unpack_pinned_bytes<const N: usize>(bytes: &PackedBytes<N>) -> Vec<u8> {
    bytes.to_vec()
}

/// XOR two packed byte arrays element-wise.
#[must_use]
pub fn xor_packed_bytes<const N: usize>(
    lhs: &PackedBytes<N>,
    rhs: &PackedBytes<N>,
) -> PackedBytes<N> {
    let mut data = [0u8; N];
    for ((dst, a), b) in data.iter_mut().zip(lhs.as_slice()).zip(rhs.as_slice()) {
        *dst = a ^ b;
    }
    PackedBytes::new(data)
}

/// Errors that can occur when packing bytes.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PackedBytesError {
    #[error("length mismatch: expected {expected}, got {actual}")]
    LengthMismatch { expected: usize, actual: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_and_unpack_roundtrip() {
        let source = b"01234567";
        let packed = pack_bytes::<8>(source, 0);
        assert_eq!(unpack_bytes(&packed), *source);
    }

    #[test]
    fn pack_bytes_maybe_respects_offset() {
        let source = b"abcdefghijklmnopqrstuvwxyz";
        let packed = pack_bytes::<5>(source, 1);
        assert_eq!(packed.as_slice(), b"bcdef");
    }

    #[test]
    fn xor_matches_manual() {
        let a = pack_bytes::<8>(&[0xff; 8], 0);
        let b = pack_bytes::<8>(&[0x0f; 8], 0);
        let xor = xor_packed_bytes(&a, &b);
        assert_eq!(xor.as_slice(), &[0xf0; 8]);
    }

    #[test]
    fn pack_bytes_maybe_fails_out_of_bounds() {
        let bytes = b"abc";
        assert!(pack_bytes_maybe::<4>(bytes, 0).is_none());
    }

    #[test]
    fn pack_pinned_bytes_validates_length() {
        let bytes = vec![1, 2, 3];
        let err = pack_pinned_bytes::<2>(&bytes).unwrap_err();
        assert_eq!(
            err,
            PackedBytesError::LengthMismatch {
                expected: 2,
                actual: 3
            }
        );
    }

    #[test]
    fn ordering_is_lexicographic() {
        let a = pack_bytes::<3>(b"abc", 0);
        let b = pack_bytes::<3>(b"abd", 0);
        assert!(a < b);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn serde_roundtrip() {
        let packed = pack_bytes::<4>(b"test", 0);
        let json = serde_json::to_string(&packed).unwrap();
        let back: PackedBytes<4> = serde_json::from_str(&json).unwrap();
        assert_eq!(packed, back);
    }
}
