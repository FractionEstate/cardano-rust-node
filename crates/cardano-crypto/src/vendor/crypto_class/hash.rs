// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Extended hash functions for cross-chain compatibility.
//!
//! This module provides additional hash algorithms beyond Blake2b (used in Cardano),
//! supporting cross-chain bridges to Bitcoin, Ethereum, and other blockchain systems.
//!
//! # Algorithms
//!
//! - **SHA-256**: Used in Bitcoin
//! - **SHA-512**: General cryptographic use
//! - **SHA3-256**: Keccak-based, used in Ethereum 2.0
//! - **SHA3-512**: Keccak-based, longer digest
//! - **Keccak-256**: Original Keccak, used in Ethereum 1.0
//! - **RIPEMD-160**: Used in Bitcoin addresses
//! - **Blake2b-224/256/512**: Used across Cardano for verification-key and tree hashing

use blake2::Blake2b;
use blake2::digest::consts::U28;
use digest::Digest;
use ripemd::Ripemd160;
use sha2::{Sha256, Sha512};
use sha3::{Keccak256, Sha3_256, Sha3_512};
use subtle::ConstantTimeEq;

// Re-export KES Blake2b implementations for unified hashing API surface.
pub use crate::vendor::crypto_class::kes::hash::{Blake2b224, Blake2b256, Blake2b512};

/// SHA-256 hash (32 bytes output).
///
/// Used extensively in Bitcoin for transaction hashing, block mining, and address generation.
#[must_use]
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Double SHA-256 hash (32 bytes output).
///
/// Common pattern in Bitcoin: `SHA256(SHA256(data))`.
/// Used for transaction IDs and block hashing.
#[must_use]
pub fn sha256d(data: &[u8]) -> [u8; 32] {
    sha256(&sha256(data))
}

/// SHA-512 hash (64 bytes output).
///
/// General purpose cryptographic hash with longer output.
#[must_use]
pub fn sha512(data: &[u8]) -> [u8; 64] {
    let mut hasher = Sha512::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// SHA3-256 hash (32 bytes output).
///
/// Keccak-based standardized hash function.
/// Used in Ethereum 2.0 and various modern protocols.
#[must_use]
pub fn sha3_256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// SHA3-512 hash (64 bytes output).
///
/// Keccak-based standardized hash function with longer output.
#[must_use]
pub fn sha3_512(data: &[u8]) -> [u8; 64] {
    let mut hasher = Sha3_512::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Keccak-256 hash (32 bytes output).
///
/// Original Keccak algorithm before NIST standardization.
/// Used in Ethereum 1.0 for transaction hashing and address generation.
#[must_use]
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// RIPEMD-160 hash (20 bytes output).
///
/// Used in Bitcoin address generation: `RIPEMD160(SHA256(pubkey))`.
#[must_use]
pub fn ripemd160(data: &[u8]) -> [u8; 20] {
    let mut hasher = Ripemd160::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Bitcoin-style address hash: `RIPEMD160(SHA256(data))`.
///
/// Used in Bitcoin P2PKH address generation.
#[must_use]
pub fn hash160(data: &[u8]) -> [u8; 20] {
    ripemd160(&sha256(data))
}

/// Blake2b-224 hash (28 bytes output).
///
/// Cardano uses this digest size when hashing verification keys during address
/// construction. The variant matches `Cardano.Crypto.Hash.Blake2b_224` by
/// fixing the output length to 224 bits without truncating a longer digest.
#[must_use]
pub fn blake2b224(data: &[u8]) -> [u8; 28] {
    let mut hasher = Blake2b::<U28>::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Constant-time equality over raw hash byte slices.
///
/// Returns `false` if the inputs differ in length. When lengths match, the
/// comparison is performed in constant time using `subtle::ConstantTimeEq` to
/// avoid data-dependent early exits.
#[must_use]
pub fn constant_time_eq(lhs: &[u8], rhs: &[u8]) -> bool {
    if lhs.len() != rhs.len() {
        return false;
    }

    bool::from(lhs.ct_eq(rhs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vendor::crypto_class::kes::hash::KesHashAlgorithm; // bring trait providing ::hash into scope

    #[test]
    fn test_blake2b224_empty() {
        let expected = "836cc68931c2e4e3e838602eca1902591d216837bafddfe6f0c8cb07"; // Blake2b-224("")
        let out = blake2b224(b"");
        assert_eq!(hex::encode(out), expected);
    }

    #[test]
    fn test_blake2b224_hello_world() {
        let expected = "42d1854b7d69e3b57c64fcc7b4f64171b47dff43fba6ac0499ff437f"; // Blake2b-224("hello world")
        let out = blake2b224(b"hello world");
        assert_eq!(hex::encode(out), expected);
    }

    #[test]
    fn test_blake2b256_empty() {
        let expected = "0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a8"; // Blake2b-256("")
        let out = Blake2b256::hash(b"");
        assert_eq!(hex::encode(out), expected);
    }

    #[test]
    fn test_blake2b256_hello_world() {
        let expected = "256c83b297114d201b30179f3f0ef0cace9783622da5974326b436178aeef610"; // Blake2b-256("hello world")
        let out = Blake2b256::hash(b"hello world");
        assert_eq!(hex::encode(out), expected);
    }

    #[test]
    fn test_blake2b512_empty() {
        let expected = "786a02f742015903c6c6fd852552d272912f4740e15847618a86e217f71f5419d25e1031afee585313896444934eb04b903a685b1448b755d56f701afe9be2ce"; // Blake2b-512("")
        let out = Blake2b512::hash(b"");
        assert_eq!(hex::encode(out), expected);
    }

    #[test]
    fn test_blake2b512_hello_world() {
        let expected = "021ced8799296ceca557832ab941a50b4a11f83478cf141f51f933f653ab9fbcc05a037cddbed06e309bf334942c4e58cdf1a46e237911ccd7fcf9787cbc7fd0"; // Blake2b-512("hello world")
        let out = Blake2b512::hash(b"hello world");
        assert_eq!(hex::encode(out), expected);
    }

    #[test]
    fn test_sha256_empty() {
        let hash = sha256(b"");
        let expected =
            hex::decode("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
                .unwrap();
        assert_eq!(hash.as_ref(), expected.as_slice());
    }

    #[test]
    fn test_sha256_hello() {
        let hash = sha256(b"hello world");
        let expected =
            hex::decode("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9")
                .unwrap();
        assert_eq!(hash.as_ref(), expected.as_slice());
    }

    #[test]
    fn test_sha256d() {
        // Bitcoin double-sha256
        let hash = sha256d(b"hello");
        // Should be SHA256(SHA256("hello"))
        let first = sha256(b"hello");
        let expected = sha256(&first);
        assert_eq!(hash, expected);
    }

    #[test]
    fn test_sha512_hello() {
        let hash = sha512(b"hello world");
        let expected = hex::decode(
            "309ecc489c12d6eb4cc40f50c902f2b4d0ed77ee511a7c7a9bcd3ca86d4cd86f\
             989dd35bc5ff499670da34255b45b0cfd830e81f605dcf7dc5542e93ae9cd76f",
        )
        .unwrap();
        assert_eq!(hash.as_ref(), expected.as_slice());
    }

    #[test]
    fn test_sha3_256_empty() {
        let hash = sha3_256(b"");
        let expected =
            hex::decode("a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a")
                .unwrap();
        assert_eq!(hash.as_ref(), expected.as_slice());
    }

    #[test]
    fn test_sha3_512_empty() {
        let hash = sha3_512(b"");
        let expected = hex::decode(
            "a69f73cca23a9ac5c8b567dc185a756e97c982164fe25859e0d1dcc1475c80a6\
             15b2123af1f5f94c11e3e9402c3ac558f500199d95b6d3e301758586281dcd26",
        )
        .unwrap();
        assert_eq!(hash.as_ref(), expected.as_slice());
    }

    #[test]
    fn test_keccak256_empty() {
        let hash = keccak256(b"");
        let expected =
            hex::decode("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470")
                .unwrap();
        assert_eq!(hash.as_ref(), expected.as_slice());
    }

    #[test]
    fn test_keccak256_vs_sha3_256() {
        // Keccak256 and SHA3-256 should produce different results
        // (different padding in NIST standardization)
        let data = b"test";
        let keccak = keccak256(data);
        let sha3 = sha3_256(data);
        assert_ne!(keccak, sha3);
    }

    #[test]
    fn test_ripemd160_empty() {
        let hash = ripemd160(b"");
        let expected = hex::decode("9c1185a5c5e9fc54612808977ee8f548b2258d31").unwrap();
        assert_eq!(hash.as_ref(), expected.as_slice());
    }

    #[test]
    fn test_ripemd160_hello() {
        let hash = ripemd160(b"hello world");
        let expected = hex::decode("98c615784ccb5fe5936fbc0cbe9dfdb408d92f0f").unwrap();
        assert_eq!(hash.as_ref(), expected.as_slice());
    }

    #[test]
    fn test_hash160() {
        // Bitcoin-style RIPEMD160(SHA256(data))
        let hash = hash160(b"hello");
        let sha_first = sha256(b"hello");
        let expected = ripemd160(&sha_first);
        assert_eq!(hash, expected);
    }

    #[test]
    fn test_constant_time_eq_success() {
        let a = sha256(b"ct-eq");
        let b = sha256(b"ct-eq");
        assert!(constant_time_eq(&a, &b));
    }

    #[test]
    fn test_constant_time_eq_detects_difference() {
        let a = sha256(b"left");
        let mut b = a;
        b[0] ^= 0xff;
        assert!(!constant_time_eq(&a, &b));
    }

    #[test]
    fn test_constant_time_eq_length_mismatch() {
        let a = sha256(b"short");
        let b = hash160(b"short");
        assert!(!constant_time_eq(&a, &b));
    }

    #[test]
    fn test_all_hash_lengths() {
        let data = b"test data";

        assert_eq!(sha256(data).len(), 32);
        assert_eq!(sha256d(data).len(), 32);
        assert_eq!(sha512(data).len(), 64);
        assert_eq!(sha3_256(data).len(), 32);
        assert_eq!(sha3_512(data).len(), 64);
        assert_eq!(keccak256(data).len(), 32);
        assert_eq!(ripemd160(data).len(), 20);
        assert_eq!(hash160(data).len(), 20);
        assert_eq!(blake2b224(data).len(), 28);
    }

    #[test]
    fn test_blake2b_output_lengths() {
        let inputs = [b"".as_ref(), b"cardano".as_ref(), b"hash-length".as_ref()];

        for input in inputs {
            assert_eq!(Blake2b224::hash(input).len(), 28);
            assert_eq!(Blake2b256::hash(input).len(), 32);
            assert_eq!(Blake2b512::hash(input).len(), 64);
        }
    }

    #[test]
    fn test_large_input_hashing_stability() {
        let mut data = vec![0u8; 1 << 20]; // 1 MiB repeating pattern
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = (i & 0xff) as u8;
        }

        let sha256_once = sha256(&data);
        assert_eq!(sha256_once.len(), 32);
        assert_eq!(sha256_once, sha256(&data));

        let sha256d_once = sha256d(&data);
        assert_eq!(sha256d_once.len(), 32);
        assert_eq!(sha256d_once, sha256d(&data));

        let sha512_once = sha512(&data);
        assert_eq!(sha512_once.len(), 64);
        assert_eq!(sha512_once, sha512(&data));

        let sha3_256_once = sha3_256(&data);
        assert_eq!(sha3_256_once.len(), 32);
        assert_eq!(sha3_256_once, sha3_256(&data));

        let sha3_512_once = sha3_512(&data);
        assert_eq!(sha3_512_once.len(), 64);
        assert_eq!(sha3_512_once, sha3_512(&data));

        let keccak_once = keccak256(&data);
        assert_eq!(keccak_once.len(), 32);
        assert_eq!(keccak_once, keccak256(&data));

        let ripemd_once = ripemd160(&data);
        assert_eq!(ripemd_once.len(), 20);
        assert_eq!(ripemd_once, ripemd160(&data));

        let hash160_once = hash160(&data);
        assert_eq!(hash160_once.len(), 20);
        assert_eq!(hash160_once, hash160(&data));

        let blake2b224_once = blake2b224(&data);
        assert_eq!(blake2b224_once.len(), 28);
        assert_eq!(blake2b224_once, blake2b224(&data));

        let blake2b256_once = Blake2b256::hash(&data);
        assert_eq!(blake2b256_once.len(), 32);
        assert_eq!(blake2b256_once, Blake2b256::hash(&data));

        let blake2b512_once = Blake2b512::hash(&data);
        assert_eq!(blake2b512_once.len(), 64);
        assert_eq!(blake2b512_once, Blake2b512::hash(&data));
    }

    #[test]
    fn test_deterministic_hashing() {
        // Hashing should be deterministic
        let data = b"deterministic test";

        assert_eq!(sha256(data), sha256(data));
        assert_eq!(sha512(data), sha512(data));
        assert_eq!(keccak256(data), keccak256(data));
        assert_eq!(ripemd160(data), ripemd160(data));
        assert_eq!(blake2b224(data), blake2b224(data));
    }

    #[test]
    fn test_blake2b256_not_simple_truncation() {
        let cases = [
            b"".as_ref(),
            b"cardano".as_ref(),
            b"longer-message-for-blake2b-compat".as_ref(),
        ];

        for input in cases {
            let blake512 = Blake2b512::hash(input);
            let blake256 = Blake2b256::hash(input);
            assert_ne!(&blake512[..32], &blake256[..]);
        }
    }

    #[test]
    fn test_blake2b224_not_truncation() {
        let cases = [
            b"".as_ref(),
            b"address-key".as_ref(),
            b"longer-message-for-blake2b-224".as_ref(),
        ];

        for input in cases {
            let blake512 = Blake2b512::hash(input);
            let blake224 = blake2b224(input);
            assert_ne!(&blake512[..28], blake224.as_ref());

            let blake256 = Blake2b256::hash(input);
            assert_ne!(&blake256[..28], blake224.as_ref());
        }
    }
}
