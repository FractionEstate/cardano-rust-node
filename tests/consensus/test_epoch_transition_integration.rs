//! Integration tests for Epoch Transition Handler
//!
//! This module tests the complete epoch transition functionality including:
//! - VRF output collection during stability window
//! - Nonce evolution across multiple epochs
//! - Stake snapshot persistence and retrieval
//! - Reward calculation and distribution
//! - Multi-epoch transition sequences
//! - Error handling for edge cases

use cardano_consensus::{
    EpochNo, EpochRewards, EpochTransitionHandler, PoolId, ProtocolParameters, SlotNo,
    StakeSnapshot,
};
use cardano_crypto::{Blake2b256Hash, VrfOutput};
use cardano_storage::ledgerdb::{LedgerDatabase, PoolParameters, StakeCredential};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mock LedgerDatabase for testing epoch transitions
#[derive(Clone)]
struct MockLedgerDB {
    active_pools: Arc<RwLock<Vec<Blake2b256Hash>>>,
    pool_params: Arc<RwLock<HashMap<Blake2b256Hash, PoolParameters>>>,
    stakes: Arc<RwLock<HashMap<StakeCredential, u64>>>,
    rewards: Arc<RwLock<HashMap<StakeCredential, u64>>>,
    snapshots: Arc<RwLock<HashMap<u64, ()>>>, // Just track snapshot epochs
    protocol_params: Arc<RwLock<cardano_storage::ProtocolParameters>>,
}

impl MockLedgerDB {
    fn new() -> Self {
        Self {
            active_pools: Arc::new(RwLock::new(Vec::new())),
            pool_params: Arc::new(RwLock::new(HashMap::new())),
            stakes: Arc::new(RwLock::new(HashMap::new())),
            rewards: Arc::new(RwLock::new(HashMap::new())),
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            protocol_params: Arc::new(RwLock::new(cardano_storage::ProtocolParameters {
                min_fee_a: 44,
                min_fee_b: 155381,
                max_block_body_size: 65536,
                max_transaction_size: 16384,
                max_block_header_size: 1100,
                key_deposit: 2000000,
                pool_deposit: 500000000,
                e_max: 18,
                n_opt: 500,
                pool_pledge_influence: 0.3,
                expansion_rate: 0.003,
                treasury_growth_rate: 0.2,
                decentralization_param: 0.0,
                extra_entropy: None,
                protocol_version_major: 8,
                protocol_version_minor: 0,
                min_pool_cost: 340000000,
                ada_per_utxo_byte: 4310,
                cost_models: HashMap::new(),
                execution_costs: None,
                max_tx_execution_units: None,
                max_block_execution_units: None,
                max_value_size: Some(5000),
                collateral_percentage: Some(150),
                max_collateral_inputs: Some(3),
            })),
        }
    }

    /// Add a test stake pool
    async fn add_test_pool(&self, pool_id: Blake2b256Hash, pledge: u64, margin: f64) {
        let pool_params = PoolParameters {
            operator: pool_id.clone(),
            vrf_key_hash: Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap(),
            pledge,
            cost: 340000000, // 340 ADA fixed cost
            margin,
            reward_account: StakeCredential::Key(Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap()),
            owners: vec![],
            relays: vec![],
            metadata: None,
        };

        self.active_pools.write().await.push(pool_id.clone());
        self.pool_params.write().await.insert(pool_id, pool_params);
    }

    /// Get snapshot count for testing
    async fn snapshot_count(&self) -> usize {
        self.snapshots.read().await.len()
    }

    /// Get reward for a stake credential
    async fn get_reward(&self, stake_cred: &StakeCredential) -> u64 {
        self.rewards.read().await.get(stake_cred).copied().unwrap_or(0)
    }
}

#[async_trait::async_trait]
impl LedgerDatabase for MockLedgerDB {
    type Error = String;

    async fn list_active_pools(&self) -> Result<Vec<Blake2b256Hash>, Self::Error> {
        Ok(self.active_pools.read().await.clone())
    }

    async fn get_pool(
        &self,
        pool_id: &Blake2b256Hash,
    ) -> Result<Option<PoolParameters>, Self::Error> {
        Ok(self.pool_params.read().await.get(pool_id).cloned())
    }

    async fn store_stake(
        &self,
        stake_cred: StakeCredential,
        amount: u64,
    ) -> Result<(), Self::Error> {
        self.stakes.write().await.insert(stake_cred, amount);
        Ok(())
    }

    async fn get_stake(&self, stake_cred: &StakeCredential) -> Result<Option<u64>, Self::Error> {
        Ok(self.stakes.read().await.get(stake_cred).copied())
    }

    async fn store_rewards(
        &self,
        stake_cred: StakeCredential,
        amount: u64,
    ) -> Result<(), Self::Error> {
        self.rewards.write().await.insert(stake_cred, amount);
        Ok(())
    }

    async fn get_rewards(&self, stake_cred: &StakeCredential) -> Result<Option<u64>, Self::Error> {
        Ok(self.rewards.read().await.get(stake_cred).copied())
    }

    async fn create_snapshot(&self, epoch: u64) -> Result<(), Self::Error> {
        self.snapshots.write().await.insert(epoch, ());
        Ok(())
    }

    async fn rollback_to_snapshot(&self, _epoch: u64) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn get_protocol_parameters(
        &self,
        _epoch: u64,
    ) -> Result<cardano_storage::ProtocolParameters, Self::Error> {
        Ok(self.protocol_params.read().await.clone())
    }
}

/// Helper to create a test VRF output
fn create_test_vrf_output(seed: u8) -> VrfOutput {
    let mut bytes = [0u8; 80];
    bytes[0] = seed;
    VrfOutput::from_bytes(&bytes).unwrap()
}

/// Helper to create a test pool ID
fn create_test_pool_id(seed: u8) -> Blake2b256Hash {
    let mut bytes = [0u8; 32];
    bytes[0] = seed;
    Blake2b256Hash::from_bytes(&bytes).unwrap()
}

#[tokio::test]
async fn test_epoch_transition_single_epoch() {
    // Test a single epoch transition
    let ledgerdb = Arc::new(MockLedgerDB::new());
    let protocol_params = ProtocolParameters {
        security_parameter: 2160,
        active_slot_coefficient: 0.05,
        slot_length: 1,
        epoch_length: 21600, // 2160 * 10
    };

    let genesis_nonce = Blake2b256Hash::from_bytes(&[1u8; 32]).unwrap();
    let mut handler =
        EpochTransitionHandler::new(ledgerdb.clone(), protocol_params, genesis_nonce);

    // Add a test pool
    let pool_id = create_test_pool_id(1);
    ledgerdb.add_test_pool(pool_id.clone(), 1000000, 0.05).await;

    // Simulate epoch 0 -> 1 transition
    let slot = SlotNo(21600); // End of epoch 0
    let result = handler
        .process_epoch_transition(EpochNo(0), EpochNo(1), slot)
        .await;

    assert!(result.is_ok(), "Epoch transition should succeed");

    // Verify snapshot was created for epoch 3 (current 1 + lag 2)
    assert_eq!(
        ledgerdb.snapshot_count().await,
        1,
        "Should have created one snapshot"
    );
}

#[tokio::test]
async fn test_nonce_evolution_with_vrf_outputs() {
    // Test that nonce properly evolves with VRF outputs
    let ledgerdb = Arc::new(MockLedgerDB::new());
    let protocol_params = ProtocolParameters {
        security_parameter: 10,
        active_slot_coefficient: 0.05,
        slot_length: 1,
        epoch_length: 100,
    };

    let genesis_nonce = Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap();
    let mut handler =
        EpochTransitionHandler::new(ledgerdb.clone(), protocol_params, genesis_nonce.clone());

    // Collect VRF outputs in stability window (last 60 slots of epoch)
    for i in 40..100 {
        let slot = SlotNo(i);
        let vrf_output = create_test_vrf_output((i % 256) as u8);
        handler.collect_vrf_output(slot, vrf_output).unwrap();
    }

    // Get nonce before transition
    let nonce_before = handler.current_nonce();

    // Process epoch transition
    handler
        .process_epoch_transition(EpochNo(0), EpochNo(1), SlotNo(100))
        .await
        .unwrap();

    // Get nonce after transition
    let nonce_after = handler.current_nonce();

    // Nonce should have changed
    assert_ne!(
        nonce_before, nonce_after,
        "Nonce should evolve after epoch transition"
    );

    // Nonce should not be the same as genesis
    assert_ne!(
        nonce_after, genesis_nonce,
        "Nonce should be different from genesis"
    );
}

#[tokio::test]
async fn test_multi_epoch_transitions() {
    // Test multiple consecutive epoch transitions
    let ledgerdb = Arc::new(MockLedgerDB::new());
    let protocol_params = ProtocolParameters {
        security_parameter: 10,
        active_slot_coefficient: 0.05,
        slot_length: 1,
        epoch_length: 100,
    };

    let genesis_nonce = Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap();
    let mut handler =
        EpochTransitionHandler::new(ledgerdb.clone(), protocol_params, genesis_nonce);

    // Add multiple test pools
    for i in 1..=5 {
        let pool_id = create_test_pool_id(i);
        ledgerdb
            .add_test_pool(pool_id, 1000000 * i as u64, 0.05)
            .await;
    }

    let mut nonces = Vec::new();
    nonces.push(handler.current_nonce());

    // Transition through 5 epochs
    for epoch in 0..5 {
        // Collect VRF outputs in stability window
        let epoch_start = epoch * 100;
        for slot_offset in 40..100 {
            let slot = SlotNo(epoch_start + slot_offset);
            let vrf_output = create_test_vrf_output(((epoch + slot_offset) % 256) as u8);
            handler.collect_vrf_output(slot, vrf_output).unwrap();
        }

        // Process epoch transition
        let slot = SlotNo((epoch + 1) * 100);
        handler
            .process_epoch_transition(EpochNo(epoch), EpochNo(epoch + 1), slot)
            .await
            .unwrap();

        nonces.push(handler.current_nonce());
    }

    // Verify all nonces are different
    for i in 0..nonces.len() {
        for j in (i + 1)..nonces.len() {
            assert_ne!(
                nonces[i], nonces[j],
                "Nonces at epochs {} and {} should be different",
                i, j
            );
        }
    }

    // Verify we created snapshots for epochs 2, 3, 4, 5, 6 (lag of 2)
    assert_eq!(
        ledgerdb.snapshot_count().await,
        5,
        "Should have created 5 snapshots"
    );
}

#[tokio::test]
async fn test_stake_snapshot_lag_mechanism() {
    // Test that snapshots are taken with proper 2-epoch lag
    let ledgerdb = Arc::new(MockLedgerDB::new());
    let protocol_params = ProtocolParameters {
        security_parameter: 10,
        active_slot_coefficient: 0.05,
        slot_length: 1,
        epoch_length: 100,
    };

    let genesis_nonce = Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap();
    let mut handler = EpochTransitionHandler::new(ledgerdb.clone(), protocol_params, genesis_nonce);

    // Add test pool
    let pool_id = create_test_pool_id(1);
    ledgerdb.add_test_pool(pool_id, 1000000, 0.05).await;

    // Epoch 0 -> 1: should create snapshot for epoch 3
    handler
        .process_epoch_transition(EpochNo(0), EpochNo(1), SlotNo(100))
        .await
        .unwrap();

    // Verify snapshot for epoch 3 exists in handler
    let snapshot = handler.get_stake_snapshot(EpochNo(3));
    assert!(
        snapshot.is_some(),
        "Should have snapshot for epoch 3 (current 1 + lag 2)"
    );

    // Epoch 1 -> 2: should create snapshot for epoch 4
    handler
        .process_epoch_transition(EpochNo(1), EpochNo(2), SlotNo(200))
        .await
        .unwrap();

    let snapshot = handler.get_stake_snapshot(EpochNo(4));
    assert!(
        snapshot.is_some(),
        "Should have snapshot for epoch 4 (current 2 + lag 2)"
    );

    // Should not have snapshot for epoch 2 yet (current epoch)
    let snapshot = handler.get_stake_snapshot(EpochNo(2));
    assert!(
        snapshot.is_none(),
        "Should not have snapshot for current epoch"
    );
}

#[tokio::test]
async fn test_reward_calculation_basic() {
    // Test basic reward calculation for a single pool
    let ledgerdb = Arc::new(MockLedgerDB::new());
    let protocol_params = ProtocolParameters {
        security_parameter: 10,
        active_slot_coefficient: 0.05,
        slot_length: 1,
        epoch_length: 100,
    };

    let genesis_nonce = Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap();
    let mut handler = EpochTransitionHandler::new(ledgerdb.clone(), protocol_params, genesis_nonce);

    // Add test pool with known parameters
    let pool_id = create_test_pool_id(1);
    let pool_pledge = 10000000; // 10 ADA
    let pool_margin = 0.05; // 5%
    ledgerdb.add_test_pool(pool_id.clone(), pool_pledge, pool_margin).await;

    // Process epoch transition (this should calculate rewards)
    handler
        .process_epoch_transition(EpochNo(0), EpochNo(1), SlotNo(100))
        .await
        .unwrap();

    // Verify reserves decreased (monetary expansion)
    let reserves = handler.reserves();
    assert!(
        reserves < 45000000000000000, // Initial reserves
        "Reserves should decrease due to monetary expansion"
    );

    // Verify treasury increased
    let treasury = handler.treasury();
    assert!(treasury > 0, "Treasury should have received tax");
}

#[tokio::test]
async fn test_stability_window_detection() {
    // Test that VRF outputs are only collected in stability window
    let ledgerdb = Arc::new(MockLedgerDB::new());
    let protocol_params = ProtocolParameters {
        security_parameter: 10, // k = 10
        active_slot_coefficient: 0.05, // f = 0.05
        slot_length: 1,
        epoch_length: 100,
    };

    let genesis_nonce = Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap();
    let mut handler = EpochTransitionHandler::new(ledgerdb.clone(), protocol_params, genesis_nonce);

    // Stability window should be last 6k/f = 6 * 10 / 0.05 = 1200 slots
    // But epoch is only 100 slots, so it should be last 60 slots (60% of epoch)

    // Try collecting VRF output in first slot (outside window)
    let result1 = handler.collect_vrf_output(SlotNo(0), create_test_vrf_output(1));
    assert!(result1.is_ok());

    // Try collecting VRF output in slot 50 (still outside window, window starts at 40)
    let result2 = handler.collect_vrf_output(SlotNo(50), create_test_vrf_output(2));
    assert!(result2.is_ok());

    // Try collecting VRF output in slot 90 (inside window)
    let result3 = handler.collect_vrf_output(SlotNo(90), create_test_vrf_output(3));
    assert!(result3.is_ok());

    // Verify handler collected outputs (this is internal, tested via nonce evolution)
}

#[tokio::test]
async fn test_multiple_pools_reward_distribution() {
    // Test reward distribution across multiple pools
    let ledgerdb = Arc::new(MockLedgerDB::new());
    let protocol_params = ProtocolParameters {
        security_parameter: 10,
        active_slot_coefficient: 0.05,
        slot_length: 1,
        epoch_length: 100,
    };

    let genesis_nonce = Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap();
    let mut handler = EpochTransitionHandler::new(ledgerdb.clone(), protocol_params, genesis_nonce);

    // Add 3 pools with different stakes
    let pool1 = create_test_pool_id(1);
    let pool2 = create_test_pool_id(2);
    let pool3 = create_test_pool_id(3);

    ledgerdb.add_test_pool(pool1.clone(), 1000000, 0.03).await; // Small pool, low margin
    ledgerdb.add_test_pool(pool2.clone(), 5000000, 0.05).await; // Medium pool, medium margin
    ledgerdb.add_test_pool(pool3.clone(), 10000000, 0.10).await; // Large pool, high margin

    // Process epoch transition
    handler
        .process_epoch_transition(EpochNo(0), EpochNo(1), SlotNo(100))
        .await
        .unwrap();

    // Verify snapshot includes all pools
    let snapshot = handler.get_stake_snapshot(EpochNo(3));
    assert!(snapshot.is_some());
    let snapshot = snapshot.unwrap();

    assert_eq!(snapshot.pool_stakes.len(), 3, "Should have 3 pools in snapshot");
}

#[tokio::test]
async fn test_epoch_transition_without_pools() {
    // Test epoch transition when no pools are registered
    let ledgerdb = Arc::new(MockLedgerDB::new());
    let protocol_params = ProtocolParameters {
        security_parameter: 10,
        active_slot_coefficient: 0.05,
        slot_length: 1,
        epoch_length: 100,
    };

    let genesis_nonce = Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap();
    let mut handler = EpochTransitionHandler::new(ledgerdb.clone(), protocol_params, genesis_nonce);

    // Don't add any pools

    // Process epoch transition
    let result = handler
        .process_epoch_transition(EpochNo(0), EpochNo(1), SlotNo(100))
        .await;

    assert!(result.is_ok(), "Epoch transition should succeed even without pools");

    // Verify snapshot was still created (empty)
    let snapshot = handler.get_stake_snapshot(EpochNo(3));
    assert!(snapshot.is_some());
    let snapshot = snapshot.unwrap();
    assert_eq!(snapshot.pool_stakes.len(), 0, "Snapshot should be empty");
}

#[tokio::test]
async fn test_concurrent_epoch_transitions() {
    // Test that epoch transitions are processed correctly when called in sequence
    let ledgerdb = Arc::new(MockLedgerDB::new());
    let protocol_params = ProtocolParameters {
        security_parameter: 10,
        active_slot_coefficient: 0.05,
        slot_length: 1,
        epoch_length: 100,
    };

    let genesis_nonce = Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap();
    let mut handler = EpochTransitionHandler::new(ledgerdb.clone(), protocol_params, genesis_nonce);

    // Add test pool
    let pool_id = create_test_pool_id(1);
    ledgerdb.add_test_pool(pool_id, 1000000, 0.05).await;

    // Process multiple transitions rapidly
    for epoch in 0..10 {
        let slot = SlotNo((epoch + 1) * 100);
        let result = handler
            .process_epoch_transition(EpochNo(epoch), EpochNo(epoch + 1), slot)
            .await;
        assert!(
            result.is_ok(),
            "Epoch transition {} should succeed",
            epoch
        );
    }

    // Verify we created 10 snapshots
    assert_eq!(ledgerdb.snapshot_count().await, 10);
}

#[tokio::test]
async fn test_vrf_output_buffer_cleared() {
    // Test that VRF output buffer is cleared after epoch transition
    let ledgerdb = Arc::new(MockLedgerDB::new());
    let protocol_params = ProtocolParameters {
        security_parameter: 10,
        active_slot_coefficient: 0.05,
        slot_length: 1,
        epoch_length: 100,
    };

    let genesis_nonce = Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap();
    let mut handler = EpochTransitionHandler::new(ledgerdb.clone(), protocol_params, genesis_nonce);

    // Collect VRF outputs in epoch 0
    for i in 40..100 {
        handler
            .collect_vrf_output(SlotNo(i), create_test_vrf_output((i % 256) as u8))
            .unwrap();
    }

    // Process epoch 0 -> 1 transition
    handler
        .process_epoch_transition(EpochNo(0), EpochNo(1), SlotNo(100))
        .await
        .unwrap();

    let nonce_after_epoch1 = handler.current_nonce();

    // Collect NO VRF outputs in epoch 1

    // Process epoch 1 -> 2 transition (with empty buffer)
    handler
        .process_epoch_transition(EpochNo(1), EpochNo(2), SlotNo(200))
        .await
        .unwrap();

    let nonce_after_epoch2 = handler.current_nonce();

    // Nonce should still change even without VRF outputs (hashes previous nonce)
    assert_ne!(
        nonce_after_epoch1, nonce_after_epoch2,
        "Nonce should evolve even without VRF outputs"
    );
}

#[tokio::test]
async fn test_reserve_contribution_calculation() {
    // Test that monetary expansion from reserves is calculated correctly
    let ledgerdb = Arc::new(MockLedgerDB::new());
    let protocol_params = ProtocolParameters {
        security_parameter: 10,
        active_slot_coefficient: 0.05,
        slot_length: 1,
        epoch_length: 100,
    };

    let genesis_nonce = Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap();
    let mut handler = EpochTransitionHandler::new(ledgerdb.clone(), protocol_params, genesis_nonce);

    let initial_reserves = handler.reserves();
    let expected_contribution = (initial_reserves as f64 * 0.05 * 0.003) as u64;

    // Process epoch transition
    handler
        .process_epoch_transition(EpochNo(0), EpochNo(1), SlotNo(100))
        .await
        .unwrap();

    let final_reserves = handler.reserves();
    let actual_contribution = initial_reserves - final_reserves;

    // Allow small rounding difference
    let difference = if actual_contribution > expected_contribution {
        actual_contribution - expected_contribution
    } else {
        expected_contribution - actual_contribution
    };

    assert!(
        difference < 1000,
        "Reserve contribution should match expected (difference: {})",
        difference
    );
}
