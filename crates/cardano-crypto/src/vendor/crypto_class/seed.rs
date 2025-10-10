use std::fmt;
use std::sync::Arc;

use digest::Digest;
use rand::rngs::OsRng;
use rand_core::{CryptoRng, RngCore};
use thiserror::Error;

/// Deterministic seed material for cryptographic operations.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Seed {
    bytes: Arc<[u8]>,
}

impl fmt::Debug for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Seed")
            .field(&format_args!("{} bytes", self.bytes.len()))
            .finish()
    }
}

impl Seed {
    /// Construct a [`Seed`] from raw bytes.
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Seed {
            bytes: bytes.into().into(),
        }
    }

    /// Return an owned byte vector of the seed contents.
    #[must_use]
    pub fn to_vec(&self) -> Vec<u8> {
        self.bytes.as_ref().to_vec()
    }

    /// View the seed contents as a byte slice.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    /// Number of bytes contained in the seed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether the seed is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Take `n` bytes from the start of the seed, returning the bytes and a
    /// new seed representing the remainder.
    ///
    /// # Errors
    ///
    /// Returns an error if fewer than `n` bytes are available in the seed.
    pub fn take(&self, n: usize) -> Result<(Vec<u8>, Seed), SeedBytesExhausted> {
        get_bytes_from_seed_either(n, self.clone())
    }
}

impl From<Vec<u8>> for Seed {
    fn from(value: Vec<u8>) -> Self {
        Seed::from_bytes(value)
    }
}

impl From<&[u8]> for Seed {
    fn from(value: &[u8]) -> Self {
        Seed::from_bytes(value.to_vec())
    }
}

impl AsRef<[u8]> for Seed {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

/// Construct a [`Seed`] deterministically from the provided bytes.
pub fn mk_seed_from_bytes(bytes: impl Into<Vec<u8>>) -> Seed {
    Seed::from_bytes(bytes)
}

/// Obtain the raw bytes of a [`Seed`].
#[must_use]
pub fn get_seed_bytes(seed: &Seed) -> Vec<u8> {
    seed.to_vec()
}

/// Return the number of bytes stored in the [`Seed`].
#[must_use]
pub fn get_seed_size(seed: &Seed) -> usize {
    seed.len()
}

/// Take `n` bytes from the seed, returning `None` if insufficient bytes remain.
#[must_use]
pub fn get_bytes_from_seed(n: usize, seed: Seed) -> Option<(Vec<u8>, Seed)> {
    get_bytes_from_seed_either(n, seed).ok()
}

/// Take `n` bytes from the seed, returning an error describing how many bytes
/// were supplied versus demanded if there is insufficient material left.
///
/// # Errors
///
/// Returns an error when fewer than `n` bytes remain.
pub fn get_bytes_from_seed_either(
    n: usize,
    seed: Seed,
) -> Result<(Vec<u8>, Seed), SeedBytesExhausted> {
    if seed.bytes.len() < n {
        return Err(SeedBytesExhausted {
            supplied: seed.bytes.len(),
            demanded: n,
        });
    }

    let (head, tail) = seed.bytes.split_at(n);
    Ok((head.to_vec(), Seed::from_bytes(tail.to_vec())))
}

/// Take `n` bytes from the seed, panicking with [`SeedBytesExhausted`] on
/// exhaustion.
///
/// # Panics
///
/// Panics when the seed contains fewer than `n` bytes.
#[must_use]
pub fn get_bytes_from_seed_t(n: usize, seed: Seed) -> (Vec<u8>, Seed) {
    get_bytes_from_seed(n, seed).expect("seed bytes exhausted")
}

/// Split a seed into two smaller seeds. The first contains `n` bytes and the
/// second the remaining bytes.
#[must_use]
pub fn split_seed(n: usize, seed: Seed) -> Option<(Seed, Seed)> {
    get_bytes_from_seed(n, seed).map(|(bytes, remainder)| (Seed::from_bytes(bytes), remainder))
}

/// Expand a seed into two seeds using the specified digest algorithm. The
/// entire input seed is consumed. The resulting seeds each have the length of
/// the digest output.
#[must_use]
pub fn expand_seed<D>(seed: &Seed) -> (Seed, Seed)
where
    D: Digest + Default,
{
    let mut first_hasher = D::new();
    first_hasher.update([1]);
    first_hasher.update(seed.as_ref());
    let first = first_hasher.finalize().to_vec();

    let mut second_hasher = D::new();
    second_hasher.update([2]);
    second_hasher.update(seed.as_ref());
    let second = second_hasher.finalize().to_vec();

    (Seed::from_bytes(first), Seed::from_bytes(second))
}

/// Obtain a [`Seed`] by reading `n` bytes of entropy from the operating
/// system.
///
/// # Panics
///
/// Panics if the operating system RNG fails to provide entropy.
#[must_use]
pub fn read_seed_from_system_entropy(n: usize) -> Seed {
    let mut buffer = vec![0u8; n];
    let mut rng = OsRng;
    rng.fill_bytes(&mut buffer);
    Seed::from_bytes(buffer)
}

/// Error raised when the seed does not contain enough bytes for a request.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("seed bytes exhausted: supplied {supplied}, demanded {demanded}")]
pub struct SeedBytesExhausted {
    pub supplied: usize,
    pub demanded: usize,
}

/// Deterministic RNG backed by a [`Seed`].
#[derive(Clone, Debug)]
pub struct SeedRng {
    data: Vec<u8>,
    position: usize,
}

impl SeedRng {
    /// Create a new RNG from the supplied seed data.
    #[must_use]
    pub fn new(seed: Seed) -> Self {
        Self {
            data: seed.to_vec(),
            position: 0,
        }
    }

    /// Remaining bytes in the RNG.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }

    fn consume(&mut self, amount: usize) -> Result<&[u8], SeedBytesExhausted> {
        if self.remaining() < amount {
            return Err(SeedBytesExhausted {
                supplied: self.remaining(),
                demanded: amount,
            });
        }

        let start = self.position;
        self.position += amount;
        Ok(&self.data[start..self.position])
    }

    /// Fill the provided buffer with bytes from the RNG.
    ///
    /// # Errors
    ///
    /// Returns an error if insufficient bytes remain in the seed.
    pub fn fill_bytes_checked(&mut self, dest: &mut [u8]) -> Result<(), SeedBytesExhausted> {
        let bytes = self.consume(dest.len())?;
        dest.copy_from_slice(bytes);
        Ok(())
    }

    /// Produce an owned vector of the requested number of bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if insufficient bytes remain in the seed.
    pub fn random_bytes(&mut self, len: usize) -> Result<Vec<u8>, SeedBytesExhausted> {
        let bytes = self.consume(len)?;
        Ok(bytes.to_vec())
    }
}

impl RngCore for SeedRng {
    fn next_u32(&mut self) -> u32 {
        let mut buf = [0u8; 4];
        self.fill_bytes_checked(&mut buf)
            .expect("seed bytes exhausted while requesting u32");
        u32::from_le_bytes(buf)
    }

    fn next_u64(&mut self) -> u64 {
        let mut buf = [0u8; 8];
        self.fill_bytes_checked(&mut buf)
            .expect("seed bytes exhausted while requesting u64");
        u64::from_le_bytes(buf)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.fill_bytes_checked(dest)
            .expect("seed bytes exhausted while filling bytes");
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes_checked(dest)
            .map_err(|e| rand_core::Error::new(format!("seed bytes exhausted: {}", e)))
    }
}

impl CryptoRng for SeedRng {}

/// Run a deterministic action using [`SeedRng`].
///
/// # Errors
///
/// Returns an error if the action returns an error or if seed bytes are exhausted.
pub fn run_with_seed<F, R>(seed: Seed, mut f: F) -> Result<R, SeedBytesExhausted>
where
    F: FnMut(&mut SeedRng) -> Result<R, SeedBytesExhausted>,
{
    let mut rng = SeedRng::new(seed);
    f(&mut rng)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Sha256;

    #[test]
    fn take_bytes_success() {
        let seed = mk_seed_from_bytes(vec![1, 2, 3, 4, 5]);
        let (taken, rest) = get_bytes_from_seed(3, seed).expect("enough bytes");
        assert_eq!(taken, vec![1, 2, 3]);
        assert_eq!(rest.to_vec(), vec![4, 5]);
    }

    #[test]
    fn take_bytes_failure() {
        let seed = mk_seed_from_bytes(vec![1, 2]);
        let err = get_bytes_from_seed_either(3, seed).unwrap_err();
        assert_eq!(err.supplied, 2);
        assert_eq!(err.demanded, 3);
    }

    #[test]
    fn split_seed_works() {
        let seed = mk_seed_from_bytes(vec![10, 11, 12, 13]);
        let (left, right) = split_seed(2, seed).expect("split");
        assert_eq!(left.to_vec(), vec![10, 11]);
        assert_eq!(right.to_vec(), vec![12, 13]);
    }

    #[test]
    fn expand_seed_sha256() {
        let seed = mk_seed_from_bytes(vec![0u8; 32]);
        let (a, b) = expand_seed::<Sha256>(&seed);
        assert_eq!(a.len(), 32);
        assert_eq!(b.len(), 32);
        assert_ne!(a.to_vec(), b.to_vec());
    }

    #[test]
    fn seed_rng_yields_bytes() {
        let seed = mk_seed_from_bytes((0u8..=9).collect::<Vec<_>>());
        let mut rng = SeedRng::new(seed);
        let bytes = rng.random_bytes(4).unwrap();
        assert_eq!(bytes, vec![0, 1, 2, 3]);
        let mut rest = [0u8; 3];
        rng.fill_bytes_checked(&mut rest).unwrap();
        assert_eq!(&rest, &[4, 5, 6]);
        assert_eq!(rng.remaining(), 3);
    }

    #[test]
    fn seed_rng_random_bytes_errors_when_exhausted() {
        let seed = mk_seed_from_bytes(vec![0u8; 4]);
        let mut rng = SeedRng::new(seed);
        let err = rng.random_bytes(5).unwrap_err();
        assert_eq!(err.supplied, 4);
        assert_eq!(err.demanded, 5);
    }

    #[test]
    fn run_with_seed_closure() {
        let seed = mk_seed_from_bytes(vec![42, 0, 0, 0]);
        let value = run_with_seed(seed, |rng| {
            let mut buf = [0u8; 4];
            rng.fill_bytes_checked(&mut buf)?;
            Ok(u32::from_le_bytes(buf))
        })
        .unwrap();
        assert_eq!(value, 42);
    }

    #[test]
    fn run_with_seed_error_propagates() {
        let seed = mk_seed_from_bytes(vec![1, 2]);
        let err = run_with_seed(seed, |rng| {
            let mut buf = [0u8; 4];
            rng.fill_bytes_checked(&mut buf)?;
            Ok(())
        })
        .unwrap_err();
        assert_eq!(err.supplied, 2);
        assert_eq!(err.demanded, 4);
    }
}
