use std::fmt::{self, Formatter};
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

use subtle::{Choice, ConstantTimeEq};
use thiserror::Error;

use crate::vendor::crypto_class::ffi::{SizedMutPtr, SizedPtr};
use crate::vendor::crypto_class::util::{DecodeHexError, decode_hex_string};

/// Error raised when constructing a [`PinnedSizedBytes`] from an input with an
/// unexpected length.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PinnedSizedBytesError {
    #[error("expected {expected} bytes, got {actual}")]
    SizeMismatch { expected: usize, actual: usize },
    #[error("hex decode error: {0}")]
    Hex(#[from] DecodeHexError),
}

/// Heap allocated, pinned byte array whose size is tracked by the type system.
///
/// This mirrors the semantics of the Haskell `PinnedSizedBytes` type backed by
/// a pinned `ByteArray`. The pointer exposed through the helper methods remains
/// valid for the duration of the borrow, but must not be stored.
#[derive(Clone)]
pub struct PinnedSizedBytes<const N: usize> {
    data: Box<[u8; N]>,
}

impl<const N: usize> PinnedSizedBytes<N> {
    /// Construct from an owned byte array of the exact size.
    #[must_use]
    pub fn from_array(array: [u8; N]) -> Self {
        Self {
            data: Box::new(array),
        }
    }

    /// Deprecated Haskell helper preserved for parity: pad or truncate the
    /// provided slice to fit the fixed size.
    #[must_use]
    pub fn from_bytes_padded(bytes: &[u8]) -> Self {
        let mut array = [0u8; N];
        if bytes.len() >= N {
            array.copy_from_slice(&bytes[bytes.len() - N..]);
        } else {
            array[N - bytes.len()..].copy_from_slice(bytes);
        }
        Self::from_array(array)
    }

    /// Attempt to construct from a slice, ensuring the length matches `N`.
    ///
    /// # Errors
    ///
    /// Returns an error if `slice.len()` differs from `N`.
    pub fn from_slice(slice: &[u8]) -> Result<Self, PinnedSizedBytesError> {
        if slice.len() != N {
            return Err(PinnedSizedBytesError::SizeMismatch {
                expected: N,
                actual: slice.len(),
            });
        }
        let mut array = [0u8; N];
        array.copy_from_slice(slice);
        Ok(Self::from_array(array))
    }

    /// Decode a hexadecimal string into the pinned buffer, matching Haskell's
    /// Template Haskell helper.
    ///
    /// # Errors
    ///
    /// Returns an error if the string cannot be decoded or does not yield `N`
    /// bytes.
    pub fn from_hex(hex: &str) -> Result<Self, PinnedSizedBytesError> {
        let bytes = decode_hex_string(hex, N)?;
        Self::from_slice(&bytes)
    }

    /// Borrow the underlying bytes as a slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; N] {
        &self.data
    }

    /// Borrow the underlying bytes mutably.
    pub fn as_mut_bytes(&mut self) -> &mut [u8; N] {
        &mut self.data
    }

    /// Return the bytes as a `Vec<u8>`.
    #[must_use]
    pub fn to_vec(&self) -> Vec<u8> {
        self.data.to_vec()
    }

    /// Equivalent to `psbToBytes`, returning an owned byte vector.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.to_vec()
    }

    /// Run `f` with a raw pointer to the byte buffer.
    pub fn with_c_ptr<R>(&self, f: impl FnOnce(*const u8) -> R) -> R {
        f(self.data.as_ptr())
    }

    /// Run `f` with a raw pointer and length.
    pub fn with_c_ptr_len<R>(&self, f: impl FnOnce(*const u8, usize) -> R) -> R {
        f(self.data.as_ptr(), N)
    }

    /// Run `f` with a sized pointer wrapper mirroring the Haskell API.
    pub fn with_sized_ptr<R>(&self, f: impl FnOnce(SizedPtr<'_, N>) -> R) -> R {
        let ptr = NonNull::from(&*self.data).cast::<u8>();
        f(SizedPtr::new(ptr))
    }

    /// Allocate a new pinned buffer, execute `f` with a mutable pointer, and
    /// return the initialised bytes alongside the function's result.
    pub fn create_result<R>(mut f: impl FnMut(*mut u8) -> R) -> (Self, R) {
        let mut data = Box::new([0u8; N]);
        let result = f(data.as_mut_ptr());
        (Self { data }, result)
    }

    /// Variant of [\`create_result\`] that also passes the buffer length.
    pub fn create_result_len<R>(mut f: impl FnMut(*mut u8, usize) -> R) -> (Self, R) {
        let mut data = Box::new([0u8; N]);
        let result = f(data.as_mut_ptr(), N);
        (Self { data }, result)
    }

    /// Variant of [\`create_result\`] providing the sized pointer newtype.
    pub fn create_sized_result<R>(mut f: impl FnMut(SizedMutPtr<'_, N>) -> R) -> (Self, R) {
        let mut data = Box::new([0u8; N]);
        let ptr = NonNull::from(data.as_mut()).cast::<u8>();
        let result = f(SizedMutPtr::new(ptr));
        (Self { data }, result)
    }

    /// Safe helper that exposes the buffer as a mutable slice while creating it.
    pub fn create_result_with_slice<R>(mut f: impl FnMut(&mut [u8; N]) -> R) -> (Self, R) {
        let mut data = Box::new([0u8; N]);
        let result = f(data.as_mut());
        (Self { data }, result)
    }

    /// Helper matching `psbCreate`: run `f` for side effects only.
    pub fn create(mut f: impl FnMut(*mut u8)) -> Self {
        Self::create_result(|ptr| {
            f(ptr);
        })
        .0
    }

    /// Helper matching `psbCreateLen`: run `f` with pointer and length.
    pub fn create_len(mut f: impl FnMut(*mut u8, usize)) -> Self {
        Self::create_result_len(|ptr, len| {
            f(ptr, len);
        })
        .0
    }

    /// Helper matching `psbCreateSized`: run `f` with sized pointer.
    pub fn create_sized(mut f: impl FnMut(SizedMutPtr<'_, N>)) -> Self {
        Self::create_sized_result(|ptr| {
            f(ptr);
        })
        .0
    }

    /// Helper mirroring `psbZero`, returning a zero-initialised array.
    #[must_use]
    pub fn zeroed() -> Self {
        Self {
            data: Box::new([0u8; N]),
        }
    }

    /// Equivalent of `psbFromByteString` which panics on size mismatch.
    ///
    /// # Panics
    ///
    /// Panics if `slice.len()` differs from `N`.
    #[must_use]
    pub fn from_slice_or_panic(slice: &[u8]) -> Self {
        Self::from_slice(slice).expect("psbFromByteString: size mismatch")
    }

    /// Convert a pointer to pinned bytes into a sized pointer wrapper.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `ptr` is a valid pointer to a `PinnedSizedBytes<N>` instance
    /// - `ptr` points to memory containing at least N valid bytes
    /// - The lifetime of the returned `SizedPtr` doesn't outlive the pointed-to data
    #[must_use]
    pub fn as_sized_ptr(&self) -> SizedPtr<'_, N> {
        let ptr = NonNull::from(&*self.data).cast::<u8>();
        SizedPtr::new(ptr)
    }
}

impl<const N: usize> Deref for PinnedSizedBytes<N> {
    type Target = [u8; N];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<const N: usize> DerefMut for PinnedSizedBytes<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<const N: usize> fmt::Debug for PinnedSizedBytes<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "\"")?;
        for byte in self.data.iter() {
            write!(f, "{:02x}", byte)?;
        }
        write!(f, "\"")
    }
}

impl<const N: usize> fmt::Display for PinnedSizedBytes<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl<const N: usize> PartialEq for PinnedSizedBytes<N> {
    fn eq(&self, other: &Self) -> bool {
        self.data.as_ref().ct_eq(other.data.as_ref()).unwrap_u8() == 1
    }
}

impl<const N: usize> Eq for PinnedSizedBytes<N> {}

impl<const N: usize> PartialOrd for PinnedSizedBytes<N> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<const N: usize> Ord for PinnedSizedBytes<N> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        constant_time_compare(self.data.as_ref(), other.data.as_ref())
    }
}

impl<const N: usize> From<[u8; N]> for PinnedSizedBytes<N> {
    fn from(value: [u8; N]) -> Self {
        Self::from_array(value)
    }
}

impl<'a, const N: usize> TryFrom<&'a [u8]> for PinnedSizedBytes<N> {
    type Error = PinnedSizedBytesError;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        Self::from_slice(value)
    }
}

impl<const N: usize> TryFrom<Vec<u8>> for PinnedSizedBytes<N> {
    type Error = PinnedSizedBytesError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Self::from_slice(&value)
    }
}

fn constant_time_compare<const N: usize>(lhs: &[u8; N], rhs: &[u8; N]) -> std::cmp::Ordering {
    let mut less = Choice::from(0);
    let mut greater = Choice::from(0);

    for (&x, &y) in lhs.iter().zip(rhs.iter()) {
        let lt = Choice::from((x < y) as u8);
        let gt = Choice::from((x > y) as u8);
        let undecided = !(less | greater);
        less |= undecided & lt;
        greater |= undecided & gt;
    }

    if greater.unwrap_u8() == 1 {
        std::cmp::Ordering::Greater
    } else if less.unwrap_u8() == 1 {
        std::cmp::Ordering::Less
    } else {
        std::cmp::Ordering::Equal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_slice_exact() {
        let value = PinnedSizedBytes::<4>::from_slice(b"ABCD").unwrap();
        assert_eq!(value.as_bytes(), b"ABCD");
    }

    #[test]
    fn from_slice_size_mismatch() {
        let err = PinnedSizedBytes::<4>::from_slice(b"ABC").unwrap_err();
        assert_eq!(
            err,
            PinnedSizedBytesError::SizeMismatch {
                expected: 4,
                actual: 3,
            }
        );
    }

    #[test]
    fn constant_time_equality() {
        let lhs = PinnedSizedBytes::<4>::from_array(*b"ABCD");
        let rhs = PinnedSizedBytes::<4>::from_array(*b"ABCD");
        assert_eq!(lhs, rhs);
    }

    #[test]
    fn constant_time_ordering() {
        let lhs = PinnedSizedBytes::<4>::from_array(*b"ABCD");
        let rhs = PinnedSizedBytes::<4>::from_array(*b"ABCE");
        assert!(lhs < rhs);
    }

    #[test]
    fn create_with_populates_buffer() {
        let (bytes, ()) = PinnedSizedBytes::<4>::create_result_with_slice(|buf| {
            buf.copy_from_slice(b"DATA");
        });
        assert_eq!(bytes.as_bytes(), b"DATA");
    }

    #[test]
    fn create_with_len_passes_length() {
        let (bytes, len) = PinnedSizedBytes::<4>::create_result_with_slice(|buf| {
            buf.copy_from_slice(b"DATA");
            buf.len()
        });
        assert_eq!(bytes.as_bytes(), b"DATA");
        assert_eq!(len, 4);
    }

    #[test]
    fn create_with_sized_ptr_allows_mutation() {
        let (bytes, ()) = PinnedSizedBytes::<4>::create_result_with_slice(|buf| {
            buf.copy_from_slice(b"DATA");
        });
        assert_eq!(bytes.as_bytes(), b"DATA");
    }

    #[test]
    fn padded_constructor_matches_haskell_semantics() {
        let psb = PinnedSizedBytes::<4>::from_bytes_padded(&[1, 2]);
        assert_eq!(psb.as_bytes(), &[0, 0, 1, 2]);

        let psb_long = PinnedSizedBytes::<4>::from_bytes_padded(&[1, 2, 3, 4, 5, 6]);
        assert_eq!(psb_long.as_bytes(), &[3, 4, 5, 6]);
    }

    #[test]
    fn panic_constructor_on_mismatch() {
        let result = std::panic::catch_unwind(|| {
            let _ = PinnedSizedBytes::<4>::from_slice_or_panic(b"abc");
        });
        assert!(result.is_err());
    }
}
