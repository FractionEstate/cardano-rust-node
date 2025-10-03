//! Ouroboros Consensus Protocol Tests
//!
//! Tests for the Ouroboros proof-of-stake consensus protocol.
//! Covers slot leadership, VRF proofs, block validation, epoch transitions,
//! and stake distribution calculations following the Ouroboros protocol specification.

use cardano_consensus::{ConsensusError, Result};
use cardano_crypto::{
    Blake2b256Hash,
    Ed25519KeyHash,
    VrfOutput,
    VrfProof,
};
use std::collections::HashMap;

#[path = "../common/mod.rs"]
mod common;

use common::vrf::{
    vrf_fixture_output,
    vrf_fixture_proof,
    vrf_prove_message,
};

/// Slot number in the blockchain
pub type SlotNumber = u64;

/// Epoch number
pub type EpochNumber = u64;

/// Relative slot within an epoch
pub type EpochSlot = u32;

/// Stake amount in lovelace
pub type Stake = u64;

/// Block header for Ouroboros consensus
#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub slot: SlotNumber,
    pub prev_hash: Blake2b256Hash,
    pub issuer_vkey: Ed25519KeyHash,
    pub vrf_proof: VrfProof,
    pub vrf_output: VrfOutput,
    pub block_body_hash: Blake2b256Hash,
    pub block_size: u32,
    pub operational_cert: OperationalCertificate,
    pub protocol_magic: u32,
}

/// Operational certificate for block production
#[derive(Debug, Clone)]
pub struct OperationalCertificate {
    pub hot_vkey: Ed25519KeyHash,
    pub sequence_number: u64,
    pub kes_period: u64,
    pub sigma: Blake2b256Hash, // Signature from cold key
}

/// VRF evaluation context
#[derive(Debug, Clone)]
pub struct VrfContext {
    pub epoch_nonce: Blake2b256Hash,
    pub slot: SlotNumber,
    pub extra_entropy: Option<Blake2b256Hash>,
}

/// Stake distribution snapshot
#[derive(Debug, Clone)]
pub struct StakeDistribution {
    pub pools: HashMap<Ed25519KeyHash, PoolStake>,
    pub total_stake: Stake,
    pub snapshot_epoch: EpochNumber,
}

/// Individual stake pool information
#[derive(Debug, Clone)]
pub struct PoolStake {
    pub stake: Stake,
    pub vrf_key: Blake2b256Hash,
    pub pool_params: PoolParams,
}

/// Pool parameters
#[derive(Debug, Clone)]
pub struct PoolParams {
    pub pledge: Stake,
    pub cost: u64,
    pub margin: Rational,
    pub reward_account: Blake2b256Hash,
}

/// Rational number for pool margin
#[derive(Debug, Clone)]
pub struct Rational {
    pub numerator: u64,
    pub denominator: u64,
}

/// Consensus state for protocol validation
#[derive(Debug, Clone)]
pub struct ConsensusState {
    pub current_epoch: EpochNumber,
    pub current_slot: SlotNumber,
    pub epoch_length: u32, // Slots per epoch
    pub active_slot_coeff: f64, // Active slot coefficient (f)
    pub stake_distribution: StakeDistribution,
    pub epoch_nonce: Blake2b256Hash,
    pub epoch_boundary_blocks: HashMap<EpochNumber, Blake2b256Hash>,
    pub security_param: u32, // k parameter
}

/// Slot leadership test result
#[derive(Debug, Clone)]
pub struct SlotLeadershipTest {
    pub is_leader: bool,
    pub vrf_proof: VrfProof,
    pub vrf_output: VrfOutput,
}

impl Rational {
    pub fn new(numerator: u64, denominator: u64) -> Result<Self> {
        if denominator == 0 {
            return Err(ConsensusError::InvalidRational("Denominator cannot be zero".to_string()));
        }
        Ok(Self { numerator, denominator })
    }

    pub fn to_f64(&self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }
}

impl BlockHeader {
    /// Validate block header according to Ouroboros rules
    pub fn validate(&self, consensus_state: &ConsensusState) -> Result<()> {
        // Basic slot validation
        if self.slot < consensus_state.current_slot {
            return Err(ConsensusError::InvalidSlot("Block from past slot".to_string()));
        }

        // Check if issuer is eligible for this slot
        let eligibility = self.check_slot_leadership(consensus_state)?;
        if !eligibility.is_leader {
            return Err(ConsensusError::NotSlotLeader("Pool not elected for this slot".to_string()));
        }

        // Validate VRF proof
        self.validate_vrf_proof(consensus_state)?;

        // Validate operational certificate
        self.validate_operational_certificate(consensus_state)?;

        // Check protocol magic
        if self.protocol_magic != 764824073 { // Mainnet magic
            return Err(ConsensusError::InvalidProtocolMagic("Wrong network".to_string()));
        }

        Ok(())
    }

    fn check_slot_leadership(&self, consensus_state: &ConsensusState) -> Result<SlotLeadershipTest> {
        // Get pool stake
        let pool_stake = consensus_state.stake_distribution.pools
            .get(&self.issuer_vkey)
            .ok_or_else(|| ConsensusError::PoolNotFound("Pool not in stake distribution".to_string()))?;

        // Calculate relative stake (sigma)
        let sigma = pool_stake.stake as f64 / consensus_state.stake_distribution.total_stake as f64;

        // Create VRF context for slot leadership
        let vrf_context = VrfContext {
            epoch_nonce: consensus_state.epoch_nonce,
            slot: self.slot,
            extra_entropy: None,
        };

        // Evaluate VRF for slot leadership
        let vrf_output = self.evaluate_vrf(&vrf_context, &pool_stake.vrf_key)?;

        // Convert VRF output to unit interval [0,1)
        let y = self.vrf_output_to_natural(&vrf_output);

        // Check if leader: y < 1 - (1-f)^sigma
        // Where f is the active slot coefficient
        let threshold = 1.0 - (1.0 - consensus_state.active_slot_coeff).powf(sigma);
        let is_leader = y < threshold;

        Ok(SlotLeadershipTest {
            is_leader,
            vrf_proof: self.vrf_proof.clone(),
            vrf_output,
        })
    }

    fn evaluate_vrf(&self, context: &VrfContext, _vrf_key: &Blake2b256Hash) -> Result<VrfOutput> {
        let extra_capacity = context.extra_entropy.as_ref().map(|_| 32).unwrap_or(0);
        let mut payload = Vec::with_capacity(
            context.epoch_nonce.as_bytes().len()
                + std::mem::size_of::<SlotNumber>()
                + extra_capacity,
        );

        payload.extend_from_slice(context.epoch_nonce.as_bytes());
        payload.extend_from_slice(&context.slot.to_le_bytes());

        if let Some(extra) = context.extra_entropy.as_ref() {
            payload.extend_from_slice(extra.as_bytes());
        }

        let (output, proof) = vrf_prove_message(&payload);

        if self.vrf_proof != proof {
            return Err(ConsensusError::InvalidVrfProof(
                "VRF proof does not match payload".to_string(),
            ));
        }

        if self.vrf_output != output {
            return Err(ConsensusError::InvalidVrfProof(
                "VRF output does not match payload".to_string(),
            ));
        }

        Ok(output)
    }

    fn vrf_output_to_natural(&self, vrf_output: &VrfOutput) -> f64 {
        // Convert VRF output bytes to natural number in [0,1)
        let bytes = vrf_output.as_bytes();
        let mut value = 0u64;

        // Take first 8 bytes and convert to u64
        for (i, &byte) in bytes.iter().take(8).enumerate() {
            value |= (byte as u64) << (i * 8);
        }

        // Normalize to [0,1)
        value as f64 / (u64::MAX as f64)
    }

    fn validate_vrf_proof(&self, _consensus_state: &ConsensusState) -> Result<()> {
        // Verify VRF proof is valid for the given input
        // This involves cryptographic verification of the VRF proof

        if self.vrf_proof.as_bytes().is_empty() {
            return Err(ConsensusError::InvalidVrfProof("Empty VRF proof".to_string()));
        }

        // In practice: verify_vrf_proof(public_key, input, proof, output)
        Ok(())
    }

    fn validate_operational_certificate(&self, consensus_state: &ConsensusState) -> Result<()> {
        // Check operational certificate validity

        // KES period validation
        let current_kes_period = consensus_state.current_slot / 129600; // ~36 hours per KES period
        if self.operational_cert.kes_period != current_kes_period {
            return Err(ConsensusError::InvalidOperationalCert("Wrong KES period".to_string()));
        }

        // Sequence number should be monotonically increasing
        // (This requires checking against previous certificates from same pool)

        // Verify signature from cold key
        if self.operational_cert.sigma.as_bytes().is_empty() {
            return Err(ConsensusError::InvalidOperationalCert("Invalid cold key signature".to_string()));
        }

        Ok(())
    }

    pub fn hash(&self) -> Blake2b256Hash {
        Blake2b256Hash::new(&format!("{:?}", self).as_bytes())
    }
}

impl StakeDistribution {
    /// Create new stake distribution from pool registrations
    pub fn new(epoch: EpochNumber) -> Self {
        Self {
            pools: HashMap::new(),
            total_stake: 0,
            snapshot_epoch: epoch,
        }
    }

    /// Add pool to stake distribution
    pub fn add_pool(&mut self, pool_id: Ed25519KeyHash, pool_stake: PoolStake) {
        self.total_stake += pool_stake.stake;
        self.pools.insert(pool_id, pool_stake);
    }

    /// Remove pool from stake distribution
    pub fn remove_pool(&mut self, pool_id: &Ed25519KeyHash) -> Option<PoolStake> {
        if let Some(pool_stake) = self.pools.remove(pool_id) {
            self.total_stake -= pool_stake.stake;
            Some(pool_stake)
        } else {
            None
        }
    }

    /// Calculate decentralization metric (Nakamoto coefficient)
    pub fn calculate_decentralization(&self) -> u32 {
        if self.pools.is_empty() {
            return 0;
        }

        // Sort pools by stake (descending)
        let mut stakes: Vec<Stake> = self.pools.values().map(|p| p.stake).collect();
        stakes.sort_by(|a, b| b.cmp(a));

        let majority_threshold = self.total_stake / 2;
        let mut cumulative_stake = 0;
        let mut count = 0;

        for stake in stakes {
            cumulative_stake += stake;
            count += 1;
            if cumulative_stake > majority_threshold {
                return count;
            }
        }

        count
    }

    /// Validate stake distribution integrity
    pub fn validate(&self) -> Result<()> {
        let calculated_total: Stake = self.pools.values().map(|p| p.stake).sum();

        if calculated_total != self.total_stake {
            return Err(ConsensusError::InvalidStakeDistribution(
                "Total stake mismatch".to_string()
            ));
        }

        // Check no pool has zero stake
        for (pool_id, pool_stake) in &self.pools {
            if pool_stake.stake == 0 {
                return Err(ConsensusError::InvalidStakeDistribution(
                    format!("Pool {:?} has zero stake", pool_id)
                ));
            }
        }

        Ok(())
    }
}

impl ConsensusState {
    pub fn new() -> Self {
        Self {
            current_epoch: 0,
            current_slot: 0,
            epoch_length: 432000, // 5 days in 1-second slots
            active_slot_coeff: 0.05, // 5% of slots should have blocks
            stake_distribution: StakeDistribution::new(0),
            epoch_nonce: Blake2b256Hash::new(b"genesis_nonce"),
            epoch_boundary_blocks: HashMap::new(),
            security_param: 2160, // k = 2160 slots (~36 hours)
        }
    }

    /// Advance to next slot
    pub fn advance_slot(&mut self) -> Result<()> {
        self.current_slot += 1;

        // Check if we've crossed epoch boundary
        let new_epoch = self.current_slot / self.epoch_length as u64;
        if new_epoch > self.current_epoch {
            self.transition_epoch(new_epoch)?;
        }

        Ok(())
    }

    /// Handle epoch transition
    fn transition_epoch(&mut self, new_epoch: EpochNumber) -> Result<()> {
        let old_epoch = self.current_epoch;
        self.current_epoch = new_epoch;

        // Update epoch nonce (simplified)
        let new_nonce = Blake2b256Hash::new(&format!("epoch_{}", new_epoch).as_bytes());
        self.epoch_nonce = new_nonce;

        // In practice, this would:
        // 1. Take stake distribution snapshot
        // 2. Calculate new epoch nonce from VRF outputs
        // 3. Update protocol parameters if scheduled

        println!("Transitioned from epoch {} to epoch {}", old_epoch, new_epoch);
        Ok(())
    }

    /// Get current epoch from slot number
    pub fn slot_to_epoch(&self, slot: SlotNumber) -> EpochNumber {
        slot / self.epoch_length as u64
    }

    /// Get relative slot within epoch
    pub fn slot_to_epoch_slot(&self, slot: SlotNumber) -> EpochSlot {
        (slot % self.epoch_length as u64) as EpochSlot
    }

    /// Check if we're in the last k slots of an epoch (settlement delay)
    pub fn in_settlement_delay(&self, slot: SlotNumber) -> bool {
        let epoch_slot = self.slot_to_epoch_slot(slot);
        let remaining_slots = self.epoch_length - epoch_slot;
        remaining_slots <= self.security_param
    }
}

/// Epoch boundary calculation utilities
pub struct EpochBoundary;

impl EpochBoundary {
    /// Calculate rewards for an epoch
    pub fn calculate_epoch_rewards(
        stake_distribution: &StakeDistribution,
        total_fees: u64,
        treasury_rate: f64,
        reserve_rate: f64,
    ) -> Result<HashMap<Ed25519KeyHash, u64>> {
        let mut rewards = HashMap::new();

        // Reserve portion goes to treasury and reserves
        let treasury_portion = (total_fees as f64 * treasury_rate) as u64;
        let reserve_portion = (total_fees as f64 * reserve_rate) as u64;
        let distributable = total_fees - treasury_portion - reserve_portion;

        // Distribute remaining fees proportionally to stake
        for (pool_id, pool_stake) in &stake_distribution.pools {
            let pool_reward = (distributable as f64 *
                (pool_stake.stake as f64 / stake_distribution.total_stake as f64)) as u64;

            // Apply pool cost and margin
            let after_cost = if pool_reward > pool_stake.pool_params.cost {
                pool_reward - pool_stake.pool_params.cost
            } else {
                0
            };

            let pool_margin = (after_cost as f64 * pool_stake.pool_params.margin.to_f64()) as u64;
            let delegator_reward = after_cost - pool_margin;

            // Pool gets cost + margin, delegators get the rest
            rewards.insert(*pool_id, pool_stake.pool_params.cost + pool_margin);

            // In practice, delegator rewards would be distributed to individual delegators
            let _ = delegator_reward; // Suppress unused warning
        }

        Ok(rewards)
    }

    /// Update stake distribution for new epoch
    pub fn update_stake_distribution(
        current: &StakeDistribution,
        new_delegations: &HashMap<Ed25519KeyHash, Stake>,
        pool_updates: &HashMap<Ed25519KeyHash, PoolStake>,
    ) -> Result<StakeDistribution> {
        let mut new_distribution = StakeDistribution::new(current.snapshot_epoch + 1);

        // Start with existing pools
        for (pool_id, pool_stake) in &current.pools {
            new_distribution.add_pool(*pool_id, pool_stake.clone());
        }

        // Apply delegation changes
        for (pool_id, additional_stake) in new_delegations {
            if let Some(pool_stake) = new_distribution.pools.get_mut(pool_id) {
                pool_stake.stake += additional_stake;
                new_distribution.total_stake += additional_stake;
            }
        }

        // Apply pool parameter updates
        for (pool_id, updated_stake) in pool_updates {
            new_distribution.pools.insert(*pool_id, updated_stake.clone());
        }

        new_distribution.validate()?;
        Ok(new_distribution)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vrf_pair_for(consensus_state: &ConsensusState, slot: SlotNumber) -> (VrfOutput, VrfProof) {
        let mut payload = Vec::with_capacity(
            consensus_state.epoch_nonce.as_bytes().len() + std::mem::size_of::<SlotNumber>(),
        );
        payload.extend_from_slice(consensus_state.epoch_nonce.as_bytes());
        payload.extend_from_slice(&slot.to_le_bytes());

        super::vrf_prove_message(&payload)
    }

    fn create_test_pool_stake() -> PoolStake {
        PoolStake {
            stake: 1_000_000_000_000, // 1M ADA
            vrf_key: Blake2b256Hash::new(b"test_vrf_key"),
            pool_params: PoolParams {
                pledge: 100_000_000_000, // 100K ADA
                cost: 340_000_000, // 340 ADA minimum cost
                margin: Rational::new(3, 100).unwrap(), // 3% margin
                reward_account: Blake2b256Hash::new(b"reward_account"),
            },
        }
    }

    #[test]
    fn test_slot_to_epoch_conversion() {
        let consensus_state = ConsensusState::new();

        assert_eq!(consensus_state.slot_to_epoch(0), 0);
        assert_eq!(consensus_state.slot_to_epoch(431999), 0);
        assert_eq!(consensus_state.slot_to_epoch(432000), 1);
        assert_eq!(consensus_state.slot_to_epoch(864000), 2);

        assert_eq!(consensus_state.slot_to_epoch_slot(0), 0);
        assert_eq!(consensus_state.slot_to_epoch_slot(100), 100);
        assert_eq!(consensus_state.slot_to_epoch_slot(432000), 0);
        assert_eq!(consensus_state.slot_to_epoch_slot(432100), 100);
    }

    #[test]
    fn test_stake_distribution_validation() {
        let mut distribution = StakeDistribution::new(1);

        let pool1_id = Ed25519KeyHash::new(b"pool1");
        let pool1_stake = create_test_pool_stake();
        distribution.add_pool(pool1_id, pool1_stake);

        let pool2_id = Ed25519KeyHash::new(b"pool2");
        let mut pool2_stake = create_test_pool_stake();
        pool2_stake.stake = 500_000_000_000; // 500K ADA
        distribution.add_pool(pool2_id, pool2_stake);

        assert_eq!(distribution.total_stake, 1_500_000_000_000);
        assert!(distribution.validate().is_ok());

        // Test removal
        let removed = distribution.remove_pool(&pool1_id);
        assert!(removed.is_some());
        assert_eq!(distribution.total_stake, 500_000_000_000);
        assert!(distribution.validate().is_ok());
    }

    #[test]
    fn test_decentralization_metric() {
        let mut distribution = StakeDistribution::new(1);

        // Create 5 pools with different stakes
        let stakes = vec![
            1_000_000_000_000, // 1M ADA (largest)
            800_000_000_000,   // 800K ADA
            600_000_000_000,   // 600K ADA
            400_000_000_000,   // 400K ADA
            200_000_000_000,   // 200K ADA
        ];

        for (i, stake) in stakes.iter().enumerate() {
            let pool_id = Ed25519KeyHash::new(&format!("pool{}", i).as_bytes());
            let mut pool_stake = create_test_pool_stake();
            pool_stake.stake = *stake;
            distribution.add_pool(pool_id, pool_stake);
        }

        // Total: 3M ADA, majority threshold: 1.5M ADA
        // Pool 0 (1M) + Pool 1 (800K) = 1.8M ADA > 1.5M ADA
        // So Nakamoto coefficient should be 2
        let nakamoto_coeff = distribution.calculate_decentralization();
        assert_eq!(nakamoto_coeff, 2);
    }

    #[test]
    fn test_block_header_validation() {
        let mut consensus_state = ConsensusState::new();
        consensus_state.active_slot_coeff = 1.0; // deterministic leadership for tests

        // Add a test pool to stake distribution
        let pool_id = Ed25519KeyHash::new(b"test_pool");
        let pool_stake = create_test_pool_stake();
        consensus_state.stake_distribution.add_pool(pool_id, pool_stake);

        let target_slot = 1000;
        let (vrf_output, vrf_proof) = vrf_pair_for(&consensus_state, target_slot);

        let block_header = BlockHeader {
            slot: target_slot,
            prev_hash: Blake2b256Hash::new(b"prev_block_hash"),
            issuer_vkey: pool_id,
            vrf_proof,
            vrf_output,
            block_body_hash: Blake2b256Hash::new(b"block_body"),
            block_size: 65536,
            operational_cert: OperationalCertificate {
                hot_vkey: Ed25519KeyHash::new(b"hot_key"),
                sequence_number: 1,
                kes_period: 0, // Will be calculated based on slot
                sigma: Blake2b256Hash::new(b"cold_signature"),
            },
            protocol_magic: 764824073, // Mainnet magic
        };

        consensus_state.current_slot = target_slot - 1; // Block is from next slot

        let result = block_header.validate(&consensus_state);
        assert!(result.is_ok(), "block header should validate with matching VRF payload: {:?}", result);
    }

    #[test]
    fn test_past_slot_rejection() {
        let mut consensus_state = ConsensusState::new();
        consensus_state.current_slot = 1000;

        let pool_id = Ed25519KeyHash::new(b"test_pool");
        let pool_stake = create_test_pool_stake();
        consensus_state.stake_distribution.add_pool(pool_id, pool_stake);

        let past_slot = 999;
        let (vrf_output, vrf_proof) = vrf_pair_for(&consensus_state, past_slot);

        let block_header = BlockHeader {
            slot: past_slot, // Past slot
            prev_hash: Blake2b256Hash::new(b"prev_block_hash"),
            issuer_vkey: pool_id,
            vrf_proof,
            vrf_output,
            block_body_hash: Blake2b256Hash::new(b"block_body"),
            block_size: 65536,
            operational_cert: OperationalCertificate {
                hot_vkey: Ed25519KeyHash::new(b"hot_key"),
                sequence_number: 1,
                kes_period: 0,
                sigma: Blake2b256Hash::new(b"cold_signature"),
            },
            protocol_magic: 764824073,
        };

        let result = block_header.validate(&consensus_state);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConsensusError::InvalidSlot(_)));
    }

    #[test]
    fn test_epoch_transition() {
        let mut consensus_state = ConsensusState::new();

        // Start at slot near epoch boundary
        consensus_state.current_slot = 431999; // Last slot of epoch 0
        assert_eq!(consensus_state.current_epoch, 0);

        // Advance one slot to trigger epoch transition
        let result = consensus_state.advance_slot();
        assert!(result.is_ok());
        assert_eq!(consensus_state.current_slot, 432000);
        assert_eq!(consensus_state.current_epoch, 1);
    }

    #[test]
    fn test_settlement_delay() {
        let consensus_state = ConsensusState::new();

        // Test slots near epoch boundary (k = 2160)
        assert!(!consensus_state.in_settlement_delay(429839)); // 2161 slots remaining
        assert!(consensus_state.in_settlement_delay(429840));  // 2160 slots remaining
        assert!(consensus_state.in_settlement_delay(431999));  // 1 slot remaining
        assert!(!consensus_state.in_settlement_delay(432000)); // New epoch starts
    }

    #[test]
    fn test_epoch_reward_calculation() {
        let mut distribution = StakeDistribution::new(1);

        let pool1_id = Ed25519KeyHash::new(b"pool1");
        let pool1_stake = create_test_pool_stake(); // 1M ADA stake
        distribution.add_pool(pool1_id, pool1_stake);

        let pool2_id = Ed25519KeyHash::new(b"pool2");
        let mut pool2_stake = create_test_pool_stake();
        pool2_stake.stake = 500_000_000_000; // 500K ADA stake
        distribution.add_pool(pool2_id, pool2_stake);

        // Calculate rewards for 1000 ADA in fees
        let total_fees = 1000_000_000; // 1000 ADA
        let treasury_rate = 0.2; // 20% to treasury
        let reserve_rate = 0.0;  // 0% to reserves

        let rewards = EpochBoundary::calculate_epoch_rewards(
            &distribution,
            total_fees,
            treasury_rate,
            reserve_rate,
        ).unwrap();

        // Available for distribution: 800 ADA (after 20% to treasury)
        // Pool1 should get 2/3 of rewards (1M / 1.5M stake)
        // Pool2 should get 1/3 of rewards (500K / 1.5M stake)

        assert_eq!(rewards.len(), 2);

        let pool1_reward = rewards.get(&pool1_id).unwrap();
        let pool2_reward = rewards.get(&pool2_id).unwrap();

        // Each pool gets their cost + margin on remaining rewards
        // This is a simplified test - actual reward calculation is more complex
        assert!(*pool1_reward > *pool2_reward, "Pool1 should get more rewards due to higher stake");
    }

    #[test]
    fn test_rational_operations() {
        let r1 = Rational::new(3, 100).unwrap(); // 3%
        let r2 = Rational::new(5, 100).unwrap(); // 5%

        assert!((r1.to_f64() - 0.03).abs() < f64::EPSILON);
        assert!((r2.to_f64() - 0.05).abs() < f64::EPSILON);

        // Test invalid rational
        let invalid = Rational::new(1, 0);
        assert!(invalid.is_err());
        assert!(matches!(invalid.unwrap_err(), ConsensusError::InvalidRational(_)));
    }

    #[test]
    fn test_vrf_output_conversion() {
        let header = BlockHeader {
            slot: 1000,
            prev_hash: Blake2b256Hash::new(b"prev_hash"),
            issuer_vkey: Ed25519KeyHash::new(b"issuer"),
            vrf_proof: vrf_fixture_proof("ouroboros-conversion-proof"),
            vrf_output: vrf_fixture_output("ouroboros-conversion-output"),
            block_body_hash: Blake2b256Hash::new(b"body_hash"),
            block_size: 1000,
            operational_cert: OperationalCertificate {
                hot_vkey: Ed25519KeyHash::new(b"hot_key"),
                sequence_number: 1,
                kes_period: 0,
                sigma: Blake2b256Hash::new(b"signature"),
            },
            protocol_magic: 764824073,
        };

        let natural = header.vrf_output_to_natural(&header.vrf_output);
        assert!(natural >= 0.0 && natural < 1.0, "VRF output should be in [0,1)");
    }

    #[test]
    fn test_wrong_protocol_magic() {
        let mut consensus_state = ConsensusState::new();
        let pool_id = Ed25519KeyHash::new(b"test_pool");
        consensus_state.stake_distribution.add_pool(pool_id, create_test_pool_stake());

        let slot = 1000;
        let (vrf_output, vrf_proof) = vrf_pair_for(&consensus_state, slot);

        let block_header = BlockHeader {
            slot,
            prev_hash: Blake2b256Hash::new(b"prev_hash"),
            issuer_vkey: pool_id,
            vrf_proof,
            vrf_output,
            block_body_hash: Blake2b256Hash::new(b"body"),
            block_size: 1000,
            operational_cert: OperationalCertificate {
                hot_vkey: Ed25519KeyHash::new(b"hot_key"),
                sequence_number: 1,
                kes_period: 0,
                sigma: Blake2b256Hash::new(b"signature"),
            },
            protocol_magic: 12345, // Wrong magic number
        };

        consensus_state.current_slot = 999;
        let result = block_header.validate(&consensus_state);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConsensusError::InvalidProtocolMagic(_)));
    }
}
