//! Direct serialisation helpers mirroring `Cardano.Crypto.DirectSerialise`.
//!
//! These utilities expose type classes (traits) for serialising data directly
//! to raw memory without intermediate heap allocations. This supports
//! zero-copy communication of key material while maintaining safety checks on
//! buffer sizes.

use std::cell::Cell;

use thiserror::Error;

/// Error raised when a direct serialisation or deserialisation operation
/// writes or reads more bytes than expected.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("size check failed: expected {expected_size}, actual {actual_size}")]
pub struct SizeCheckError {
    pub expected_size: usize,
    pub actual_size: usize,
}

/// Convenience alias for results produced by direct serialisation helpers.
pub type DirectResult<T> = Result<T, SizeCheckError>;

/// Trait for types that can expose their internal representation as raw
/// memory blocks for serialisation.
pub trait DirectSerialise {
    /// Emit the raw representation via the supplied callback.
    ///
    /// # Errors
    ///
    /// Propagates any error returned by `f`.
    fn direct_serialise(&self, f: &mut dyn FnMut(&[u8]) -> DirectResult<()>) -> DirectResult<()>;
}

/// Trait for types that can be reconstructed from raw memory blocks during
/// deserialisation.
pub trait DirectDeserialise: Sized {
    /// Consume the supplied callback to reconstruct the value.
    ///
    /// # Errors
    ///
    /// Propagates any error returned by `f`.
    fn direct_deserialise(f: &mut dyn FnMut(&mut [u8]) -> DirectResult<()>) -> DirectResult<Self>;
}

/// Helper that writes into a destination buffer, ensuring no more than
/// `dst_len` bytes are produced. Returns the number of bytes written.
///
/// # Errors
///
/// Returns an error if more than `dst_len` bytes are written.
pub fn direct_serialise_to<T: DirectSerialise>(
    mut write: impl FnMut(usize, &[u8]) -> DirectResult<()>,
    dst_len: usize,
    value: &T,
) -> DirectResult<usize> {
    let pos = Cell::new(0usize);

    value.direct_serialise(&mut |chunk| {
        let len = chunk.len();
        let current = pos.get();
        let next = current + len;
        if next > dst_len {
            return Err(SizeCheckError {
                expected_size: dst_len - current,
                actual_size: next - current,
            });
        }
        write(current, chunk)?;
        pos.set(next);
        Ok(())
    })?;

    Ok(pos.get())
}

/// Size-checked variant ensuring exactly `dst_len` bytes are written.
///
/// # Errors
///
/// Returns an error if:
/// - More than `dst_len` bytes are written
/// - Fewer than `dst_len` bytes are written
pub fn direct_serialise_to_checked<T: DirectSerialise>(
    write: impl FnMut(usize, &[u8]) -> DirectResult<()>,
    dst_len: usize,
    value: &T,
) -> DirectResult<()> {
    let written = direct_serialise_to(write, dst_len, value)?;
    if written != dst_len {
        Err(SizeCheckError {
            expected_size: dst_len,
            actual_size: written,
        })
    } else {
        Ok(())
    }
}

/// Serialise to an in-memory buffer.
///
/// # Errors
///
/// Returns an error if more than `dst_len` bytes are written.
pub fn direct_serialise_buf<T: DirectSerialise>(dst: &mut [u8], value: &T) -> DirectResult<usize> {
    let dst_len = dst.len();
    direct_serialise_to(
        |offset, chunk| {
            let end = offset + chunk.len();
            dst[offset..end].copy_from_slice(chunk);
            Ok(())
        },
        dst_len,
        value,
    )
}

/// Serialise to an in-memory buffer, ensuring the buffer is filled exactly.
///
/// # Errors
///
/// Returns an error if:
/// - More than `dst_len` bytes are written
/// - Fewer than `dst_len` bytes are written
pub fn direct_serialise_buf_checked<T: DirectSerialise>(
    dst: &mut [u8],
    value: &T,
) -> DirectResult<()> {
    let dst_len = dst.len();
    direct_serialise_to_checked(
        |offset, chunk| {
            let end = offset + chunk.len();
            dst[offset..end].copy_from_slice(chunk);
            Ok(())
        },
        dst_len,
        value,
    )
}

/// Helper that reads from a source buffer, ensuring no more than `src_len`
/// bytes are consumed. Returns the deserialised value and the number of bytes
/// read.
///
/// # Errors
///
/// Returns an error if more than `src_len` bytes are read.
pub fn direct_deserialise_from<T: DirectDeserialise>(
    mut read: impl FnMut(usize, &mut [u8]) -> DirectResult<()>,
    src_len: usize,
) -> DirectResult<(T, usize)> {
    let pos = Cell::new(0usize);

    let value = T::direct_deserialise(&mut |chunk| {
        let len = chunk.len();
        let current = pos.get();
        let next = current + len;
        if next > src_len {
            return Err(SizeCheckError {
                expected_size: src_len - current,
                actual_size: next - current,
            });
        }
        read(current, chunk)?;
        pos.set(next);
        Ok(())
    })?;

    Ok((value, pos.get()))
}

/// Size-checked variant ensuring all `src_len` bytes are consumed.
///
/// # Errors
///
/// Returns an error if:
/// - More than `src_len` bytes are read
/// - Fewer than `src_len` bytes are read
pub fn direct_deserialise_from_checked<T: DirectDeserialise>(
    read: impl FnMut(usize, &mut [u8]) -> DirectResult<()>,
    src_len: usize,
) -> DirectResult<T> {
    let (value, read_len) = direct_deserialise_from(read, src_len)?;
    if read_len != src_len {
        Err(SizeCheckError {
            expected_size: src_len,
            actual_size: read_len,
        })
    } else {
        Ok(value)
    }
}

/// Deserialise from an in-memory buffer with bounds checking.
///
/// # Errors
///
/// Returns an error if more than `src_len` bytes are read.
pub fn direct_deserialise_buf<T: DirectDeserialise>(src: &[u8]) -> DirectResult<(T, usize)> {
    direct_deserialise_from(
        |offset, dst| {
            let end = offset + dst.len();
            dst.copy_from_slice(&src[offset..end]);
            Ok(())
        },
        src.len(),
    )
}

/// Deserialise from an in-memory buffer, ensuring the buffer is consumed.
///
/// # Errors
///
/// Returns an error if:
/// - More than `src_len` bytes are read
/// - Fewer than `src_len` bytes are read
pub fn direct_deserialise_buf_checked<T: DirectDeserialise>(src: &[u8]) -> DirectResult<T> {
    direct_deserialise_from_checked(
        |offset, dst| {
            let end = offset + dst.len();
            dst.copy_from_slice(&src[offset..end]);
            Ok(())
        },
        src.len(),
    )
}

// ============================================================================
// Standard implementations for common types
// ============================================================================

/// Implementation for Vec<u8> to support hash-based verification keys in SumKES and CompactSumKES.
/// Since Vec<u8> has dynamic length, the serialization simply writes all bytes.
impl DirectSerialise for Vec<u8> {
    fn direct_serialise(&self, f: &mut dyn FnMut(&[u8]) -> DirectResult<()>) -> DirectResult<()> {
        f(&self[..])
    }
}

/// Implementation for Vec<u8> to support hash-based verification keys in SumKES and CompactSumKES.
/// The deserialization reads bytes into a temporary buffer and returns it as a Vec.
/// This is used for variable-length data like hashed verification keys.
impl DirectDeserialise for Vec<u8> {
    fn direct_deserialise(f: &mut dyn FnMut(&mut [u8]) -> DirectResult<()>) -> DirectResult<Self> {
        // For Vec<u8>, we need a fixed-size buffer to read into.
        // The KES verification keys are 32-byte hashes, so we use that as the standard size.
        const HASH_SIZE: usize = 32;
        let mut buffer = vec![0u8; HASH_SIZE];
        f(&mut buffer)?;
        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Pair([u8; 4], [u8; 4]);

    impl DirectSerialise for Pair {
        fn direct_serialise(
            &self,
            f: &mut dyn FnMut(&[u8]) -> DirectResult<()>,
        ) -> DirectResult<()> {
            f(&self.0)?;
            f(&self.1)
        }
    }

    impl DirectDeserialise for Pair {
        fn direct_deserialise(
            f: &mut dyn FnMut(&mut [u8]) -> DirectResult<()>,
        ) -> DirectResult<Self> {
            let mut first = [0u8; 4];
            let mut second = [0u8; 4];
            f(&mut first)?;
            f(&mut second)?;
            Ok(Pair(first, second))
        }
    }

    #[test]
    fn serialise_to_buffer_roundtrip() {
        let pair = Pair(*b"ABCD", *b"WXYZ");
        let mut buffer = [0u8; 8];
        direct_serialise_buf_checked(&mut buffer, &pair).unwrap();
        assert_eq!(&buffer, b"ABCDWXYZ");

        let (decoded, read) = direct_deserialise_buf::<Pair>(&buffer).unwrap();
        assert_eq!(read, buffer.len());
        assert_eq!(decoded, pair);
    }

    #[test]
    fn serialise_size_mismatch_errors() {
        let pair = Pair(*b"AAAA", *b"BBBB");
        let mut buf = [0u8; 4];
        let err = direct_serialise_buf(&mut buf, &pair).unwrap_err();
        assert_eq!(
            err,
            SizeCheckError {
                expected_size: 0,
                actual_size: 4
            }
        );
    }

    #[test]
    fn deserialise_size_mismatch_errors() {
        let buf = [0u8; 4];
        let err = direct_deserialise_buf_checked::<Pair>(&buf).unwrap_err();
        assert_eq!(
            err,
            SizeCheckError {
                expected_size: 0,
                actual_size: 4
            }
        );
    }
}
