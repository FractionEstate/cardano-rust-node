/// Trait for hash algorithms used in KES schemes.
///
/// This trait provides a simple interface for hash algorithms used in
/// Key Evolving Signature (KES) constructions, particularly for the
/// binary sum composition where verification keys are hashed.
pub trait KesHashAlgorithm: Clone + Send + Sync + 'static {
    /// The size of the hash output in bytes.
    const OUTPUT_SIZE: usize;

    /// The name of the hash algorithm (for debugging).
    const ALGORITHM_NAME: &'static str;

    /// Hash arbitrary data and return a fixed-size output.
    fn hash(data: &[u8]) -> Vec<u8>;

    /// Hash two pieces of data concatenated together.
    /// Default implementation concatenates then hashes, but can be overridden for efficiency.
    #[must_use]
    fn hash_concat(data1: &[u8], data2: &[u8]) -> Vec<u8> {
        let mut combined = Vec::with_capacity(data1.len() + data2.len());
        combined.extend_from_slice(data1);
        combined.extend_from_slice(data2);
        Self::hash(&combined)
    }

    /// Expand a seed into two seeds using the hash algorithm.
    /// This is used for seed expansion in Sum compositions.
    /// Uses prefixes 1 and 2 to match Haskell cardano-base implementation:
    /// - r0 = hash(1 || seed)
    /// - r1 = hash(2 || seed)
    #[must_use]
    fn expand_seed(seed: &[u8]) -> (Vec<u8>, Vec<u8>) {
        // Hash with different prefixes to get two independent seeds
        // Using 1 and 2 to match Haskell: BS.cons 1 and BS.cons 2
        let mut seed0_input = vec![1u8];
        seed0_input.extend_from_slice(seed);
        let seed0 = Self::hash(&seed0_input);

        let mut seed1_input = vec![2u8];
        seed1_input.extend_from_slice(seed);
        let seed1 = Self::hash(&seed1_input);

        (seed0, seed1)
    }
}

/// Blake2b-224 hash algorithm (28-byte output).
/// Mirrors the shorter digest variant used for verification key hashing in
/// address derivation paths.
#[derive(Clone, Debug)]
pub struct Blake2b224;

impl KesHashAlgorithm for Blake2b224 {
    const OUTPUT_SIZE: usize = 28;
    const ALGORITHM_NAME: &'static str = "blake2b_224";

    fn hash(data: &[u8]) -> Vec<u8> {
        use blake2::digest::consts::U28;
        use blake2::{Blake2b, Digest};

        let mut hasher = Blake2b::<U28>::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }
}

/// Blake2b-256 hash algorithm (32-byte output).
/// This is the hash algorithm used in Haskell's cardano-base for Sum types.
#[derive(Clone, Debug)]
pub struct Blake2b256;

impl KesHashAlgorithm for Blake2b256 {
    const OUTPUT_SIZE: usize = 32;
    const ALGORITHM_NAME: &'static str = "blake2b_256";

    fn hash(data: &[u8]) -> Vec<u8> {
        use blake2::digest::consts::U32;
        use blake2::{Blake2b, Digest};

        let mut hasher = Blake2b::<U32>::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }
}

/// Blake2b-512 hash algorithm (64-byte output).
/// This is kept for compatibility with existing code that may use it.
#[derive(Clone, Debug)]
pub struct Blake2b512;

impl KesHashAlgorithm for Blake2b512 {
    const OUTPUT_SIZE: usize = 64;
    const ALGORITHM_NAME: &'static str = "blake2b_512";

    fn hash(data: &[u8]) -> Vec<u8> {
        use blake2::{Blake2b512 as Blake2b512Hasher, Digest};

        let mut hasher = Blake2b512Hasher::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blake2b224_output_size() {
        let data = b"test data";
        let hash = Blake2b224::hash(data);
        assert_eq!(hash.len(), 28, "Blake2b-224 should output 28 bytes");
    }

    #[test]
    fn test_blake2b256_output_size() {
        let data = b"test data";
        let hash = Blake2b256::hash(data);
        assert_eq!(hash.len(), 32, "Blake2b-256 should output 32 bytes");
    }

    #[test]
    fn test_blake2b512_output_size() {
        let data = b"test data";
        let hash = Blake2b512::hash(data);
        assert_eq!(hash.len(), 64, "Blake2b-512 should output 64 bytes");
    }

    #[test]
    fn test_hash_concat() {
        let data1 = b"hello";
        let data2 = b"world";
        let hash1 = Blake2b256::hash_concat(data1, data2);
        let hash2 = Blake2b256::hash(b"helloworld");
        assert_eq!(hash1, hash2, "hash_concat should match concatenated hash");
    }

    #[test]
    fn test_expand_seed() {
        let seed = b"test seed";
        let (seed0, seed1) = Blake2b256::expand_seed(seed);
        assert_eq!(seed0.len(), 32);
        assert_eq!(seed1.len(), 32);
        assert_ne!(seed0, seed1, "Expanded seeds should be different");
    }
}
