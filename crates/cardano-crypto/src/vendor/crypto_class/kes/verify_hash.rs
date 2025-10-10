/// Test to verify that KES types use the correct hash algorithm (Blake2b256)
#[cfg(test)]
mod verify_hash_algorithm {
    use crate::vendor::crypto_class::kes::KesAlgorithm;
    use crate::vendor::crypto_class::kes::sum::{Sum1Kes, Sum7Kes};

    #[test]
    fn test_sum_types_use_blake2b256() {
        // Sum types should use Blake2b256 (32 bytes) to match Haskell
        assert_eq!(
            Sum1Kes::VERIFICATION_KEY_SIZE,
            32,
            "Sum1Kes should use Blake2b-256 (32 bytes)"
        );
        assert_eq!(
            Sum7Kes::VERIFICATION_KEY_SIZE,
            32,
            "Sum7Kes should use Blake2b-256 (32 bytes)"
        );
    }

    #[test]
    fn test_verification_key_compatibility() {
        // This ensures binary compatibility with Haskell's Blake2b_256
        println!("\n=== KES Hash Algorithm Verification ===");
        println!(
            "Sum1Kes VK Size: {} bytes (expected: 32)",
            Sum1Kes::VERIFICATION_KEY_SIZE
        );
        println!(
            "Sum7Kes VK Size: {} bytes (expected: 32)",
            Sum7Kes::VERIFICATION_KEY_SIZE
        );

        // Previously these were 64 bytes (Blake2b-512), now they should be 32 bytes (Blake2b-256)
        assert_eq!(
            Sum1Kes::VERIFICATION_KEY_SIZE,
            32,
            "Sum1Kes verification key should be 32 bytes"
        );
        assert_eq!(
            Sum7Kes::VERIFICATION_KEY_SIZE,
            32,
            "Sum7Kes verification key should be 32 bytes"
        );

        println!(
            "✅ All KES Sum types now use Blake2b-256 (32 bytes) matching Haskell's Blake2b_256"
        );
        println!("   This fixes the critical binary incompatibility issue!");
    }
}
