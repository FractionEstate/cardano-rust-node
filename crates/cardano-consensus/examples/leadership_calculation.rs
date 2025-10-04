//! Example: Slot Leadership Calculation
//!
//! This example demonstrates how to:
//! 1. Set up a leadership calculator
//! 2. Check if a pool is leader for specific slots
//! 3. Calculate a full epoch leader schedule
//! 4. Verify leadership proofs from other pools

use cardano_consensus::leadership::{LeadershipCalculator, LeadershipCheck};
use cardano_consensus::ouroboros::{
    EpochNo, PoolId, ProtocolParameters, SlotNo, StakeDistribution,
};
use cardano_crypto::vrf::VrfPrivateKey;
use cardano_crypto::Blake2b256Hash;
use std::collections::HashMap;

fn main() {
    println!("===========================================");
    println!("Cardano Slot Leadership Calculation Example");
    println!("===========================================\n");

    // 1. Set up protocol parameters (Preview testnet)
    let params = ProtocolParameters {
        security_parameter: 432,
        active_slot_coefficient: 0.05,
        slot_length: 1,
        epoch_length: 86400, // 1 day for this example
    };

    println!("Protocol Parameters:");
    println!("  Security parameter (k): {}", params.security_parameter);
    println!(
        "  Active slot coefficient (f): {}",
        params.active_slot_coefficient
    );
    println!("  Epoch length: {} slots", params.epoch_length);
    println!();

    // 2. Set up stake distribution
    let total_stake = 1_000_000_000_000; // 1 million ADA
    let pool_stake = 10_000_000_000; // 10,000 ADA (1% of total)

    let mut pools = HashMap::new();
    let pool_id = PoolId(Blake2b256Hash::hash(b"example_pool"));
    pools.insert(pool_id.clone(), pool_stake);

    let stake_distribution = StakeDistribution {
        pools: pools.clone(),
        total_stake,
    };

    println!("Stake Distribution:");
    println!("  Total stake: {} ADA", total_stake / 1_000_000);
    println!("  Pool stake: {} ADA", pool_stake / 1_000_000);
    println!(
        "  Pool relative stake: {:.2}%",
        (pool_stake as f64 / total_stake as f64) * 100.0
    );
    println!();

    // 3. Generate VRF key for the pool
    let vrf_seed = [42u8; 32]; // Example seed (in production, load from file)
    let vrf_private_key = VrfPrivateKey::generate(&vrf_seed);
    let vrf_public_key = vrf_private_key.public_key();

    println!("VRF Keys:");
    println!(
        "  VRF public key: {}...",
        hex::encode(&vrf_public_key.to_bytes()[..8])
    );
    println!();

    // 4. Create leadership calculator
    let epoch_nonce = Blake2b256Hash::hash(b"example_epoch_nonce");
    let current_epoch = EpochNo(100);

    let calculator = LeadershipCalculator::new(
        stake_distribution.clone(),
        params.clone(),
        epoch_nonce,
        current_epoch,
    );

    println!("Leadership Calculator:");
    println!("  Epoch: {}", current_epoch.0);
    println!(
        "  Epoch nonce: {}...",
        hex::encode(&epoch_nonce.as_bytes()[..8])
    );
    println!();

    // 5. Calculate expected blocks
    let expected_blocks = calculator.expected_blocks_per_epoch(pool_stake);
    println!("Expected Blocks:");
    println!("  Expected blocks per epoch: {:.2}", expected_blocks);
    println!("  Expected blocks per day: {:.2}", expected_blocks);
    println!();

    // 6. Check leadership for specific slots
    println!("Checking Leadership for First 10 Slots:");
    println!("----------------------------------------");

    let first_slot = current_epoch.first_slot(&params);
    let mut leader_count = 0;

    for i in 0..10 {
        let slot = SlotNo(first_slot.0 + i);

        match calculator.check_slot_leadership(&pool_id, pool_stake, &vrf_private_key, slot) {
            Ok(LeadershipCheck::Leader(proof)) => {
                leader_count += 1;
                println!("  Slot {}: ✓ LEADER", slot.0);
                println!(
                    "    VRF output: {}...",
                    hex::encode(&proof.vrf_output.to_bytes()[..8])
                );
            }
            Ok(LeadershipCheck::NotLeader { .. }) => {
                println!("  Slot {}: ✗ Not leader", slot.0);
            }
            Err(e) => {
                println!("  Slot {}: Error: {}", slot.0, e);
            }
        }
    }

    println!("\nLeader in {} out of 10 slots checked\n", leader_count);

    // 7. Calculate full epoch leader schedule (first 1000 slots for demo)
    println!("Calculating Leader Schedule (first 1000 slots)...");

    let mut schedule = Vec::new();
    for i in 0..1000 {
        let slot = SlotNo(first_slot.0 + i);

        if let Ok(LeadershipCheck::Leader(proof)) =
            calculator.check_slot_leadership(&pool_id, pool_stake, &vrf_private_key, slot)
        {
            schedule.push(proof);
        }
    }

    println!("Leader Schedule Results:");
    println!("  Slots checked: 1000");
    println!("  Leader slots found: {}", schedule.len());
    println!(
        "  Leadership rate: {:.2}%",
        (schedule.len() as f64 / 1000.0) * 100.0
    );

    if !schedule.is_empty() {
        println!("\nFirst 5 Leader Slots:");
        for (i, proof) in schedule.iter().take(5).enumerate() {
            println!("  {}. Slot {}", i + 1, proof.slot.0);
        }
    }
    println!();

    // 8. Verify a leadership proof
    if let Some(proof) = schedule.first() {
        println!("Verifying Leadership Proof:");
        println!("  Slot: {}", proof.slot.0);

        match calculator.verify_leadership_proof(proof, pool_stake, &vrf_public_key) {
            Ok(true) => println!("  ✓ Proof verified successfully"),
            Ok(false) => println!("  ✗ Proof verification failed"),
            Err(e) => println!("  Error verifying proof: {}", e),
        }
        println!();
    }

    // 9. Calculate multi-epoch schedule
    println!("Calculating Multi-Epoch Schedule (3 epochs, first 100 slots each)...");

    let mut multi_epoch_stats = Vec::new();
    for epoch_offset in 0..3 {
        let epoch = EpochNo(current_epoch.0 + epoch_offset);
        let epoch_first_slot = epoch.first_slot(&params);

        let mut epoch_leaders = 0;
        for i in 0..100 {
            let slot = SlotNo(epoch_first_slot.0 + i);

            if let Ok(LeadershipCheck::Leader(_)) =
                calculator.check_slot_leadership(&pool_id, pool_stake, &vrf_private_key, slot)
            {
                epoch_leaders += 1;
            }
        }

        multi_epoch_stats.push((epoch.0, epoch_leaders));
    }

    println!("\nMulti-Epoch Results:");
    for (epoch, leaders) in multi_epoch_stats {
        println!("  Epoch {}: {} leaders in first 100 slots", epoch, leaders);
    }
    println!();

    // 10. Stake requirements for target blocks
    println!("Stake Requirements:");
    println!(
        "  For 1 block per epoch: {} ADA",
        cardano_consensus::leadership::min_stake_for_expected_blocks(1.0, total_stake, &params)
            / 1_000_000
    );
    println!(
        "  For 10 blocks per epoch: {} ADA",
        cardano_consensus::leadership::min_stake_for_expected_blocks(10.0, total_stake, &params)
            / 1_000_000
    );
    println!(
        "  For 100 blocks per epoch: {} ADA",
        cardano_consensus::leadership::min_stake_for_expected_blocks(100.0, total_stake, &params)
            / 1_000_000
    );
    println!();

    println!("===========================================");
    println!("Example Complete!");
    println!("===========================================");
}
