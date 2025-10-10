#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz ChainSync protocol message decoding
    // ChainSync messages contain block headers, points, and tips
    // All must handle malformed CBOR gracefully

    // Try to decode as Point (slot + hash)
    if data.len() >= 40 {
        let _ = cardano_network::protocols::chainsync::Point::new(
            u64::from_be_bytes(data[0..8].try_into().unwrap_or([0; 8])),
            &data[8..40],
        );
    }

    // Try to decode as Tip (point + block number)
    if data.len() >= 48 {
        let point_result = cardano_network::protocols::chainsync::Point::new(
            u64::from_be_bytes(data[0..8].try_into().unwrap_or([0; 8])),
            &data[8..40],
        );

        if let Ok(point) = point_result {
            let block_no = u64::from_be_bytes(data[40..48].try_into().unwrap_or([0; 8]));
            let _ = cardano_network::protocols::chainsync::Tip::new(
                point.slot.0,
                block_no,
                point.hash.as_bytes(),
            );
        }
    }
});
