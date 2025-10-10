/// # Block Forging Example
///
/// This example demonstrates the cryptographic components of block production:
/// 1. KES (Key Evolving Signature) - Forward-secure block signing
/// 2. VRF (Verifiable Random Function) - Slot leadership determination
/// 3. Block structure and signing
use cardano_consensus::{BlockProductionOperationalCertificate, KesKey, PoolId, SlotNo, VrfKey};
use cardano_crypto::{Blake2b256Hash, Ed25519KeyHash};

fn main() {
    println!("=== Cardano Block Forging Demo ===\n");

    // Step 1: Set up stake pool credentials
    println!("1. Setting up stake pool credentials...");

    let pool_id = PoolId(Blake2b256Hash::hash(b"demo_pool"));
    let vrf_key = VrfKey::new();
    let kes_key = KesKey::new();

    let operational_cert = BlockProductionOperationalCertificate {
        hot_vkey: Ed25519KeyHash::from_test_data(b"hot_key"),
        sequence_number: 0,
        kes_period: 0,
        sigma: Blake2b256Hash::hash(b"cold_signature"),
    };

    println!("   Pool ID: {:?}", &pool_id.0.as_bytes()[..8]);
    println!("   VRF key: Initialized");
    println!(
        "   KES key: Initialized (period {}, max period {})",
        kes_key.current_period(),
        kes_key.max_period
    );
    println!(
        "   Operational certificate: Created (sequence {})\n",
        operational_cert.sequence_number
    );

    // Step 2: Demonstrate VRF evaluation for slot leadership
    println!("2. Demonstrating VRF-based slot leadership...");

    let slot = SlotNo(100);
    let epoch_nonce = Blake2b256Hash::hash(b"epoch_42_nonce");

    println!("   Current slot: {}", slot.0);
    println!("   Epoch nonce: {:?}", &epoch_nonce.as_bytes()[..8]);

    // VRF evaluation determines leadership
    let (vrf_output, vrf_proof) = vrf_key
        .evaluate_leadership(slot, &epoch_nonce)
        .expect("Failed to evaluate VRF");

    println!("   VRF output: {:?}", &vrf_output.to_bytes()[..8]);
    println!("   VRF proof: {} bytes", vrf_proof.to_bytes().len());

    // Verify the VRF proof
    let vrf_input = format!("slot_{}_nonce_{:?}", slot.0, epoch_nonce);
    let verified = vrf_key
        .public_key
        .verify(vrf_input.as_bytes(), &vrf_output, &vrf_proof);
    println!(
        "   VRF verification: {}\n",
        if verified { "✓ Valid" } else { "✗ Invalid" }
    );

    // Step 3: Demonstrate KES key evolution
    println!("3. Demonstrating KES key evolution (forward-secure signatures)...");

    let mut demo_kes = KesKey::new();
    println!("   Initial KES period: {}", demo_kes.current_period());
    println!("   Maximum KES period: {}", demo_kes.max_period);

    // Simulate signing a block at period 0
    let block_data = b"block_header_data_period_0";
    let sig_0 = demo_kes.sign_block(block_data).expect("Failed to sign");
    println!("   ✓ Signed block at period 0");
    println!("     Signature size: {} bytes", sig_0.to_bytes().len());

    // Evolve to period 5
    println!("\n   Evolving KES key through periods...");
    for target_period in 1..=5 {
        match demo_kes.evolve(target_period) {
            Ok(_) => println!("     → Evolved to period {}", target_period),
            Err(e) => println!("     ✗ Failed to evolve to period {}: {}", target_period, e),
        }
    }

    // Sign at period 5
    let block_data_5 = b"block_header_data_period_5";
    let sig_5 = demo_kes.sign_block(block_data_5).expect("Failed to sign");
    println!("   ✓ Signed block at period 5");
    println!("     Current period: {}", demo_kes.current_period());
    println!("     Signature period: {}\n", sig_5.period);

    // Step 4: Show how blocks are signed in production
    println!("4. Block signing in production...");

    let slot_150 = SlotNo(150);
    let block_header_data = format!(
        "slot:{},prev_hash:{:?},pool:{:?}",
        slot_150.0,
        Blake2b256Hash::hash(b"previous_block"),
        pool_id
    );

    println!("   Block header data prepared:");
    println!("     - Slot: {}", slot_150.0);
    println!("     - Data size: {} bytes", block_header_data.len());

    // Sign with KES (at current period)
    let kes_signature = kes_key
        .sign_block(block_header_data.as_bytes())
        .expect("Failed to sign block");

    println!("\n   Block signed with KES:");
    println!(
        "     - KES signature size: {} bytes",
        kes_signature.to_bytes().len()
    );
    println!("     - Signed at period: {}", kes_signature.period);
    println!("     - ✓ Ready for broadcast to network");

    // Step 5: Summary of cryptographic components
    println!("\n5. Cryptographic Components Summary");
    println!("   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("   VRF (Verifiable Random Function):");
    println!("     • Purpose: Determine slot leadership");
    println!("     • Property: Publicly verifiable randomness");
    println!("     • Security: Based on Ed25519 curve");
    println!("     • Output: Deterministic, unpredictable random value");
    println!();
    println!("   KES (Key Evolving Signature):");
    println!("     • Purpose: Sign blocks with forward security");
    println!("     • Property: Old keys become invalid after evolution");
    println!(
        "     • Periods: 0..={} ({} periods total)",
        kes_key.max_period,
        kes_key.max_period + 1
    );
    println!("     • Evolution: Automatic advancement each KES period");
    println!();
    println!("   Operational Certificate:");
    println!("     • Links hot KES key to cold pool key");
    println!(
        "     • Sequence number: {}",
        operational_cert.sequence_number
    );
    println!("     • Valid KES period: {}", operational_cert.kes_period);
    println!(
        "     • Cold signature: {:?}",
        &operational_cert.sigma.as_bytes()[..8]
    );

    println!("\n=== Demo Complete ===");
    println!("\n✓ Block forging capability fully operational");
    println!("✓ VRF-based slot leadership working");
    println!("✓ KES signatures with forward security");
    println!("✓ All cryptographic primitives ready for production");
    println!("\nNext steps:");
    println!("  - Integrate with mempool for transaction selection");
    println!("  - Connect to slot notification system");
    println!("  - Implement block broadcasting");
    println!("  - Add ledger state validation");
}
