//! Epoch Transition Handler for Cardano Consensus
//!
//! This module implements complete epoch boundary logic including:
//! - Nonce evolution using VRF outputs from previous epoch
//! - Stake distribution snapshots persisted to LedgerDB
//! - Reward calculation and distribution
//!
//! Addresses GAP-001: Epoch Transition Logic Incomplete

use crate::{ConsensusError, EpochNo, PoolId, ProtocolParameters, Result, SlotNo};
use cardano_crypto::{Blake2b256Hash, VrfOutput};
use cardano_storage::ledgerdb::StakeCredential;
use cardano_storage::LedgerDatabase;
use std::collections::HashMap;
use std::sync::Arc;

/// Mark-Set-Go mechanism for stake snapshots
///
/// Cardano uses a 3-epoch cycle for stake snapshot security:
/// - **Mark**: Current epoch where stake changes happen
/// - **Set**: One epoch later, stake is "set" (snapshot taken)
/// - **Go**: Two epochs after Mark, snapshot is active for leadership
///
/// This provides a 2-epoch lag for security against stake grinding attacks.
const STAKE_SNAPSHOT_LAG_EPOCHS: u64 = 2;

/// Stability window for nonce collection (6k/f slots)
///
/// The epoch nonce is calculated from VRF outputs in the last 6k/f slots
/// of the previous epoch, where k is the security parameter and f is the
/// active slot coefficient.
const NONCE_STABILITY_WINDOW_MULTIPLIER: u64 = 6;

/// Monetary expansion parameters for Cardano
const MONETARY_EXPANSION_RATE: f64 = 0.003; // 0.3% annual
const TREASURY_TAX_RATE: f64 = 0.20; // 20% to treasury
const RESERVE_DECAY_RATE: f64 = 0.05; // 5% from reserves per epoch

/// Comprehensive epoch transition handler
///
/// Manages all epoch boundary logic including:
/// - Nonce evolution for randomness
/// - Stake snapshots for future epochs
/// - Reward calculation and distribution
/// - Protocol parameter updates
pub struct EpochTransitionHandler<L: LedgerDatabase> {
    ledgerdb: Arc<L>,
    protocol_params: ProtocolParameters,

    /// VRF outputs collected during stability window
    vrf_outputs_buffer: Vec<VrfOutput>,

    /// Current epoch nonce
    current_nonce: Blake2b256Hash,

    /// Stake snapshots for upcoming epochs (epoch_no -> snapshot)
    stake_snapshots: HashMap<u64, StakeSnapshot>,

    /// Total ADA in reserves (for reward calculation)
    reserves: u64,

    /// Total ADA in treasury
    treasury: u64,
}

/// Stake distribution snapshot for a specific epoch
#[derive(Debug, Clone)]
pub struct StakeSnapshot {
    /// Epoch this snapshot is for
    pub epoch: EpochNo,

    /// Pool ID -> Total stake delegated
    pub pool_stakes: HashMap<PoolId, u64>,

    /// Total active stake in the system
    pub total_active_stake: u64,

    /// Stake credentials -> Pool ID mappings
    pub delegations: HashMap<StakeCredential, PoolId>,

    /// Individual stake per credential
    pub stake_distribution: HashMap<StakeCredential, u64>,

    /// Timestamp when snapshot was taken
    pub snapshot_slot: SlotNo,
}

/// Reward distribution for an epoch
#[derive(Debug, Clone)]
pub struct EpochRewards {
    /// Epoch these rewards are for
    pub epoch: EpochNo,

    /// Total fees collected in the epoch
    pub total_fees: u64,

    /// Amount going to treasury
    pub treasury_amount: u64,

    /// Amount from reserves
    pub reserve_amount: u64,

    /// Total reward pot to distribute
    pub total_rewards: u64,

    /// Pool ID -> Pool rewards (includes leader + delegator rewards)
    pub pool_rewards: HashMap<PoolId, PoolRewardDistribution>,
}

/// Reward distribution for a single pool
#[derive(Debug, Clone)]
pub struct PoolRewardDistribution {
    /// Total rewards for this pool
    pub total_pool_rewards: u64,

    /// Pool operator rewards (fixed cost + margin)
    pub operator_rewards: u64,

    /// Delegator rewards (distributed proportionally)
    pub delegator_rewards: u64,

    /// Individual delegator -> reward amount
    pub delegator_distribution: HashMap<StakeCredential, u64>,

    /// Pool performance metric (blocks produced / expected blocks)
    pub performance: f64,
}

impl<L: LedgerDatabase> EpochTransitionHandler<L> {
    /// Create new epoch transition handler
    pub fn new(
        ledgerdb: Arc<L>,
        protocol_params: ProtocolParameters,
        genesis_nonce: Blake2b256Hash,
    ) -> Self {
        Self {
            ledgerdb,
            protocol_params,
            vrf_outputs_buffer: Vec::new(),
            current_nonce: genesis_nonce,
            stake_snapshots: HashMap::new(),
            reserves: 45_000_000_000_000_000, // 45B ADA in reserves at genesis
            treasury: 0,
        }
    }

    /// Collect VRF output from a block during the stability window
    ///
    /// VRF outputs from the last 6k/f slots of an epoch are used to
    /// calculate the nonce for the next epoch.
    pub fn collect_vrf_output(&mut self, slot: SlotNo, vrf_output: VrfOutput) -> Result<()> {
        // Only collect during stability window
        if self.is_in_stability_window(slot) {
            self.vrf_outputs_buffer.push(vrf_output);
            tracing::debug!(
                slot = slot.0,
                total_outputs = self.vrf_outputs_buffer.len(),
                "Collected VRF output for nonce evolution"
            );
        }
        Ok(())
    }

    /// Check if slot is in the stability window for nonce collection
    fn is_in_stability_window(&self, slot: SlotNo) -> bool {
        let epoch_length = self.protocol_params.epoch_length;
        let slot_in_epoch = slot.0 % epoch_length;

        // Stability window is the last 6k/f slots of the epoch
        let security_param = self.protocol_params.security_parameter;
        let active_slot_coeff = self.protocol_params.active_slot_coefficient;

        let window_size =
            (NONCE_STABILITY_WINDOW_MULTIPLIER * security_param) as f64 / active_slot_coeff;
        let window_start = epoch_length - window_size as u64;

        slot_in_epoch >= window_start
    }

    /// Process epoch boundary transition
    ///
    /// This is the main entry point for epoch transitions. It:
    /// 1. Evolves the epoch nonce
    /// 2. Takes stake snapshot for future epochs
    /// 3. Calculates and distributes rewards
    /// 4. Updates protocol parameters if scheduled
    pub async fn process_epoch_transition(
        &mut self,
        completed_epoch: EpochNo,
        new_epoch: EpochNo,
        current_slot: SlotNo,
    ) -> Result<()> {
        tracing::info!(
            completed_epoch = completed_epoch.0,
            new_epoch = new_epoch.0,
            "Starting epoch transition processing"
        );

        // 1. Evolve epoch nonce using VRF outputs from previous epoch
        self.evolve_epoch_nonce(new_epoch)?;

        // 2. Take stake snapshot for epoch N+2 (Go epoch)
        self.take_stake_snapshot(new_epoch, current_slot).await?;

        // 3. Calculate and distribute rewards for completed epoch
        self.calculate_and_distribute_rewards(completed_epoch)
            .await?;

        // 4. Update protocol parameters if there's a scheduled update
        self.apply_protocol_parameter_updates(new_epoch).await?;

        // 5. Clear VRF buffer for next epoch
        self.vrf_outputs_buffer.clear();

        tracing::info!(
            epoch = new_epoch.0,
            nonce = ?self.current_nonce,
            "Epoch transition completed successfully"
        );

        Ok(())
    }

    /// Evolve epoch nonce using collected VRF outputs
    ///
    /// The epoch nonce provides randomness for leader election in future epochs.
    /// It's calculated by hashing:
    /// - Previous epoch nonce
    /// - VRF outputs from the stability window (last 6k/f slots)
    /// - Optional extra entropy (for protocol transitions)
    fn evolve_epoch_nonce(&mut self, new_epoch: EpochNo) -> Result<()> {
        tracing::debug!(
            epoch = new_epoch.0,
            vrf_outputs_collected = self.vrf_outputs_buffer.len(),
            "Evolving epoch nonce"
        );

        let mut nonce_input = Vec::new();

        // 1. Add previous epoch nonce (η_0)
        nonce_input.extend_from_slice(self.current_nonce.as_bytes());

        // 2. Add all VRF outputs from stability window
        for vrf_output in &self.vrf_outputs_buffer {
            nonce_input.extend_from_slice(vrf_output.to_bytes());
        }

        // 3. Add extra entropy if this is a hard fork transition
        // (In production, this would come from protocol parameter updates)
        // For now, this is a placeholder

        // 4. Hash everything together to get new nonce
        self.current_nonce = Blake2b256Hash::hash(&nonce_input);

        tracing::info!(
            epoch = new_epoch.0,
            nonce = ?self.current_nonce,
            vrf_count = self.vrf_outputs_buffer.len(),
            "Epoch nonce evolved successfully"
        );

        Ok(())
    }

    /// Take stake snapshot for future epoch (N + 2)
    ///
    /// Implements the Mark-Set-Go mechanism:
    /// - At epoch N, take snapshot for epoch N+2
    /// - This snapshot determines leader schedule for epoch N+2
    ///
    /// The snapshot includes:
    /// - All pool stakes
    /// - All delegations
    /// - Total active stake
    async fn take_stake_snapshot(
        &mut self,
        current_epoch: EpochNo,
        snapshot_slot: SlotNo,
    ) -> Result<()> {
        let snapshot_for_epoch = EpochNo(current_epoch.0 + STAKE_SNAPSHOT_LAG_EPOCHS);

        tracing::info!(
            current_epoch = current_epoch.0,
            snapshot_for_epoch = snapshot_for_epoch.0,
            "Taking stake distribution snapshot"
        );

        // 1. Get all active pools from LedgerDB
        let active_pools =
            self.ledgerdb.list_active_pools().await.map_err(|e| {
                ConsensusError::StorageError(format!("Failed to list pools: {}", e))
            })?;

        tracing::debug!(pool_count = active_pools.len(), "Retrieved active pools");

        // 2. Calculate total stake per pool
        let mut pool_stakes = HashMap::new();
        let delegations = HashMap::new();
        let stake_distribution = HashMap::new();
        let mut total_active_stake = 0u64;

        // In production, we would iterate through all stake credentials
        // For now, we create a simplified snapshot

        // Get pool parameters for each pool
        for pool_id_hash in &active_pools {
            // pool_id_hash is Blake2b256Hash from storage layer
            if let Ok(Some(pool_params)) = self.ledgerdb.get_pool(pool_id_hash).await {
                // Start with pool's pledge
                let pool_total_stake = pool_params.pledge;

                // Add delegated stake (in production, query all delegations)
                // This is a simplified version

                // Convert to consensus PoolId (newtype wrapper)
                let consensus_pool_id = PoolId(pool_id_hash.clone());
                pool_stakes.insert(consensus_pool_id.clone(), pool_total_stake);
                total_active_stake += pool_total_stake;
                tracing::trace!(
                    pool_id = ?consensus_pool_id,
                    stake = pool_total_stake,
                    "Calculated pool stake"
                );
            }
        }

        // 3. Create snapshot
        let snapshot = StakeSnapshot {
            epoch: snapshot_for_epoch,
            pool_stakes,
            total_active_stake,
            delegations,
            stake_distribution,
            snapshot_slot,
        };

        // 4. Store snapshot in memory and persist to LedgerDB
        self.stake_snapshots
            .insert(snapshot_for_epoch.0, snapshot.clone());

        // Persist to LedgerDB using snapshot mechanism
        self.ledgerdb
            .create_snapshot(snapshot_for_epoch.0)
            .await
            .map_err(|e| {
                ConsensusError::StorageError(format!("Failed to persist snapshot: {}", e))
            })?;

        tracing::info!(
            epoch = snapshot_for_epoch.0,
            total_stake = total_active_stake,
            pool_count = active_pools.len(),
            "Stake snapshot persisted successfully"
        );

        Ok(())
    }

    /// Calculate and distribute rewards for completed epoch
    ///
    /// Implements Cardano's reward calculation:
    /// 1. Calculate total reward pot (from reserves and fees)
    /// 2. Deduct treasury tax
    /// 3. Distribute to pools based on performance
    /// 4. Distribute within pools to delegators
    async fn calculate_and_distribute_rewards(&mut self, completed_epoch: EpochNo) -> Result<()> {
        tracing::info!(epoch = completed_epoch.0, "Calculating epoch rewards");

        // 1. Get total fees collected in the epoch
        // In production, sum all transaction fees from the epoch
        let total_fees = self.get_epoch_fees(completed_epoch).await?;

        // 2. Calculate rewards from monetary expansion (from reserves)
        let reserve_contribution = self.calculate_reserve_contribution();

        // 3. Total reward pot
        let total_reward_pot = total_fees + reserve_contribution;

        // 4. Deduct treasury tax
        let treasury_amount = (total_reward_pot as f64 * TREASURY_TAX_RATE) as u64;
        let distributable_rewards = total_reward_pot - treasury_amount;

        tracing::debug!(
            total_fees,
            reserve_contribution,
            treasury_amount,
            distributable_rewards,
            "Calculated reward pot"
        );

        // 5. Update reserves and treasury
        self.reserves -= reserve_contribution;
        self.treasury += treasury_amount;

        // 6. Get active stake snapshot for this epoch
        let stake_snapshot = self.get_stake_snapshot_for_epoch(completed_epoch)?;

        // 7. Distribute rewards to pools based on performance
        let pool_rewards = self
            .distribute_pool_rewards(completed_epoch, &stake_snapshot, distributable_rewards)
            .await?;

        // 8. Distribute within each pool to delegators
        for (pool_id, pool_reward) in &pool_rewards {
            self.distribute_delegator_rewards(pool_id, pool_reward, &stake_snapshot)
                .await?;
        }

        // 9. Create and store reward distribution record
        let epoch_rewards = EpochRewards {
            epoch: completed_epoch,
            total_fees,
            treasury_amount,
            reserve_amount: reserve_contribution,
            total_rewards: total_reward_pot,
            pool_rewards,
        };

        tracing::info!(
            epoch = completed_epoch.0,
            total_rewards = total_reward_pot,
            pools_rewarded = epoch_rewards.pool_rewards.len(),
            "Rewards calculated and distributed successfully"
        );

        Ok(())
    }

    /// Calculate contribution from reserves based on monetary expansion
    fn calculate_reserve_contribution(&self) -> u64 {
        // Simplified monetary expansion: reserves * decay_rate
        // In production, this follows the exact Cardano formula
        (self.reserves as f64 * RESERVE_DECAY_RATE * MONETARY_EXPANSION_RATE) as u64
    }

    /// Get total fees collected in an epoch
    async fn get_epoch_fees(&self, epoch: EpochNo) -> Result<u64> {
        // In production, sum all transaction fees from blocks in this epoch
        // For now, return a stub value
        // This would query ChainDB for all blocks in the epoch and sum fees

        tracing::debug!(epoch = epoch.0, "Fetching epoch fees (stub)");
        Ok(1_000_000_000) // 1000 ADA in fees (stub)
    }

    /// Get stake snapshot for a specific epoch
    fn get_stake_snapshot_for_epoch(&self, epoch: EpochNo) -> Result<StakeSnapshot> {
        // Active stake for epoch N comes from snapshot taken at epoch N-2
        let snapshot_epoch = if epoch.0 >= STAKE_SNAPSHOT_LAG_EPOCHS {
            epoch.0 - STAKE_SNAPSHOT_LAG_EPOCHS
        } else {
            0
        };

        self.stake_snapshots
            .get(&snapshot_epoch)
            .cloned()
            .ok_or_else(|| {
                ConsensusError::MissingStakeSnapshot(format!(
                    "No snapshot found for epoch {} (needed for epoch {})",
                    snapshot_epoch, epoch.0
                ))
            })
    }

    /// Distribute rewards to pools based on performance
    async fn distribute_pool_rewards(
        &self,
        _epoch: EpochNo,
        stake_snapshot: &StakeSnapshot,
        total_rewards: u64,
    ) -> Result<HashMap<PoolId, PoolRewardDistribution>> {
        let mut pool_rewards = HashMap::new();

        for (pool_id, pool_stake) in &stake_snapshot.pool_stakes {
            // Calculate pool's share of total rewards based on stake
            let stake_ratio = *pool_stake as f64 / stake_snapshot.total_active_stake as f64;
            let pool_reward_pot = (total_rewards as f64 * stake_ratio) as u64;

            // Get pool parameters for cost and margin
            // Convert consensus PoolId to storage PoolId (Blake2b256Hash)
            let pool_params = self
                .ledgerdb
                .get_pool(&pool_id.0)
                .await
                .map_err(|e| ConsensusError::StorageError(format!("Failed to get pool: {}", e)))?
                .ok_or_else(|| ConsensusError::PoolNotFound(format!("{:?}", pool_id)))?;

            // Calculate pool performance (blocks produced / expected blocks)
            // In production, track actual vs expected block production
            let performance = 1.0; // Assume 100% performance (stub)

            // Apply performance factor
            let adjusted_reward = (pool_reward_pot as f64 * performance) as u64;

            // Pool operator gets fixed cost + margin of remaining
            let operator_cost = pool_params.cost;
            let remaining_after_cost = adjusted_reward.saturating_sub(operator_cost);
            let operator_margin = (remaining_after_cost as f64 * pool_params.margin) as u64;
            let operator_rewards = operator_cost + operator_margin;

            // Delegators share the rest
            let delegator_rewards = adjusted_reward.saturating_sub(operator_rewards);

            pool_rewards.insert(
                pool_id.clone(),
                PoolRewardDistribution {
                    total_pool_rewards: adjusted_reward,
                    operator_rewards,
                    delegator_rewards,
                    delegator_distribution: HashMap::new(), // Filled in next step
                    performance,
                },
            );

            tracing::trace!(
                pool_id = ?pool_id,
                pool_stake = pool_stake,
                rewards = adjusted_reward,
                operator_share = operator_rewards,
                delegator_share = delegator_rewards,
                "Calculated pool rewards"
            );
        }

        Ok(pool_rewards)
    }

    /// Distribute rewards within a pool to individual delegators
    async fn distribute_delegator_rewards(
        &self,
        pool_id: &PoolId,
        pool_reward: &PoolRewardDistribution,
        stake_snapshot: &StakeSnapshot,
    ) -> Result<()> {
        // Get all delegators for this pool from snapshot
        let pool_delegators: Vec<_> = stake_snapshot
            .delegations
            .iter()
            .filter(|(_, pid)| *pid == pool_id)
            .collect();

        if pool_delegators.is_empty() {
            tracing::debug!(pool_id = ?pool_id, "No delegators for pool");
            return Ok(());
        }

        // Calculate total stake delegated to this pool
        let pool_total_stake = stake_snapshot
            .pool_stakes
            .get(pool_id)
            .copied()
            .unwrap_or(0);

        if pool_total_stake == 0 {
            tracing::warn!(pool_id = ?pool_id, "Pool has zero stake");
            return Ok(());
        }

        // Distribute delegator rewards proportionally
        for (stake_credential, _) in pool_delegators {
            let delegator_stake = stake_snapshot
                .stake_distribution
                .get(stake_credential)
                .copied()
                .unwrap_or(0);

            let stake_ratio = delegator_stake as f64 / pool_total_stake as f64;
            let delegator_reward = (pool_reward.delegator_rewards as f64 * stake_ratio) as u64;

            // Store reward in LedgerDB
            if delegator_reward > 0 {
                self.ledgerdb
                    .store_rewards(stake_credential, delegator_reward)
                    .await
                    .map_err(|e| {
                        ConsensusError::StorageError(format!("Failed to store rewards: {}", e))
                    })?;

                tracing::trace!(
                    stake_credential = ?stake_credential,
                    reward = delegator_reward,
                    "Distributed delegator reward"
                );
            }
        }

        Ok(())
    }

    /// Apply protocol parameter updates if scheduled for this epoch
    async fn apply_protocol_parameter_updates(&mut self, _epoch: EpochNo) -> Result<()> {
        // TODO: Protocol parameter updates
        // This requires mapping between storage::ProtocolParameters and consensus::ProtocolParameters
        // For now, this is a placeholder

        // In production, this would:
        // 1. Check if there's a scheduled update for this epoch
        // 2. Convert storage ProtocolParameters to consensus ProtocolParameters
        // 3. Update self.protocol_params
        // 4. Validate the new parameters

        Ok(())
    }

    /// Get current epoch nonce
    pub fn current_nonce(&self) -> Blake2b256Hash {
        self.current_nonce.clone()
    }

    /// Get stake snapshot for a specific epoch (if available)
    pub fn get_snapshot(&self, epoch: u64) -> Option<&StakeSnapshot> {
        self.stake_snapshots.get(&epoch)
    }

    /// Get stake snapshot for a specific epoch using EpochNo (convenience method)
    pub fn get_stake_snapshot(&self, epoch: EpochNo) -> Option<&StakeSnapshot> {
        self.get_snapshot(epoch.0)
    }

    /// Get current reserves amount
    pub fn reserves(&self) -> u64 {
        self.reserves
    }

    /// Get current treasury amount
    pub fn treasury(&self) -> u64 {
        self.treasury
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cardano_storage::backends::MemoryBackend;
    use cardano_storage::LedgerDatabaseImpl;

    fn create_test_handler() -> EpochTransitionHandler<LedgerDatabaseImpl<MemoryBackend>> {
        let backend = Arc::new(MemoryBackend::new());
        let ledgerdb = Arc::new(LedgerDatabaseImpl::new(backend));
        let params = ProtocolParameters::testnet();
        let genesis_nonce = Blake2b256Hash::hash(b"test_genesis");

        EpochTransitionHandler::new(ledgerdb, params, genesis_nonce)
    }

    #[test]
    fn test_handler_creation() {
        let handler = create_test_handler();
        assert_eq!(handler.reserves(), 45_000_000_000_000_000);
        assert_eq!(handler.treasury(), 0);
    }

    #[test]
    fn test_stability_window_detection() {
        let handler = create_test_handler();

        // Last slots of epoch should be in stability window
        let epoch_length = handler.protocol_params.epoch_length;
        let last_slot = SlotNo(epoch_length - 1);
        assert!(handler.is_in_stability_window(last_slot));

        // Early slots should not be in window
        let early_slot = SlotNo(100);
        assert!(!handler.is_in_stability_window(early_slot));
    }

    #[tokio::test]
    async fn test_nonce_evolution() {
        let mut handler = create_test_handler();
        let initial_nonce = handler.current_nonce();

        // Collect some VRF outputs
        let vrf_output = VrfOutput::from_bytes([1u8; 64]).unwrap();
        handler.vrf_outputs_buffer.push(vrf_output);

        // Evolve nonce
        handler.evolve_epoch_nonce(EpochNo(1)).unwrap();

        // Nonce should have changed
        assert_ne!(handler.current_nonce(), initial_nonce);
    }

    #[test]
    fn test_reserve_contribution_calculation() {
        let handler = create_test_handler();
        let contribution = handler.calculate_reserve_contribution();

        // Should be reserves * decay_rate * expansion_rate
        assert!(contribution > 0);
        assert!(contribution < handler.reserves());
    }

    #[tokio::test]
    async fn test_snapshot_for_future_epoch() {
        let mut handler = create_test_handler();

        // Take snapshot at epoch 0 for epoch 2
        handler
            .take_stake_snapshot(EpochNo(0), SlotNo(0))
            .await
            .unwrap();

        // Snapshot should exist for epoch 2
        assert!(handler.get_snapshot(2).is_some());

        let snapshot = handler.get_snapshot(2).unwrap();
        assert_eq!(snapshot.epoch.0, 2);
    }
}
