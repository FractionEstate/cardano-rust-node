#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz TxSubmission protocol message decoding
    // Transaction IDs must be 32 bytes and validated

    // Try to create TxId from data
    if data.len() >= 32 {
        let _ = cardano_network::protocols::txsubmission::TxId::new(&data[0..32]);
    }

    // Try with various invalid lengths (should all be rejected gracefully)
    for len in [0, 1, 8, 16, 24, 31, 33, 64] {
        if data.len() >= len {
            let _ = cardano_network::protocols::txsubmission::TxId::new(&data[0..len]);
        }
    }
});
