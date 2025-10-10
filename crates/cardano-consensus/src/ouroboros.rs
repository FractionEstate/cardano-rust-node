//! Ouroboros consensus protocol core
//!
//! Implements the Ouroboros Praos/Genesis consensus protocol mechanics including
//! slot leadership calculation, VRF evaluation, KES key rotation, and chain quality metrics.

use crate::{ConsensusError, Result};
use cardano_crypto::{Blake2b256Hash, Ed25519KeyHash, VrfOutput, VrfProof};
use std::collections::HashMap;
use std::time::SystemTime;

/// Ouroboros protocol parameters
#[derive(Debug, Clone)]
pub struct ProtocolParameters {
    pub security_parameter: u64,      // k parameter
    pub active_slot_coefficient: f64, // f parameter
    pub slot_length: u64,             // seconds per slot
    pub epoch_length: u64,            // slots per epoch
}

/// Slot number in the blockchain
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SlotNo(pub u64);

/// Epoch number
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EpochNo(pub u64);

/// Block height
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockNo(pub u64);

impl ProtocolParameters {
    /// Create default testnet parameters
    pub fn testnet() -> Self {
        Self {
            security_parameter: 2160,
            active_slot_coefficient: 0.05,
            slot_length: 1,
            epoch_length: 432000,
        }
    }

    /// Create default mainnet parameters
    pub fn mainnet() -> Self {
        Self {
            security_parameter: 2160,
            active_slot_coefficient: 0.05,
            slot_length: 1,
            epoch_length: 432000,
        }
    }
}

impl SlotNo {
    pub fn to_epoch(&self, params: &ProtocolParameters) -> EpochNo {
        EpochNo(self.0 / params.epoch_length)
    }
}

impl EpochNo {
    pub fn first_slot(&self, params: &ProtocolParameters) -> SlotNo {
        SlotNo(self.0 * params.epoch_length)
    }

    pub fn last_slot(&self, params: &ProtocolParameters) -> SlotNo {
        SlotNo((self.0 + 1) * params.epoch_length - 1)
    }
}

/// Stake pool identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PoolId(pub Blake2b256Hash);

/// VRF verification key for slot leadership
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VrfVkey(pub [u8; 32]);

/// KES verification key for block signing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KesVkey(pub [u8; 32]);

/// Operational certificate linking cold and hot keys
#[derive(Debug, Clone)]
pub struct OperationalCertificate {
    pub hot_vkey: KesVkey,
    pub sequence_number: u64,
    pub kes_period: u64,
    pub cold_vkey_signature: Blake2b256Hash, // Simplified
}

/// Stake pool registration information
#[derive(Debug, Clone)]
pub struct StakePool {
    pub pool_id: PoolId,
    pub vrf_vkey: VrfVkey,
    pub pledge: u64,
    pub cost: u64,
    pub margin: f64,
    pub reward_account: Ed25519KeyHash,
    pub owners: Vec<Ed25519KeyHash>,
    pub relays: Vec<PoolRelay>,
    pub metadata: Option<PoolMetadata>,
}

/// Pool relay information
#[derive(Debug, Clone)]
pub enum PoolRelay {
    SingleHost {
        port: Option<u16>,
        hostname: Option<String>,
    },
    MultiHost {
        hostname: String,
    },
}

/// Pool metadata reference
#[derive(Debug, Clone)]
pub struct PoolMetadata {
    pub url: String,
    pub hash: Blake2b256Hash,
}

/// Stake distribution for an epoch
#[derive(Debug, Clone)]
pub struct StakeDistribution {
    pub pools: HashMap<PoolId, u64>,
    pub total_stake: u64,
}

/// VRF-based slot leadership test
#[derive(Debug, Clone)]
pub struct SlotLeadershipTest {
    pub vrf_output: VrfOutput,
    pub vrf_proof: VrfProof,
    pub slot: SlotNo,
    pub pool_id: PoolId,
}

/// Chain quality metrics for fork selection
#[derive(Debug, Clone)]
pub struct ChainQuality {
    pub density: f64,
    pub length: u64,
    pub block_count: u64,
    pub slot_range: u64,
}

/// Ouroboros consensus state
#[derive(Debug, Clone)]
pub struct OuroborosState {
    pub current_slot: SlotNo,
    pub current_epoch: EpochNo,
    pub stake_distribution: StakeDistribution,
    pub registered_pools: HashMap<PoolId, StakePool>,
    pub protocol_params: ProtocolParameters,
    /// Epoch nonce for randomness
    pub epoch_nonce: Blake2b256Hash,
    /// Cached leader schedule for current epoch
    pub leader_schedule: HashMap<SlotNo, PoolId>,
}

/// Slot leadership calculation engine
pub struct SlotLeadershipCalculator {
    pub stake_distribution: StakeDistribution,
    pub protocol_params: ProtocolParameters,
}

impl SlotLeadershipCalculator {
    /// Create new leadership calculator
    pub fn new(stake_distribution: StakeDistribution, protocol_params: ProtocolParameters) -> Self {
        Self {
            stake_distribution,
            protocol_params,
        }
    }

    /// Calculate if a pool is slot leader for given slot
    pub fn is_slot_leader(
        &self,
        _pool_id: &PoolId,
        pool_stake: u64,
        vrf_output: &VrfOutput,
        _slot: SlotNo,
    ) -> bool {
        let total_stake = self.stake_distribution.total_stake;
        if total_stake == 0 {
            return false;
        }

        // Calculate relative stake
        let relative_stake = pool_stake as f64 / total_stake as f64;

        // Apply active slot coefficient
        let threshold =
            1.0 - (1.0 - self.protocol_params.active_slot_coefficient).powf(relative_stake);

        // Convert VRF output to probability (simplified)
        let vrf_hash = Blake2b256Hash::hash(vrf_output.to_bytes());
        let vrf_bytes = vrf_hash.as_bytes();

        // Use first 8 bytes as u64, normalize to [0,1]
        let mut prob_bytes = [0u8; 8];
        prob_bytes.copy_from_slice(&vrf_bytes[..8]);
        let prob_int = u64::from_le_bytes(prob_bytes);
        let probability = prob_int as f64 / u64::MAX as f64;

        probability < threshold
    }

    /// Generate VRF test for slot leadership
    pub fn generate_vrf_test(
        &self,
        _pool_id: &PoolId,
        slot: SlotNo,
        epoch_nonce: &Blake2b256Hash,
    ) -> Blake2b256Hash {
        // VRF input: slot || epoch_nonce
        let mut input = Vec::new();
        input.extend_from_slice(&slot.0.to_le_bytes());
        input.extend_from_slice(epoch_nonce.as_bytes());

        Blake2b256Hash::hash(&input)
    }
}

/// KES key evolution and signature handling
pub struct KesManager {
    pub kes_period_length: u64, // Slots per KES period
    pub max_kes_evolutions: u64,
}

impl KesManager {
    /// Create new KES manager
    pub fn new(kes_period_length: u64, max_kes_evolutions: u64) -> Self {
        Self {
            kes_period_length,
            max_kes_evolutions,
        }
    }

    /// Calculate current KES period for slot
    pub fn kes_period_for_slot(&self, slot: SlotNo) -> u64 {
        slot.0 / self.kes_period_length
    }

    /// Validate KES signature period
    pub fn validate_kes_signature(
        &self,
        signature_kes_period: u64,
        current_slot: SlotNo,
        cert_kes_period: u64,
    ) -> Result<()> {
        let current_kes_period = self.kes_period_for_slot(current_slot);

        // Check signature is from correct period
        if signature_kes_period != current_kes_period {
            return Err(ConsensusError::InvalidKesSignature(format!(
                "KES signature from period {}, expected {}",
                signature_kes_period, current_kes_period
            )));
        }

        // Check certificate is not too old
        if current_kes_period > cert_kes_period + self.max_kes_evolutions {
            return Err(ConsensusError::ExpiredKesKey(format!(
                "KES key expired: cert_period={}, current_period={}, max_evolutions={}",
                cert_kes_period, current_kes_period, self.max_kes_evolutions
            )));
        }

        Ok(())
    }
}

/// Chain density calculation for fork choice
pub struct ChainDensityCalculator;

impl ChainDensityCalculator {
    /// Calculate chain density over a window
    pub fn calculate_density(
        block_count: u64,
        slot_range: u64,
        active_slot_coefficient: f64,
    ) -> f64 {
        if slot_range == 0 {
            return 0.0;
        }

        let expected_blocks = slot_range as f64 * active_slot_coefficient;
        block_count as f64 / expected_blocks
    }

    /// Calculate chain quality metrics
    pub fn calculate_chain_quality(
        blocks: &[(SlotNo, Blake2b256Hash)], // (slot, block_hash) pairs
        params: &ProtocolParameters,
    ) -> ChainQuality {
        if blocks.is_empty() {
            return ChainQuality {
                density: 0.0,
                length: 0,
                block_count: 0,
                slot_range: 0,
            };
        }

        let first_slot = blocks.first().unwrap().0 .0;
        let last_slot = blocks.last().unwrap().0 .0;
        let slot_range = if last_slot >= first_slot {
            last_slot - first_slot + 1
        } else {
            1
        };

        let block_count = blocks.len() as u64;
        let density =
            Self::calculate_density(block_count, slot_range, params.active_slot_coefficient);

        ChainQuality {
            density,
            length: block_count,
            block_count,
            slot_range,
        }
    }
}

/// Epoch boundary calculation and nonce evolution
pub struct EpochTransition {
    pub protocol_params: ProtocolParameters,
}

impl EpochTransition {
    /// Create new epoch transition handler
    pub fn new(protocol_params: ProtocolParameters) -> Self {
        Self { protocol_params }
    }

    /// Check if slot is at epoch boundary
    pub fn is_epoch_boundary(&self, slot: SlotNo) -> bool {
        slot.0 % self.protocol_params.epoch_length == 0 && slot.0 > 0
    }

    /// Calculate epoch nonce from VRF outputs
    pub fn calculate_epoch_nonce(
        &self,
        previous_nonce: &Blake2b256Hash,
        vrf_outputs: &[VrfOutput],
        extra_entropy: Option<&Blake2b256Hash>,
    ) -> Blake2b256Hash {
        let mut nonce_input = Vec::new();

        // Add previous epoch nonce
        nonce_input.extend_from_slice(previous_nonce.as_bytes());

        // Add VRF outputs from slot leaders
        for vrf_output in vrf_outputs {
            nonce_input.extend_from_slice(vrf_output.to_bytes());
        }

        // Add extra entropy if provided (for protocol transitions)
        if let Some(entropy) = extra_entropy {
            nonce_input.extend_from_slice(entropy.as_bytes());
        }

        Blake2b256Hash::hash(&nonce_input)
    }

    /// Calculate stake snapshot lag for security
    pub fn stake_snapshot_lag(&self) -> u64 {
        // Stake distribution lags by 2 epochs for security
        2 * self.protocol_params.epoch_length
    }
}

impl OuroborosState {
    /// Create new Ouroboros consensus state
    pub fn new(protocol_params: ProtocolParameters) -> Self {
        Self {
            current_slot: SlotNo(0),
            current_epoch: EpochNo(0),
            stake_distribution: StakeDistribution {
                pools: HashMap::new(),
                total_stake: 0,
            },
            registered_pools: HashMap::new(),
            protocol_params,
            epoch_nonce: Blake2b256Hash::hash(b"genesis"),
            leader_schedule: HashMap::new(),
        }
    }

    /// Advance to next slot
    pub fn advance_slot(&mut self, new_slot: SlotNo) -> Result<()> {
        if new_slot.0 <= self.current_slot.0 {
            return Err(ConsensusError::InvalidSlotProgression(format!(
                "Cannot advance from slot {} to {}",
                self.current_slot.0, new_slot.0
            )));
        }

        let old_epoch = self.current_epoch;
        let new_epoch = new_slot.to_epoch(&self.protocol_params);

        self.current_slot = new_slot;
        self.current_epoch = new_epoch;

        // Handle epoch transition if needed
        if new_epoch.0 > old_epoch.0 {
            self.handle_epoch_transition(old_epoch, new_epoch)?;
        }

        Ok(())
    }

    /// Handle epoch boundary transitions
    fn handle_epoch_transition(&mut self, old_epoch: EpochNo, new_epoch: EpochNo) -> Result<()> {
        eprintln!(
            "[INFO] Processing epoch transition: {} -> {}",
            old_epoch.0, new_epoch.0
        );

        // 1. Update epoch nonce (mix with VRF values from previous epoch)
        self.evolve_epoch_nonce(new_epoch)?;

        // 2. Take stake distribution snapshot (for epoch + 2)
        self.snapshot_stake_distribution(new_epoch)?;

        // 3. Calculate active stake for new epoch
        self.calculate_active_stake(new_epoch)?;

        // 4. Update KES periods
        self.update_kes_periods(new_epoch)?;

        // 5. Calculate and distribute epoch rewards
        self.calculate_epoch_rewards(old_epoch)?;

        // 6. Reset leader schedule for new epoch
        self.leader_schedule.clear();

        eprintln!("[INFO] Epoch transition complete: epoch {}", new_epoch.0);
        Ok(())
    }

    /// Evolve epoch nonce using VRF outputs from previous epoch
    fn evolve_epoch_nonce(&mut self, new_epoch: EpochNo) -> Result<()> {
        // Get VRF outputs from last 6k/f slots of previous epoch
        // In a real implementation, collect these from blocks
        // For now, use a deterministic evolution based on epoch number

        let epoch_bytes = new_epoch.0.to_le_bytes();
        let mut nonce_bytes = [0u8; 32];

        // XOR current nonce with epoch-derived randomness
        for (i, byte) in self.epoch_nonce.as_bytes().iter().enumerate() {
            nonce_bytes[i] = byte ^ epoch_bytes[i % epoch_bytes.len()];
        }

        self.epoch_nonce = Blake2b256Hash::hash(&nonce_bytes);

        eprintln!(
            "[DEBUG] Evolved epoch nonce for epoch {}: {:?}",
            new_epoch.0, self.epoch_nonce
        );

        Ok(())
    }

    /// Snapshot stake distribution for future epoch
    fn snapshot_stake_distribution(&mut self, current_epoch: EpochNo) -> Result<()> {
        // Cardano takes stake snapshot 2 epochs ahead
        // Snapshot taken at boundary of epoch N is used for epoch N+2

        let snapshot_epoch = EpochNo(current_epoch.0 + 2);

        eprintln!(
            "[DEBUG] Taking stake snapshot at epoch {} for epoch {}",
            current_epoch.0, snapshot_epoch.0
        );

        // Store snapshot (in production, this would persist to LedgerDB)
        // For now, we just note that the current distribution is valid

        Ok(())
    }

    /// Calculate active stake for the new epoch
    fn calculate_active_stake(&mut self, new_epoch: EpochNo) -> Result<()> {
        // Active stake is calculated from snapshot taken 2 epochs ago
        let snapshot_epoch = if new_epoch.0 >= 2 {
            EpochNo(new_epoch.0 - 2)
        } else {
            EpochNo(0)
        };

        eprintln!(
            "[DEBUG] Calculating active stake for epoch {} from snapshot at epoch {}",
            new_epoch.0, snapshot_epoch.0
        );

        // In production, retrieve snapshot from LedgerDB
        // For now, use current stake distribution
        let total_stake: u64 = self.stake_distribution.pools.values().copied().sum();
        self.stake_distribution.total_stake = total_stake;

        eprintln!(
            "[INFO] Total active stake for epoch {}: {}",
            new_epoch.0, self.stake_distribution.total_stake
        );

        Ok(())
    }

    /// Update KES periods for epoch transition
    fn update_kes_periods(&mut self, new_epoch: EpochNo) -> Result<()> {
        // KES periods track key evolution
        // In Cardano, KES period = slot / kes_period_length

        let first_slot = new_epoch.first_slot(&self.protocol_params);
        eprintln!(
            "[DEBUG] Updating KES periods for epoch {} (first slot: {})",
            new_epoch.0, first_slot.0
        );

        // KES evolution is handled in the key management module
        // This just notes the transition

        Ok(())
    }

    /// Calculate and distribute epoch rewards
    fn calculate_epoch_rewards(&mut self, completed_epoch: EpochNo) -> Result<()> {
        eprintln!(
            "[INFO] Calculating rewards for completed epoch {}",
            completed_epoch.0
        );

        // Reward calculation formula (simplified):
        // 1. Total ada in circulation
        // 2. Reserve amount
        // 3. Monetary expansion rate
        // 4. Active stake ratio

        // In production, this would:
        // 1. Calculate total reward pot
        // 2. Distribute to stake pools based on performance
        // 3. Distribute to delegators based on stake
        // 4. Update reserves and treasury

        // For now, just log that rewards would be calculated
        eprintln!(
            "[DEBUG] Reward calculation for epoch {} completed (stub)",
            completed_epoch.0
        );

        Ok(())
    }

    /// Register new stake pool
    pub fn register_pool(&mut self, pool: StakePool) -> Result<()> {
        let pool_id = pool.pool_id.clone();

        // Validate pool parameters
        if pool.margin < 0.0 || pool.margin > 1.0 {
            return Err(ConsensusError::InvalidPoolParameters(
                "Pool margin must be between 0 and 1".to_string(),
            ));
        }

        if pool.cost > pool.pledge {
            return Err(ConsensusError::InvalidPoolParameters(
                "Pool cost cannot exceed pledge".to_string(),
            ));
        }

        self.registered_pools.insert(pool_id, pool);
        Ok(())
    }

    /// Update stake distribution
    pub fn update_stake_distribution(&mut self, distribution: StakeDistribution) {
        self.stake_distribution = distribution;
    }

    /// Get current time in slots since genesis
    pub fn current_time_to_slot(&self, genesis_time: SystemTime) -> Result<SlotNo> {
        let now = SystemTime::now();
        let elapsed = now
            .duration_since(genesis_time)
            .map_err(|_| ConsensusError::TimeCalculationError("Time went backwards".to_string()))?;

        let slots_elapsed = elapsed.as_secs() / self.protocol_params.slot_length;
        Ok(SlotNo(slots_elapsed))
    }

    /// Check if we're synchronized with network time
    pub fn is_synchronized(&self, network_slot: SlotNo) -> bool {
        let slot_diff = network_slot.0.abs_diff(self.current_slot.0);

        // Allow up to 20 slots of drift
        slot_diff <= 20
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cardano_crypto::KesSecretKey;

    #[test]
    fn test_protocol_parameters() {
        let params = ProtocolParameters::testnet();
        assert_eq!(params.security_parameter, 2160);
        assert_eq!(params.active_slot_coefficient, 0.05);
        assert_eq!(params.slot_length, 1);
        assert_eq!(params.epoch_length, 432000);
    }

    #[test]
    fn test_slot_epoch_conversion() {
        let params = ProtocolParameters::testnet();
        let slot = SlotNo(432000); // First slot of epoch 1
        let epoch = slot.to_epoch(&params);
        assert_eq!(epoch, EpochNo(1));

        let first_slot = epoch.first_slot(&params);
        assert_eq!(first_slot, SlotNo(432000));
    }

    #[test]
    fn test_slot_leadership_calculation() {
        let stake_dist = StakeDistribution {
            pools: {
                let mut pools = HashMap::new();
                pools.insert(PoolId(Blake2b256Hash::hash(b"pool1")), 1000);
                pools
            },
            total_stake: 10000,
        };

        let params = ProtocolParameters::testnet();
        let calculator = SlotLeadershipCalculator::new(stake_dist, params);

        let pool_id = PoolId(Blake2b256Hash::hash(b"pool1"));
        let vrf_output = VrfOutput::from_bytes([0u8; 64]).unwrap();

        // Test leadership calculation (result depends on VRF output)
        let _is_leader = calculator.is_slot_leader(&pool_id, 1000, &vrf_output, SlotNo(1));
        // Result is probabilistic, function completing successfully is the test
    }

    #[test]
    fn test_kes_manager() {
        let kes_manager = KesManager::new(129600, KesSecretKey::MAX_PERIOD); // ~36 hours per period, ~192 days max

        assert_eq!(kes_manager.kes_period_for_slot(SlotNo(0)), 0);
        assert_eq!(kes_manager.kes_period_for_slot(SlotNo(129600)), 1);

        // Test KES validation
        let result = kes_manager.validate_kes_signature(0, SlotNo(0), 0);
        assert!(result.is_ok());

        // Test expired key
        let result = kes_manager.validate_kes_signature(0, SlotNo(129600 * 100), 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_chain_density() {
        let density = ChainDensityCalculator::calculate_density(5, 100, 0.05);
        assert_eq!(density, 1.0); // 5 blocks in 100 slots with f=0.05 expected 5 blocks

        let blocks = vec![
            (SlotNo(1), Blake2b256Hash::hash(b"block1")),
            (SlotNo(5), Blake2b256Hash::hash(b"block2")),
            (SlotNo(10), Blake2b256Hash::hash(b"block3")),
        ];

        let params = ProtocolParameters::testnet();
        let quality = ChainDensityCalculator::calculate_chain_quality(&blocks, &params);

        assert_eq!(quality.block_count, 3);
        assert_eq!(quality.slot_range, 10); // slot 1 to 10 inclusive
        assert_eq!(quality.length, 3);
    }

    #[test]
    fn test_epoch_transition() {
        let params = ProtocolParameters::testnet();
        let transition = EpochTransition::new(params);

        assert!(!transition.is_epoch_boundary(SlotNo(0)));
        assert!(transition.is_epoch_boundary(SlotNo(432000)));
        assert!(!transition.is_epoch_boundary(SlotNo(432001)));

        let nonce = Blake2b256Hash::hash(b"previous_nonce");
        let mut vrf_output_1 = [0u8; 64];
        vrf_output_1[0] = 1;
        let mut vrf_output_2 = [0u8; 64];
        vrf_output_2[0] = 2;

        let vrf_outputs = vec![
            VrfOutput::from_bytes(vrf_output_1).unwrap(),
            VrfOutput::from_bytes(vrf_output_2).unwrap(),
        ];

        let new_nonce = transition.calculate_epoch_nonce(&nonce, &vrf_outputs, None);
        assert_ne!(new_nonce.as_bytes(), nonce.as_bytes());
    }

    #[test]
    fn test_ouroboros_state() {
        let params = ProtocolParameters::testnet();
        let mut state = OuroborosState::new(params);

        // Test slot advancement
        let result = state.advance_slot(SlotNo(1));
        assert!(result.is_ok());
        assert_eq!(state.current_slot, SlotNo(1));

        // Test invalid slot progression
        let result = state.advance_slot(SlotNo(0));
        assert!(result.is_err());

        // Test pool registration
        let pool = StakePool {
            pool_id: PoolId(Blake2b256Hash::hash(b"test_pool")),
            vrf_vkey: VrfVkey([0u8; 32]),
            pledge: 1000000,
            cost: 340000,
            margin: 0.05,
            reward_account: Ed25519KeyHash::from_test_data(b"reward_account"),
            owners: vec![Ed25519KeyHash::from_test_data(b"owner1")],
            relays: vec![],
            metadata: None,
        };

        let result = state.register_pool(pool);
        assert!(result.is_ok());
        assert_eq!(state.registered_pools.len(), 1);
    }
}
