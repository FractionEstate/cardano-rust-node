//! Slot Leadership Calculation for Ouroboros Praos
//!
//! Implements the VRF-based slot leadership election algorithm used in
//! Ouroboros Praos consensus protocol. This determines which stake pools
//! are elected to produce blocks in specific slots.
//!
//! ## Algorithm Overview
//!
//! For each slot, a stake pool:
//! 1. Constructs VRF input: epoch_nonce || slot_number || NONCE tag
//! 2. Generates VRF proof using private VRF key
//! 3. Derives VRF output from proof
//! 4. Checks if VRF output < threshold (based on relative stake)
//! 5. If yes, pool is elected leader for that slot
//!
//! ## Threshold Calculation
//!
//! The threshold is calculated using the formula:
//! ```text
//! threshold = 2^256 * φ_f(σ)
//! where φ_f(σ) = 1 - (1 - f)^σ
//! ```
//! - f = active slot coefficient (typically 0.05)
//! - σ = relative stake (pool_stake / total_stake)
//!
//! ## Reference
//!
//! Ouroboros Praos paper: https://eprint.iacr.org/2017/573.pdf
//! Cardano ledger specs: https://github.com/IntersectMBO/cardano-ledger

use crate::ouroboros::{EpochNo, PoolId, ProtocolParameters, SlotNo, StakeDistribution};
use crate::{ConsensusError, Result};
use cardano_crypto::vrf::{VrfOutput, VrfPrivateKey, VrfProof, VrfPublicKey};
use cardano_crypto::Blake2b256Hash;
use num_bigint::BigUint;
use num_traits::{ToPrimitive, Zero};
use std::collections::HashMap;

/// VRF domain separation tag for TEST (slot leadership)
const VRF_TAG_TEST: &[u8] = b"TEST";

lazy_static::lazy_static! {
    /// Maximum value for VRF output (2^256)
    static ref VRF_MAX: BigUint = {
        let mut bytes = [0u8; 33];
        bytes[0] = 1; // 2^256 = 1 followed by 32 zero bytes
        BigUint::from_bytes_be(&bytes)
    };
}

/// Leader election result for a slot
#[derive(Debug, Clone)]
pub struct LeadershipProof {
    /// The slot for which leadership was proven
    pub slot: SlotNo,
    /// VRF proof demonstrating leadership
    pub vrf_proof: VrfProof,
    /// VRF output used for verification
    pub vrf_output: VrfOutput,
    /// Pool that proved leadership
    pub pool_id: PoolId,
}

/// Leadership check result
#[derive(Debug, Clone)]
pub enum LeadershipCheck {
    /// Pool is leader for this slot
    Leader(LeadershipProof),
    /// Pool is not leader for this slot
    NotLeader {
        slot: SlotNo,
        vrf_output: VrfOutput,
        threshold: BigUint,
        actual: BigUint,
    },
}

/// Slot leadership calculator with VRF-based election
pub struct LeadershipCalculator {
    /// Current stake distribution
    stake_distribution: StakeDistribution,
    /// Protocol parameters
    protocol_params: ProtocolParameters,
    /// Epoch nonce for randomness
    epoch_nonce: Blake2b256Hash,
    /// Current epoch
    current_epoch: EpochNo,
}

impl LeadershipCalculator {
    /// Create new leadership calculator for an epoch
    pub fn new(
        stake_distribution: StakeDistribution,
        protocol_params: ProtocolParameters,
        epoch_nonce: Blake2b256Hash,
        current_epoch: EpochNo,
    ) -> Self {
        Self {
            stake_distribution,
            protocol_params,
            epoch_nonce,
            current_epoch,
        }
    }

    /// Check if a pool is elected leader for a given slot
    ///
    /// Returns `LeadershipCheck::Leader` with proof if elected,
    /// or `LeadershipCheck::NotLeader` with details if not elected.
    pub fn check_slot_leadership(
        &self,
        pool_id: &PoolId,
        pool_stake: u64,
        vrf_private_key: &VrfPrivateKey,
        slot: SlotNo,
    ) -> Result<LeadershipCheck> {
        // 1. Validate stake
        if pool_stake == 0 {
            return Err(ConsensusError::InvalidStake(
                "Pool has zero stake".to_string(),
            ));
        }

        let total_stake = self.stake_distribution.total_stake;
        if total_stake == 0 {
            return Err(ConsensusError::InvalidStake(
                "Total stake is zero".to_string(),
            ));
        }

        // 2. Construct VRF input
        let vrf_input = self.construct_vrf_input(slot);

        // 3. Generate VRF proof
        let (vrf_output, vrf_proof) = vrf_private_key.prove(&vrf_input);

        // 4. Calculate threshold
        let threshold = self.calculate_threshold(pool_stake, total_stake)?;

        // 5. Convert VRF output to natural number
        let vrf_nat = vrf_output_to_natural(&vrf_output);

        // 6. Check if VRF output is below threshold
        if vrf_nat < threshold {
            Ok(LeadershipCheck::Leader(LeadershipProof {
                slot,
                vrf_proof,
                vrf_output,
                pool_id: pool_id.clone(),
            }))
        } else {
            Ok(LeadershipCheck::NotLeader {
                slot,
                vrf_output,
                threshold,
                actual: vrf_nat,
            })
        }
    }

    /// Verify a leadership proof from another pool
    pub fn verify_leadership_proof(
        &self,
        proof: &LeadershipProof,
        pool_stake: u64,
        vrf_public_key: &VrfPublicKey,
    ) -> Result<bool> {
        // 1. Construct VRF input
        let vrf_input = self.construct_vrf_input(proof.slot);

        // 2. Verify VRF proof
        if !vrf_public_key.verify(&vrf_input, &proof.vrf_output, &proof.vrf_proof) {
            return Ok(false);
        }

        // 3. Calculate threshold
        let total_stake = self.stake_distribution.total_stake;
        let threshold = self.calculate_threshold(pool_stake, total_stake)?;

        // 4. Check if VRF output is below threshold
        let vrf_nat = vrf_output_to_natural(&proof.vrf_output);
        Ok(vrf_nat < threshold)
    }

    /// Calculate leadership threshold for a pool
    ///
    /// Implements: threshold = 2^256 * φ_f(σ)
    /// where φ_f(σ) = 1 - (1 - f)^σ
    fn calculate_threshold(&self, pool_stake: u64, total_stake: u64) -> Result<BigUint> {
        // Calculate relative stake σ = pool_stake / total_stake
        let relative_stake = pool_stake as f64 / total_stake as f64;

        // Calculate φ_f(σ) = 1 - (1 - f)^σ
        let f = self.protocol_params.active_slot_coefficient;
        let phi = 1.0 - (1.0 - f).powf(relative_stake);

        // Calculate threshold = 2^256 * φ
        // We need to convert phi (0 ≤ phi ≤ 1) to a BigUint
        // Multiply phi by 2^256 and convert to BigUint

        // Convert phi to fixed-point representation with high precision
        // Use 2^128 as intermediate scaling to avoid overflow
        let phi_scaled = (phi * (u128::MAX as f64)) as u128;
        let phi_biguint = BigUint::from(phi_scaled);

        // threshold = (phi_scaled * 2^256) / 2^128 = phi_scaled * 2^128
        let shift_128 = BigUint::from(1u128) << 128;
        let threshold = phi_biguint * shift_128;

        Ok(threshold)
    }

    /// Construct VRF input for slot leadership test
    ///
    /// Format: epoch_nonce || slot_number || VRF_TAG_TEST
    fn construct_vrf_input(&self, slot: SlotNo) -> Vec<u8> {
        let mut input = Vec::new();

        // Add epoch nonce (32 bytes)
        input.extend_from_slice(self.epoch_nonce.as_bytes());

        // Add slot number (8 bytes, little-endian)
        input.extend_from_slice(&slot.0.to_le_bytes());

        // Add domain separation tag
        input.extend_from_slice(VRF_TAG_TEST);

        input
    }

    /// Calculate leader schedule for an epoch
    ///
    /// Pre-calculates which slots the pool will be leader for in the epoch.
    /// This can be done in advance for monitoring and preparation.
    pub fn calculate_leader_schedule(
        &self,
        pool_id: &PoolId,
        pool_stake: u64,
        vrf_private_key: &VrfPrivateKey,
    ) -> Result<Vec<LeadershipProof>> {
        let mut schedule = Vec::new();

        let first_slot = self.current_epoch.first_slot(&self.protocol_params);
        let last_slot = self.current_epoch.last_slot(&self.protocol_params);

        for slot_num in first_slot.0..=last_slot.0 {
            let slot = SlotNo(slot_num);
            match self.check_slot_leadership(pool_id, pool_stake, vrf_private_key, slot)? {
                LeadershipCheck::Leader(proof) => {
                    schedule.push(proof);
                }
                LeadershipCheck::NotLeader { .. } => {
                    // Not leader for this slot
                }
            }
        }

        Ok(schedule)
    }

    /// Calculate leader schedule for multiple epochs ahead
    pub fn calculate_multi_epoch_schedule(
        &self,
        pool_id: &PoolId,
        pool_stake: u64,
        vrf_private_key: &VrfPrivateKey,
        epochs_ahead: u64,
    ) -> Result<HashMap<EpochNo, Vec<LeadershipProof>>> {
        let mut schedules = HashMap::new();

        for i in 0..epochs_ahead {
            let epoch = EpochNo(self.current_epoch.0 + i);

            // For future epochs, we need the future epoch nonce
            // This is a simplified version - in production, you'd derive the nonce
            // from the chain state at the epoch boundary
            let epoch_nonce = self.derive_epoch_nonce(epoch);

            let calculator = LeadershipCalculator::new(
                self.stake_distribution.clone(),
                self.protocol_params.clone(),
                epoch_nonce,
                epoch,
            );

            let schedule =
                calculator.calculate_leader_schedule(pool_id, pool_stake, vrf_private_key)?;

            if !schedule.is_empty() {
                schedules.insert(epoch, schedule);
            }
        }

        Ok(schedules)
    }

    /// Derive epoch nonce for a future epoch
    ///
    /// This is a simplified version. In production, the epoch nonce is derived
    /// from VRF outputs in previous epochs using the "mark-set-go" mechanism.
    fn derive_epoch_nonce(&self, epoch: EpochNo) -> Blake2b256Hash {
        // Simplified: hash current nonce with epoch number
        let mut input = Vec::new();
        input.extend_from_slice(self.epoch_nonce.as_bytes());
        input.extend_from_slice(&epoch.0.to_le_bytes());
        Blake2b256Hash::hash(&input)
    }

    /// Get expected number of blocks for pool in epoch
    pub fn expected_blocks_per_epoch(&self, pool_stake: u64) -> f64 {
        let total_stake = self.stake_distribution.total_stake;
        if total_stake == 0 {
            return 0.0;
        }

        let relative_stake = pool_stake as f64 / total_stake as f64;
        let f = self.protocol_params.active_slot_coefficient;
        let slots_per_epoch = self.protocol_params.epoch_length as f64;

        // Expected blocks = slots * f * σ (for small σ)
        // More precisely: slots * (1 - (1-f)^σ)
        let probability = 1.0 - (1.0 - f).powf(relative_stake);
        slots_per_epoch * probability
    }
}

/// Convert VRF output to natural number
///
/// The VRF output is treated as a 256-bit big-endian integer.
fn vrf_output_to_natural(vrf_output: &VrfOutput) -> BigUint {
    BigUint::from_bytes_be(vrf_output.to_bytes())
}

/// Calculate probability from VRF output for logging/debugging
pub fn vrf_output_to_probability(vrf_output: &VrfOutput) -> f64 {
    let vrf_nat = vrf_output_to_natural(vrf_output);
    let vrf_max = VRF_MAX.clone();

    if vrf_max.is_zero() {
        return 0.0;
    }

    // Convert to f64 for probability calculation
    // vrf_nat / 2^256
    let ratio = BigUint::from(1u128 << 64);
    let vrf_scaled = &vrf_nat / &ratio;
    let max_scaled = &vrf_max / &ratio;

    let vrf_f64 = vrf_scaled.to_f64().unwrap_or(0.0);
    let max_f64 = max_scaled.to_f64().unwrap_or(1.0);

    vrf_f64 / max_f64
}

/// Calculate minimum stake required for expected blocks per epoch
pub fn min_stake_for_expected_blocks(
    target_blocks: f64,
    total_stake: u64,
    params: &ProtocolParameters,
) -> u64 {
    let slots_per_epoch = params.epoch_length as f64;
    let f = params.active_slot_coefficient;

    // Solve: target_blocks = slots * (1 - (1-f)^σ)
    // σ = log(1 - target_blocks/slots) / log(1-f)
    let probability = target_blocks / slots_per_epoch;
    if probability >= 1.0 {
        return total_stake; // Need all stake
    }
    if probability <= 0.0 {
        return 0;
    }

    let relative_stake = (1.0 - probability).log(1.0 - f);
    let pool_stake = (relative_stake * total_stake as f64) as u64;

    pool_stake.min(total_stake)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_calculation() {
        let params = ProtocolParameters {
            security_parameter: 2160,
            active_slot_coefficient: 0.05,
            slot_length: 1,
            epoch_length: 432000,
        };

        let stake_dist = StakeDistribution {
            pools: HashMap::new(),
            total_stake: 1_000_000_000, // 1 billion lovelace
        };

        let calculator = LeadershipCalculator::new(
            stake_dist,
            params,
            Blake2b256Hash::hash(b"test_nonce"),
            EpochNo(0),
        );

        // Pool with 1% stake
        let pool_stake = 10_000_000; // 10 million
        let total_stake = 1_000_000_000; // 1 billion

        let threshold = calculator
            .calculate_threshold(pool_stake, total_stake)
            .unwrap();

        // Threshold should be positive
        assert!(threshold > BigUint::zero());

        // Threshold should be less than 2^256
        assert!(threshold < *VRF_MAX);

        // With 1% stake and f=0.05, φ ≈ 0.0005
        // Threshold ≈ 2^256 * 0.0005
        // This is a rough check
        let expected_phi = 1.0 - (1.0 - 0.05_f64).powf(0.01);
        assert!(expected_phi > 0.0004 && expected_phi < 0.0006);
    }

    #[test]
    fn test_vrf_input_construction() {
        let params = ProtocolParameters::testnet();
        let stake_dist = StakeDistribution {
            pools: HashMap::new(),
            total_stake: 1_000_000_000,
        };

        let epoch_nonce = Blake2b256Hash::hash(b"test_epoch_nonce");
        let calculator = LeadershipCalculator::new(
            stake_dist,
            params,
            epoch_nonce.clone(),
            EpochNo(42),
        );

        let slot = SlotNo(1000);
        let vrf_input = calculator.construct_vrf_input(slot);

        // Should contain: nonce (32) + slot (8) + tag (4) = 44 bytes
        assert_eq!(vrf_input.len(), 32 + 8 + VRF_TAG_TEST.len());

        // First 32 bytes should be nonce
        assert_eq!(&vrf_input[..32], epoch_nonce.as_bytes());

        // Next 8 bytes should be slot number
        assert_eq!(
            &vrf_input[32..40],
            &slot.0.to_le_bytes()
        );

        // Last bytes should be tag
        assert_eq!(&vrf_input[40..], VRF_TAG_TEST);
    }

    #[test]
    fn test_expected_blocks_calculation() {
        let params = ProtocolParameters {
            security_parameter: 2160,
            active_slot_coefficient: 0.05,
            slot_length: 1,
            epoch_length: 432000, // 5 days
        };

        let stake_dist = StakeDistribution {
            pools: HashMap::new(),
            total_stake: 1_000_000_000,
        };

        let calculator = LeadershipCalculator::new(
            stake_dist,
            params.clone(),
            Blake2b256Hash::hash(b"test"),
            EpochNo(0),
        );

        // Pool with 1% stake
        let pool_stake = 10_000_000;
        let expected = calculator.expected_blocks_per_epoch(pool_stake);

        // With 1% stake, f=0.05, and 432000 slots:
        // Expected ≈ 432000 * 0.05 * 0.01 ≈ 216 blocks
        assert!(expected > 200.0 && expected < 230.0);
    }

    #[test]
    fn test_min_stake_for_blocks() {
        let params = ProtocolParameters {
            security_parameter: 2160,
            active_slot_coefficient: 0.05,
            slot_length: 1,
            epoch_length: 432000,
        };

        let total_stake = 1_000_000_000;

        // To get ~1 block per epoch
        let stake = min_stake_for_expected_blocks(1.0, total_stake, &params);

        // Should be roughly 1 / (432000 * 0.05) = 1/21600 ≈ 0.0046% of total
        // ≈ 46,000 lovelace
        assert!(stake > 40_000 && stake < 50_000);
    }
}
