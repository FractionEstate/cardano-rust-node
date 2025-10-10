//! Block Production Tests
//!
//! Tests for the block production mechanism in the Ouroboros consensus protocol.
//! Covers slot leadership determination, block forging process, VRF evaluation,
//! operational certificate management, and KES (Key Evolving Signatures).

use cardano_consensus::{ConsensusError, Result};
use cardano_crypto::{
    Blake2b256Hash, Ed25519KeyHash, VrfOutput, VrfPrivateKey, VrfProof, VrfPublicKey,
    VRF_OUTPUT_LENGTH, VRF_PROOF_LENGTH,
};
use std::collections::HashMap;

#[path = "../common/mod.rs"]
mod common;

use common::vrf::{vrf_fixture_output, vrf_fixture_proof, vrf_private_key, vrf_public_key};

pub use crate::test_ouroboros_protocol::{
    BlockHeader, ConsensusState, EpochNumber, OperationalCertificate, PoolStake, Rational,
    SlotNumber, StakeDistribution,
};

/// Block producer with keys and configuration
#[derive(Debug, Clone)]
pub struct BlockProducer {
    pub pool_id: Ed25519KeyHash,
    pub vrf_key: VrfKey,
    pub kes_key: KesKey,
    pub cold_key: ColdKey,
    pub operational_cert: OperationalCertificate,
    pub stake: u64,
}

/// VRF (Verifiable Random Function) key for slot leadership
#[derive(Debug, Clone)]
pub struct VrfKey {
    pub public_key: VrfPublicKey,
    pub private_key: VrfPrivateKey,
}

/// KES (Key Evolving Signature) key for block signing
#[derive(Debug, Clone)]
pub struct KesKey {
    pub public_key: Blake2b256Hash,
    pub private_key: Blake2b256Hash,
    pub period: u64,     // Current KES period
    pub max_period: u64, // Maximum KES period before evolution
}

/// Cold key for operational certificate signing
#[derive(Debug, Clone)]
pub struct ColdKey {
    pub public_key: Ed25519KeyHash,
    pub private_key: Blake2b256Hash,
}

/// Block forging context
#[derive(Debug, Clone)]
pub struct ForgingContext {
    pub current_slot: SlotNumber,
    pub epoch_nonce: Blake2b256Hash,
    pub prev_block_hash: Blake2b256Hash,
    pub mempool: Vec<Transaction>, // Simplified transaction pool
    pub ledger_state: SimplifiedLedgerState,
}

/// Simplified transaction for testing
#[derive(Debug, Clone)]
pub struct Transaction {
    pub tx_id: Blake2b256Hash,
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
    pub fee: u64,
    pub size: u32, // Transaction size in bytes
}

#[derive(Debug, Clone)]
pub struct TxInput {
    pub tx_hash: Blake2b256Hash,
    pub output_index: u32,
}

#[derive(Debug, Clone)]
pub struct TxOutput {
    pub address: Blake2b256Hash,
    pub value: u64,
}

/// Simplified ledger state for block production
#[derive(Debug, Clone)]
pub struct SimplifiedLedgerState {
    pub utxo_set: HashMap<TxInput, TxOutput>,
    pub total_supply: u64,
    pub treasury: u64,
    pub reserves: u64,
}

/// Block forging result
#[derive(Debug, Clone)]
pub struct ForgedBlock {
    pub header: BlockHeader,
    pub body: BlockBody,
    pub proof_of_leadership: VrfProof,
}

/// Block body containing transactions
#[derive(Debug, Clone)]
pub struct BlockBody {
    pub transactions: Vec<Transaction>,
    pub total_fee: u64,
    pub total_size: u32,
}

impl VrfKey {
    pub fn new() -> Self {
        Self {
            public_key: vrf_public_key(),
            private_key: vrf_private_key(),
        }
    }

    /// Evaluate VRF for slot leadership
    pub fn evaluate_leadership(
        &self,
        slot: SlotNumber,
        epoch_nonce: &Blake2b256Hash,
    ) -> Result<(VrfOutput, VrfProof)> {
        let mut payload =
            Vec::with_capacity(epoch_nonce.as_bytes().len() + std::mem::size_of::<SlotNumber>());
        payload.extend_from_slice(epoch_nonce.as_bytes());
        payload.extend_from_slice(&slot.to_le_bytes());

        self.private_key.try_prove(&payload).map_err(|err| {
            ConsensusError::InvalidVrfProof(format!("VRF evaluation failed: {}", err))
        })
    }
}

impl KesKey {
    pub fn new(period: u64) -> Self {
        Self {
            public_key: Blake2b256Hash::new(&format!("kes_public_{}", period).as_bytes()),
            private_key: Blake2b256Hash::new(&format!("kes_private_{}", period).as_bytes()),
            period,
            max_period: 127, // Mirrors CompactSum7 (128 periods total)
        }
    }

    /// Check if KES key needs evolution
    pub fn needs_evolution(&self, current_period: u64) -> bool {
        current_period > self.period
    }

    /// Evolve KES key to new period
    pub fn evolve(&mut self, new_period: u64) -> Result<()> {
        if new_period > self.max_period {
            return Err(ConsensusError::KesKeyExpired(
                "KES key has reached maximum evolution".to_string(),
            ));
        }

        if new_period <= self.period {
            return Err(ConsensusError::InvalidKesEvolution(
                "Cannot evolve to past period".to_string(),
            ));
        }

        // Evolve keys (simplified - real KES involves cryptographic evolution)
        self.public_key = Blake2b256Hash::new(&format!("kes_public_{}", new_period).as_bytes());
        self.private_key = Blake2b256Hash::new(&format!("kes_private_{}", new_period).as_bytes());
        self.period = new_period;

        Ok(())
    }

    /// Sign block header with KES key
    pub fn sign_block(&self, header: &BlockHeader) -> Result<Blake2b256Hash> {
        // Verify KES key is valid for this period
        let expected_period = header.slot / 129600; // ~36 hours per KES period
        if self.period != expected_period {
            return Err(ConsensusError::InvalidKesKey(
                "KES key period mismatch".to_string(),
            ));
        }

        // Create signature (simplified)
        let header_bytes = format!("{:?}", header);
        let signature = Blake2b256Hash::new(&format!("kes_sig_{}", header_bytes).as_bytes());

        Ok(signature)
    }
}

impl BlockProducer {
    pub fn new(pool_id: Ed25519KeyHash, stake: u64) -> Self {
        let vrf_key = VrfKey::new();
        let kes_key = KesKey::new(0);
        let cold_key = ColdKey {
            public_key: pool_id,
            private_key: Blake2b256Hash::new(b"cold_private_key"),
        };

        let operational_cert = OperationalCertificate {
            hot_vkey: Ed25519KeyHash::new(b"hot_vkey"),
            sequence_number: 1,
            kes_period: 0,
            sigma: Blake2b256Hash::new(b"cold_signature"),
        };

        Self {
            pool_id,
            vrf_key,
            kes_key,
            cold_key,
            operational_cert,
            stake,
        }
    }

    /// Check if this producer is slot leader for given slot
    pub fn check_slot_leadership(
        &self,
        context: &ForgingContext,
        total_stake: u64,
        active_slot_coeff: f64,
    ) -> Result<Option<(VrfOutput, VrfProof)>> {
        // Evaluate VRF for slot leadership
        let (vrf_output, vrf_proof) = self
            .vrf_key
            .evaluate_leadership(context.current_slot, &context.epoch_nonce)?;

        // Calculate leadership threshold
        let relative_stake = self.stake as f64 / total_stake as f64;
        let threshold = 1.0 - (1.0 - active_slot_coeff).powf(relative_stake);

        // Convert VRF output to natural number in [0,1)
        let vrf_natural = self.vrf_output_to_natural(&vrf_output);

        if vrf_natural < threshold {
            Ok(Some((vrf_output, vrf_proof)))
        } else {
            Ok(None)
        }
    }

    fn vrf_output_to_natural(&self, vrf_output: &VrfOutput) -> f64 {
        let bytes = vrf_output.to_bytes();
        let mut value = 0u64;

        // Take first 8 bytes and convert to u64
        for (i, &byte) in bytes.iter().take(8).enumerate() {
            value |= (byte as u64) << (i * 8);
        }

        // Normalize to [0,1)
        if value == u64::MAX {
            1.0 - f64::EPSILON
        } else {
            (value as f64) / ((u64::MAX as f64) + 1.0)
        }
    }

    /// Forge a new block for the given slot
    pub fn forge_block(&mut self, context: &ForgingContext) -> Result<ForgedBlock> {
        // Check slot leadership
        let total_stake = 1_000_000_000_000_000; // 1 billion ADA (simplified)
        let active_slot_coeff = 0.05; // 5% active slot coefficient

        let (vrf_output, vrf_proof) = self
            .check_slot_leadership(context, total_stake, active_slot_coeff)?
            .ok_or_else(|| {
                ConsensusError::NotSlotLeader("Producer not elected for this slot".to_string())
            })?;
        let proof_of_leadership = vrf_proof.clone();

        // Select transactions from mempool
        let (selected_txs, total_fee) = self.select_transactions(&context.mempool)?;

        // Create block body
        let body = BlockBody {
            total_size: selected_txs.iter().map(|tx| tx.size).sum(),
            total_fee,
            transactions: selected_txs,
        };

        // Update KES key if needed
        let kes_period = context.current_slot / 129600;
        if self.kes_key.needs_evolution(kes_period) {
            self.kes_key.evolve(kes_period)?;

            // Update operational certificate
            self.operational_cert.kes_period = kes_period;
            self.operational_cert.sequence_number += 1;
        }

        // Create block header
        let header = BlockHeader {
            slot: context.current_slot,
            prev_hash: context.prev_block_hash,
            issuer_vkey: self.pool_id,
            vrf_proof,
            vrf_output,
            block_body_hash: body.hash(),
            block_size: body.total_size,
            operational_cert: self.operational_cert.clone(),
            protocol_magic: 764824073, // Mainnet magic
        };

        // Sign block with KES key
        let _signature = self.kes_key.sign_block(&header)?;

        Ok(ForgedBlock {
            header,
            body,
            proof_of_leadership,
        })
    }

    /// Select transactions from mempool for inclusion in block
    fn select_transactions(&self, mempool: &[Transaction]) -> Result<(Vec<Transaction>, u64)> {
        const MAX_BLOCK_SIZE: u32 = 90112; // ~88KB max block size
        const MAX_TRANSACTIONS: usize = 1000; // Reasonable transaction limit

        let mut selected = Vec::new();
        let mut total_size = 0u32;
        let mut total_fee = 0u64;

        // Sort transactions by fee density (fee per byte) - higher density first
        let mut sorted_txs: Vec<_> = mempool.iter().collect();
        sorted_txs.sort_by(|a, b| {
            let density_a = a.fee as f64 / a.size as f64;
            let density_b = b.fee as f64 / b.size as f64;
            density_b.partial_cmp(&density_a).unwrap()
        });

        // Select transactions greedily
        for tx in sorted_txs {
            if selected.len() >= MAX_TRANSACTIONS {
                break;
            }

            if total_size + tx.size > MAX_BLOCK_SIZE {
                break;
            }

            // Validate transaction (simplified)
            if self.validate_transaction(tx).is_ok() {
                selected.push(tx.clone());
                total_size += tx.size;
                total_fee += tx.fee;
            }
        }

        Ok((selected, total_fee))
    }

    /// Validate transaction for inclusion (simplified)
    fn validate_transaction(&self, tx: &Transaction) -> Result<()> {
        // Check basic constraints
        if tx.inputs.is_empty() {
            return Err(ConsensusError::InvalidTransaction(
                "Transaction has no inputs".to_string(),
            ));
        }

        if tx.outputs.is_empty() {
            return Err(ConsensusError::InvalidTransaction(
                "Transaction has no outputs".to_string(),
            ));
        }

        if tx.fee == 0 {
            return Err(ConsensusError::InvalidTransaction(
                "Transaction has zero fee".to_string(),
            ));
        }

        if tx.size == 0 || tx.size > 16384 {
            // Max 16KB per transaction
            return Err(ConsensusError::InvalidTransaction(
                "Invalid transaction size".to_string(),
            ));
        }

        Ok(())
    }
}

impl BlockBody {
    pub fn hash(&self) -> Blake2b256Hash {
        let body_data = format!("{:?}", self);
        Blake2b256Hash::new(body_data.as_bytes())
    }
}

impl SimplifiedLedgerState {
    pub fn new() -> Self {
        Self {
            utxo_set: HashMap::new(),
            total_supply: 45_000_000_000_000_000, // 45 billion ADA
            treasury: 1_000_000_000_000_000,      // 1 billion ADA
            reserves: 14_000_000_000_000_000,     // 14 billion ADA
        }
    }
}

/// Block production scheduler
pub struct ProductionScheduler {
    producers: HashMap<Ed25519KeyHash, BlockProducer>,
    schedule: HashMap<SlotNumber, Ed25519KeyHash>, // slot -> producer mapping
}

impl ProductionScheduler {
    const MAINNET_EPOCH_LENGTH: SlotNumber = 432_000;

    pub fn new() -> Self {
        Self {
            producers: HashMap::new(),
            schedule: HashMap::new(),
        }
    }

    /// Add block producer to scheduler
    pub fn add_producer(&mut self, producer: BlockProducer) {
        self.producers.insert(producer.pool_id, producer);
    }

    /// Calculate slot leadership schedule for an epoch
    pub fn calculate_epoch_schedule(
        &mut self,
        epoch: EpochNumber,
        stake_distribution: &StakeDistribution,
    ) -> Result<()> {
        self.calculate_epoch_schedule_with_length(
            epoch,
            stake_distribution,
            Self::MAINNET_EPOCH_LENGTH,
        )
    }

    pub fn calculate_epoch_schedule_with_length(
        &mut self,
        epoch: EpochNumber,
        stake_distribution: &StakeDistribution,
        epoch_length: SlotNumber,
    ) -> Result<()> {
        assert!(epoch_length > 0, "epoch length must be positive");

        let start_slot = epoch * epoch_length;
        let end_slot = start_slot + epoch_length - 1;

        // Clear previous schedule
        self.schedule.clear();

        // For each slot in the epoch
        for slot in start_slot..=end_slot {
            // Check each producer for slot leadership
            for (pool_id, producer) in &self.producers {
                let context = ForgingContext {
                    current_slot: slot,
                    epoch_nonce: Blake2b256Hash::new(&format!("epoch_{}", epoch).as_bytes()),
                    prev_block_hash: Blake2b256Hash::new(b"prev_hash"),
                    mempool: vec![],
                    ledger_state: SimplifiedLedgerState::new(),
                };

                let total_stake = stake_distribution.total_stake;
                let active_slot_coeff = 0.05;

                if let Ok(Some((_output, _proof))) =
                    producer.check_slot_leadership(&context, total_stake, active_slot_coeff)
                {
                    self.schedule.insert(slot, *pool_id);
                    break; // First producer wins (simplified - real protocol handles ties differently)
                }
            }
        }

        Ok(())
    }

    /// Get scheduled producer for a slot
    pub fn get_slot_leader(&self, slot: SlotNumber) -> Option<&Ed25519KeyHash> {
        self.schedule.get(&slot)
    }

    /// Get producer statistics
    pub fn get_producer_stats(&self) -> HashMap<Ed25519KeyHash, u32> {
        let mut stats = HashMap::new();

        for producer_id in self.schedule.values() {
            *stats.entry(*producer_id).or_insert(0) += 1;
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uniform_vrf_output(byte: u8) -> VrfOutput {
        let bytes = vec![byte; VRF_OUTPUT_LENGTH];
        VrfOutput::from_bytes(&bytes).expect("uniform VRF output is valid")
    }

    fn create_test_producer() -> BlockProducer {
        let pool_id = Ed25519KeyHash::new(b"test_pool");
        BlockProducer::new(pool_id, 1_000_000_000_000) // 1M ADA stake
    }

    fn create_test_context(slot: SlotNumber) -> ForgingContext {
        ForgingContext {
            current_slot: slot,
            epoch_nonce: Blake2b256Hash::new(b"test_epoch_nonce"),
            prev_block_hash: Blake2b256Hash::new(b"prev_block_hash"),
            mempool: vec![Transaction {
                tx_id: Blake2b256Hash::new(b"tx1"),
                inputs: vec![TxInput {
                    tx_hash: Blake2b256Hash::new(b"input_tx"),
                    output_index: 0,
                }],
                outputs: vec![TxOutput {
                    address: Blake2b256Hash::new(b"output_addr"),
                    value: 1_000_000,
                }],
                fee: 200_000,
                size: 300,
            }],
            ledger_state: SimplifiedLedgerState::new(),
        }
    }

    #[test]
    fn test_vrf_evaluation() {
        let vrf_key = VrfKey::new();
        let epoch_nonce = Blake2b256Hash::new(b"test_nonce");

        let result = vrf_key.evaluate_leadership(1000, &epoch_nonce);
        assert!(result.is_ok());

        let (output, proof) = result.unwrap();
        assert!(!output.as_bytes().is_empty());
        assert!(!proof.as_bytes().is_empty());
    }

    #[test]
    fn test_kes_key_evolution() {
        let mut kes_key = KesKey::new(0);

        // Key should need evolution for future periods
        assert!(kes_key.needs_evolution(1));
        assert!(!kes_key.needs_evolution(0));

        // Evolve to period 1
        let result = kes_key.evolve(1);
        assert!(result.is_ok());
        assert_eq!(kes_key.period, 1);

        // Cannot evolve backwards
        let backwards_result = kes_key.evolve(0);
        assert!(backwards_result.is_err());
    }

    #[test]
    fn test_kes_key_expiration() {
        let mut kes_key = KesKey::new(89); // Near max period

        // Should be able to evolve to max period
        let result = kes_key.evolve(90);
        assert!(result.is_ok());

        // Should fail to evolve beyond max period
        let expired_result = kes_key.evolve(91);
        assert!(expired_result.is_err());
        assert!(matches!(
            expired_result.unwrap_err(),
            ConsensusError::KesKeyExpired(_)
        ));
    }

    #[test]
    fn test_slot_leadership_check() {
        let producer = create_test_producer();
        let context = create_test_context(1000);

        let total_stake = 100_000_000_000_000; // 100M ADA total
        let active_slot_coeff = 0.05;

        // This is probabilistic, but with 1M ADA out of 100M total stake,
        // the producer has 1% chance per slot. We can't guarantee the result
        // but we can test that the function executes without error.
        let result = producer.check_slot_leadership(&context, total_stake, active_slot_coeff);
        assert!(result.is_ok());

        // Result will be Some((output, proof)) if leader, None if not leader
        match result.unwrap() {
            Some((output, proof)) => {
                assert_eq!(output.to_bytes().len(), VRF_OUTPUT_LENGTH);
                assert_eq!(proof.to_bytes().len(), VRF_PROOF_LENGTH);
                println!("Producer elected as slot leader!");
            }
            None => {
                println!("Producer not elected for this slot");
            }
        }
    }

    #[test]
    fn test_transaction_selection() {
        let producer = create_test_producer();

        // Create mempool with various transactions
        let mempool = vec![
            Transaction {
                tx_id: Blake2b256Hash::new(b"high_fee_tx"),
                inputs: vec![TxInput {
                    tx_hash: Blake2b256Hash::new(b"in1"),
                    output_index: 0,
                }],
                outputs: vec![TxOutput {
                    address: Blake2b256Hash::new(b"out1"),
                    value: 1000000,
                }],
                fee: 500_000, // High fee
                size: 200,
            },
            Transaction {
                tx_id: Blake2b256Hash::new(b"low_fee_tx"),
                inputs: vec![TxInput {
                    tx_hash: Blake2b256Hash::new(b"in2"),
                    output_index: 0,
                }],
                outputs: vec![TxOutput {
                    address: Blake2b256Hash::new(b"out2"),
                    value: 2000000,
                }],
                fee: 100_000, // Low fee
                size: 300,
            },
            Transaction {
                tx_id: Blake2b256Hash::new(b"invalid_tx"),
                inputs: vec![], // No inputs - invalid
                outputs: vec![TxOutput {
                    address: Blake2b256Hash::new(b"out3"),
                    value: 1000000,
                }],
                fee: 200_000,
                size: 250,
            },
        ];

        let (selected, total_fee) = producer.select_transactions(&mempool).unwrap();

        // Should select 2 valid transactions (excluding the invalid one)
        assert_eq!(selected.len(), 2);
        assert_eq!(total_fee, 600_000); // 500k + 100k

        // Should prioritize high fee density transaction first
        assert_eq!(selected[0].fee, 500_000); // High fee tx first
        assert_eq!(selected[1].fee, 100_000); // Low fee tx second
    }

    #[test]
    fn test_block_forging_without_leadership() {
        let mut producer = create_test_producer();
        let context = create_test_context(1000);

        // Use very high total stake so producer is very unlikely to be leader
        let original_stake = producer.stake;
        producer.stake = 1; // 1 lovelace out of huge total stake

        let result = producer.forge_block(&context);

        // Should fail because producer is not slot leader
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConsensusError::NotSlotLeader(_)
        ));

        // Restore original stake
        producer.stake = original_stake;
    }

    #[test]
    fn test_production_scheduler() {
        let mut scheduler = ProductionScheduler::new();

        // Add test producers
        let producer1 = BlockProducer::new(Ed25519KeyHash::new(b"pool1"), 10_000_000_000_000);
        let producer2 = BlockProducer::new(Ed25519KeyHash::new(b"pool2"), 5_000_000_000_000);

        scheduler.add_producer(producer1);
        scheduler.add_producer(producer2);

        // Create stake distribution
        let mut stake_dist = StakeDistribution::new(1);
        stake_dist.total_stake = 15_000_000_000_000; // Sum of both producers

        // Calculate schedule for a small epoch (just a few slots for testing)
        // Note: This is probabilistic, so we mainly test that it doesn't crash
        let result = scheduler.calculate_epoch_schedule_with_length(1, &stake_dist, 8);
        assert!(result.is_ok());

        // Get statistics
        let stats = scheduler.get_producer_stats();
        println!("Producer statistics: {:?}", stats);

        // Should have some assignments (though exact count is probabilistic)
        let total_assignments: u32 = stats.values().sum();
        println!("Total slot assignments: {}", total_assignments);
    }

    #[test]
    fn test_vrf_output_to_natural_range() {
        let producer = create_test_producer();

        // Test with different VRF outputs
        let outputs = vec![
            uniform_vrf_output(0),
            uniform_vrf_output(0xFF),
            uniform_vrf_output(0x80),
        ];

        for output in outputs {
            let natural = producer.vrf_output_to_natural(&output);
            assert!(
                natural >= 0.0 && natural < 1.0,
                "VRF natural should be in [0,1), got {}",
                natural
            );
        }
    }

    #[test]
    fn test_kes_key_signing() {
        let kes_key = KesKey::new(0);
        let producer = create_test_producer();

        // Create header for slot in period 0
        let header = BlockHeader {
            slot: 50000, // Should be in KES period 0
            prev_hash: Blake2b256Hash::new(b"prev"),
            issuer_vkey: producer.pool_id,
            vrf_proof: vrf_fixture_proof("kes-proof"),
            vrf_output: vrf_fixture_output("kes-output"),
            block_body_hash: Blake2b256Hash::new(b"body"),
            block_size: 1000,
            operational_cert: producer.operational_cert.clone(),
            protocol_magic: 764824073,
        };

        let signature = kes_key.sign_block(&header);
        assert!(signature.is_ok());

        // Test signing with wrong period
        let wrong_period_header = BlockHeader {
            slot: 200000, // Should be in KES period 1
            ..header
        };

        let wrong_sig = kes_key.sign_block(&wrong_period_header);
        assert!(wrong_sig.is_err());
        assert!(matches!(
            wrong_sig.unwrap_err(),
            ConsensusError::InvalidKesKey(_)
        ));
    }

    #[test]
    fn test_transaction_validation() {
        let producer = create_test_producer();

        // Valid transaction
        let valid_tx = Transaction {
            tx_id: Blake2b256Hash::new(b"valid"),
            inputs: vec![TxInput {
                tx_hash: Blake2b256Hash::new(b"in"),
                output_index: 0,
            }],
            outputs: vec![TxOutput {
                address: Blake2b256Hash::new(b"out"),
                value: 1000000,
            }],
            fee: 200_000,
            size: 300,
        };

        assert!(producer.validate_transaction(&valid_tx).is_ok());

        // Invalid transactions
        let no_inputs = Transaction {
            inputs: vec![],
            ..valid_tx.clone()
        };
        assert!(producer.validate_transaction(&no_inputs).is_err());

        let no_outputs = Transaction {
            outputs: vec![],
            ..valid_tx.clone()
        };
        assert!(producer.validate_transaction(&no_outputs).is_err());

        let zero_fee = Transaction {
            fee: 0,
            ..valid_tx.clone()
        };
        assert!(producer.validate_transaction(&zero_fee).is_err());

        let oversized = Transaction {
            size: 20000,
            ..valid_tx.clone()
        }; // > 16KB
        assert!(producer.validate_transaction(&oversized).is_err());
    }
}
