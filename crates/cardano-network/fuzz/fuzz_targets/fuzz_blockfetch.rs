#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz BlockFetch protocol message decoding
    // BlockFetch deals with block data that must be validated carefully

    // Try to decode as Point
    if data.len() >= 40 {
        let _ = cardano_network::protocols::blockfetch::Point::new(
            u64::from_be_bytes(data[0..8].try_into().unwrap_or([0; 8])),
            &data[8..40],
        );
    }

    // Try to parse as block range
    if data.len() >= 80 {
        let point1 = cardano_network::protocols::blockfetch::Point::new(
            u64::from_be_bytes(data[0..8].try_into().unwrap_or([0; 8])),
            &data[8..40],
        );
        let point2 = cardano_network::protocols::blockfetch::Point::new(
            u64::from_be_bytes(data[40..48].try_into().unwrap_or([0; 8])),
            &data[48..80],
        );

        // Both points should be valid or return errors gracefully
        let _ = (point1, point2);
    }
});
