//! Property-based tests for CardanoDB types
//!
//! These tests use proptest to verify type invariants hold across
//! a wide range of generated inputs.

use crate::cardanodb::types::{Blake2b256Hash, BlockLocation, ChunkNo};
use proptest::prelude::*;

// Strategy for generating valid Blake2b256Hash values
fn blake2b_hash_strategy() -> impl Strategy<Value = Blake2b256Hash> {
    prop::array::uniform32(any::<u8>()).prop_map(Blake2b256Hash::new)
}

// Strategy for generating BlockLocation values
fn block_location_strategy() -> impl Strategy<Value = BlockLocation> {
    (0u64..1_000_000, 1u32..1_000_000).prop_map(|(offset, size)| BlockLocation::new(offset, size))
}

// Strategy for generating ChunkNo values
fn chunk_no_strategy() -> impl Strategy<Value = ChunkNo> {
    any::<u64>().prop_map(ChunkNo::new)
}

proptest! {
    #[test]
    fn blake2b_hash_from_slice_roundtrip(bytes in prop::array::uniform32(any::<u8>())) {
        let hash = Blake2b256Hash::new(bytes);
        let slice = hash.as_bytes();
        let recovered = Blake2b256Hash::from_slice(slice).unwrap();
        prop_assert_eq!(hash, recovered);
    }

    #[test]
    fn blake2b_hash_display_is_hex(bytes in prop::array::uniform32(any::<u8>())) {
        let hash = Blake2b256Hash::new(bytes);
        let display = format!("{}", hash);

        // Should be valid hex
        prop_assert_eq!(display.len(), 64); // 32 bytes * 2 hex chars
        prop_assert!(display.chars().all(|c| c.is_ascii_hexdigit()));

        // Should round-trip through hex
        let decoded = hex::decode(&display).unwrap();
        prop_assert_eq!(&decoded[..], hash.as_bytes());
    }

    #[test]
    fn blake2b_hash_from_invalid_slice_fails(
        bytes in prop::collection::vec(any::<u8>(), 0..100)
    ) {
        // Filter out valid size
        prop_assume!(bytes.len() != 32);

        let result = Blake2b256Hash::from_slice(&bytes);
        prop_assert!(result.is_none());
    }

    #[test]
    fn blake2b_hash_computation_is_deterministic(data in prop::collection::vec(any::<u8>(), 0..1000)) {
        let hash1 = Blake2b256Hash::hash(&data);
        let hash2 = Blake2b256Hash::hash(&data);
        prop_assert_eq!(hash1, hash2);
    }

    #[test]
    fn blake2b_hash_different_data_different_hash(
        data1 in prop::collection::vec(any::<u8>(), 1..100),
        data2 in prop::collection::vec(any::<u8>(), 1..100)
    ) {
        prop_assume!(data1 != data2);

        let hash1 = Blake2b256Hash::hash(&data1);
        let hash2 = Blake2b256Hash::hash(&data2);

        // With overwhelming probability, different data should produce different hashes
        // (collision resistance property of cryptographic hashes)
        prop_assert_ne!(hash1, hash2);
    }

    #[test]
    fn block_location_end_offset_calculation(offset in 0u64..1_000_000, size in 1u32..1_000_000) {
        let loc = BlockLocation::new(offset, size);
        prop_assert_eq!(loc.end_offset(), offset + size as u64);
    }

    #[test]
    fn block_location_contains_own_offset(offset in 0u64..1_000_000, size in 1u32..1_000_000) {
        let loc = BlockLocation::new(offset, size);
        prop_assert!(offset >= loc.offset);
        prop_assert!(offset < loc.end_offset());
    }

    #[test]
    fn chunk_no_ordering_preserves_u64_ordering(a in any::<u64>(), b in any::<u64>()) {
        let chunk_a = ChunkNo::new(a);
        let chunk_b = ChunkNo::new(b);

        prop_assert_eq!(chunk_a.cmp(&chunk_b), a.cmp(&b));
    }

    #[test]
    fn chunk_no_roundtrip(n in any::<u64>()) {
        let chunk = ChunkNo::new(n);
        prop_assert_eq!(chunk.to_u64(), n);
    }

    #[test]
    fn block_location_serialization_roundtrip(loc in block_location_strategy()) {
        let serialized = serde_json::to_vec(&loc).unwrap();
        let deserialized: BlockLocation = serde_json::from_slice(&serialized).unwrap();
        prop_assert_eq!(loc, deserialized);
    }

    #[test]
    fn chunk_no_serialization_roundtrip(chunk in chunk_no_strategy()) {
        let serialized = serde_json::to_vec(&chunk).unwrap();
        let deserialized: ChunkNo = serde_json::from_slice(&serialized).unwrap();
        prop_assert_eq!(chunk, deserialized);
    }

    #[test]
    fn blake2b_hash_serialization_roundtrip(hash in blake2b_hash_strategy()) {
        let serialized = serde_json::to_vec(&hash).unwrap();
        let deserialized: Blake2b256Hash = serde_json::from_slice(&serialized).unwrap();
        prop_assert_eq!(hash, deserialized);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategies_produce_valid_values() {
        // Smoke test that our strategies work
        proptest!(|(
            hash in blake2b_hash_strategy(),
            loc in block_location_strategy(),
            chunk in chunk_no_strategy()
        )| {
            // Just verify they can be constructed
            prop_assert!(hash.as_bytes().len() == 32);
            prop_assert!(loc.size > 0);
            // ChunkNo is u64, always >= 0
            let _ = chunk;
        });
    }
}
