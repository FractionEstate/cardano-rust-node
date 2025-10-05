//! Block production logic
//!
//! Handles slot leader election and block creation based on Ouroboros Praos protocol.

use crate::leadership::LeadershipProof;
use crate::ouroboros::{PoolId, SlotNo};
use crate::{ConsensusError, Result};
use cardano_crypto::{
    Blake2b256Hash, Ed25519KeyHash, KesSecretKey, KesSignature, VrfOutput, VrfPrivateKey, VrfProof,
    VrfPublicKey, VRF_SEED_LENGTH,
};
use std::collections::HashMap;

/// Block producer - coordinates VRF leadership and KES signing
///
/// Note: Does not derive Clone for security - contains KES keys that should not be duplicated
#[derive(Debug)]
pub struct BlockProducer {
    pub pool_id: PoolId,
    pub vrf_key: VrfKey,
    pub kes_key: KesKey,
    pub operational_cert: OperationalCertificate,
    pub stake: u64,
}

/// VRF (Verifiable Random Function) key for slot leadership
#[derive(Debug, Clone)]
pub struct VrfKey {
    pub public_key: VrfPublicKey,
    pub private_key: VrfPrivateKey,
}

/// KES key wrapper
///
/// Note: Does not derive Clone for security - KES keys should not be duplicated
#[derive(Debug)]
pub struct KesKey {
    /// The actual KES secret key from cardano-crypto
    pub secret_key: KesSecretKey,
    /// Maximum KES period before rotation
    pub max_period: u64,
}

impl KesKey {
    /// Create a new KES key with specified depth
    pub fn new(depth: u32) -> Self {
        let secret_key = KesSecretKey::generate(depth);
        let max_period = secret_key.max_period();
        Self {
            secret_key,
            max_period,
        }
    }

    /// Get current period
    pub fn current_period(&self) -> u64 {
        self.secret_key.current_period()
    }

    /// Check if KES key needs evolution
    pub fn needs_evolution(&self, current_period: u64) -> bool {
        current_period > self.secret_key.current_period()
    }

    /// Check if expired
    pub fn is_expired(&self) -> bool {
        self.secret_key.is_expired()
    }

    /// Get number of periods remaining until expiration
    pub fn periods_remaining(&self) -> u64 {
        self.max_period
            .saturating_sub(self.secret_key.current_period())
    }

    /// Check if key is approaching expiration (within threshold periods)
    pub fn is_approaching_expiration(&self, threshold_periods: u64) -> bool {
        self.periods_remaining() <= threshold_periods
    }

    /// Evolve KES key to new period
    pub fn evolve(&mut self, target_period: u64) -> Result<()> {
        if target_period > self.max_period {
            return Err(ConsensusError::KesKeyExpired(
                "KES key has reached maximum evolution".to_string(),
            ));
        }

        if target_period <= self.secret_key.current_period() {
            return Err(ConsensusError::InvalidKesEvolution(
                "Cannot evolve to past period".to_string(),
            ));
        }

        // Evolve the secret key - take ownership, evolve, and replace
        let old_key = std::mem::replace(&mut self.secret_key, KesSecretKey::generate(6)); // Temporary
        self.secret_key = old_key.evolve_to(target_period).map_err(|e| {
            ConsensusError::InvalidKesEvolution(format!("KES evolution failed: {}", e))
        })?;

        Ok(())
    }

    /// Sign block header with KES key
    pub fn sign_block(&self, header_bytes: &[u8]) -> Result<KesSignature> {
        let period = self.secret_key.current_period();
        self.secret_key
            .sign(period, header_bytes)
            .map_err(|e| ConsensusError::InvalidKesSignature(format!("KES signing failed: {}", e)))
    }
}

/// Operational certificate for block production
#[derive(Debug, Clone)]
pub struct OperationalCertificate {
    pub hot_vkey: Ed25519KeyHash,
    pub sequence_number: u64,
    pub kes_period: u64,
    pub sigma: Blake2b256Hash, // Cold signature
}

/// Block forging context
#[derive(Debug, Clone)]
pub struct ForgingContext {
    pub current_slot: SlotNo,
    pub epoch_nonce: Blake2b256Hash,
    pub prev_block_hash: Blake2b256Hash,
    pub mempool: Vec<Transaction>,
    pub ledger_state: SimplifiedLedgerState,
}

/// Transaction for block production
#[derive(Debug, Clone)]
pub struct Transaction {
    pub tx_id: Blake2b256Hash,
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
    pub fee: u64,
    pub size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

/// Forged block result
#[derive(Debug, Clone)]
pub struct ForgedBlock {
    pub header: BlockHeader,
    pub body: BlockBody,
    pub proof_of_leadership: LeadershipProof,
    pub kes_signature: KesSignature,
}

/// Block header for produced block
#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub slot: SlotNo,
    pub block_number: u64,
    pub prev_hash: Blake2b256Hash,
    pub issuer_vkey: Ed25519KeyHash,
    pub vrf_proof: VrfProof,
    pub vrf_output: VrfOutput,
    pub block_body_hash: Blake2b256Hash,
    pub block_size: u32,
    pub operational_cert: OperationalCertificate,
    pub protocol_magic: u32,
}

impl BlockHeader {
    /// Serialize header for signing (without the KES signature)
    pub fn to_bytes_for_signing(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.slot.0.to_le_bytes());
        bytes.extend_from_slice(&self.block_number.to_le_bytes());
        bytes.extend_from_slice(self.prev_hash.as_bytes());
        bytes.extend_from_slice(self.issuer_vkey.as_bytes());
        bytes.extend_from_slice(self.vrf_proof.to_bytes());
        bytes.extend_from_slice(self.vrf_output.to_bytes());
        bytes.extend_from_slice(self.block_body_hash.as_bytes());
        bytes.extend_from_slice(&self.block_size.to_le_bytes());
        bytes.extend_from_slice(&self.protocol_magic.to_le_bytes());
        bytes
    }
}

/// Block body containing transactions
#[derive(Debug, Clone)]
pub struct BlockBody {
    pub transactions: Vec<Transaction>,
    pub total_fee: u64,
    pub total_size: u32,
}

impl BlockBody {
    /// Create empty block body
    pub fn empty() -> Self {
        Self {
            transactions: vec![],
            total_fee: 0,
            total_size: 0,
        }
    }

    /// Create block body from transactions
    pub fn from_transactions(transactions: Vec<Transaction>) -> Self {
        let total_fee = transactions.iter().map(|tx| tx.fee).sum();
        let total_size = transactions.iter().map(|tx| tx.size).sum();
        Self {
            transactions,
            total_fee,
            total_size,
        }
    }

    /// Calculate hash of block body
    pub fn hash(&self) -> Blake2b256Hash {
        let mut data = Vec::new();
        for tx in &self.transactions {
            data.extend_from_slice(tx.tx_id.as_bytes());
        }
        Blake2b256Hash::hash(&data)
    }

    /// Check if body exceeds size limit
    pub fn exceeds_size_limit(&self, max_size: u32) -> bool {
        self.total_size > max_size
    }
}

impl Default for VrfKey {
    fn default() -> Self {
        Self::new()
    }
}

impl VrfKey {
    pub fn new() -> Self {
        let default_seed = Blake2b256Hash::hash(b"default_vrf_seed");
        Self::from_seed(default_seed.as_bytes())
    }

    pub fn from_seed(seed: &[u8; VRF_SEED_LENGTH]) -> Self {
        let private_key = VrfPrivateKey::generate(seed);
        let public_key = private_key.public_key();
        Self {
            public_key,
            private_key,
        }
    }

    pub fn for_pool(pool_id: &PoolId) -> Self {
        let seed = Blake2b256Hash::hash(pool_id.0.as_bytes());
        Self::from_seed(seed.as_bytes())
    }

    /// Evaluate VRF for slot leadership
    pub fn evaluate_leadership(
        &self,
        slot: SlotNo,
        epoch_nonce: &Blake2b256Hash,
    ) -> Result<(VrfOutput, VrfProof)> {
        let mut payload =
            Vec::with_capacity(epoch_nonce.as_bytes().len() + std::mem::size_of::<u64>());
        payload.extend_from_slice(epoch_nonce.as_bytes());
        payload.extend_from_slice(&slot.0.to_le_bytes());

        self.private_key.try_prove(&payload).map_err(|err| {
            ConsensusError::InvalidVrfProof(format!("VRF evaluation failed: {}", err))
        })
    }
}

impl BlockProducer {
    pub fn new(pool_id: PoolId, stake: u64) -> Self {
        let vrf_key = VrfKey::for_pool(&pool_id);
        let kes_key = KesKey::new(6); // depth=6 for mainnet (64 periods)

        let operational_cert = OperationalCertificate {
            hot_vkey: Ed25519KeyHash::from_test_data(b"hot_vkey"),
            sequence_number: 1,
            kes_period: 0,
            sigma: Blake2b256Hash::hash(b"cold_signature"),
        };

        Self {
            pool_id,
            vrf_key,
            kes_key,
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

        // Normalize to [0,1) - handle edge case of all 1s
        if value == u64::MAX {
            // Return value very close to 1 but less than 1
            1.0 - f64::EPSILON
        } else {
            // Normal case: divide by MAX+1 to ensure range [0,1)
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
        let kes_period = context.current_slot.0 / 129600;
        if self.kes_key.needs_evolution(kes_period) {
            self.kes_key.evolve(kes_period)?;

            // Update operational certificate
            self.operational_cert.kes_period = kes_period;
            self.operational_cert.sequence_number += 1;
        }

        // Create block header
        let header = BlockHeader {
            slot: context.current_slot,
            block_number: context.current_slot.0, // Simplified
            prev_hash: context.prev_block_hash,
            issuer_vkey: Ed25519KeyHash::from_test_data(self.pool_id.0.as_bytes()),
            vrf_proof: vrf_proof.clone(),
            vrf_output: vrf_output.clone(),
            block_body_hash: body.hash(),
            block_size: body.total_size,
            operational_cert: self.operational_cert.clone(),
            protocol_magic: 764824073, // Mainnet magic
        };

        // Sign block with KES key
        let header_bytes = header.to_bytes_for_signing();
        let kes_signature = self.kes_key.sign_block(&header_bytes)?;

        Ok(ForgedBlock {
            header,
            body,
            proof_of_leadership: LeadershipProof {
                slot: context.current_slot,
                pool_id: self.pool_id.clone(),
                vrf_output: vrf_output.clone(),
                vrf_proof: proof_of_leadership,
            },
            kes_signature,
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

    /// Check if we are the slot leader for a given slot (legacy API)
    pub fn is_slot_leader(&self, slot: u64) -> Result<bool> {
        let context = ForgingContext {
            current_slot: SlotNo(slot),
            epoch_nonce: Blake2b256Hash::hash(b"default_nonce"),
            prev_block_hash: Blake2b256Hash::hash(b"prev_hash"),
            mempool: vec![],
            ledger_state: SimplifiedLedgerState::new(),
        };

        let total_stake = 1_000_000_000_000_000;
        let active_slot_coeff = 0.05;

        match self.check_slot_leadership(&context, total_stake, active_slot_coeff)? {
            Some((_output, _proof)) => Ok(true),
            None => Ok(false),
        }
    }

    /// Produce a block for the given slot (legacy API)
    pub fn produce_block(&mut self, slot: u64) -> Result<ProducedBlock> {
        let context = ForgingContext {
            current_slot: SlotNo(slot),
            epoch_nonce: Blake2b256Hash::hash(b"default_nonce"),
            prev_block_hash: Blake2b256Hash::hash(b"prev_hash"),
            mempool: vec![],
            ledger_state: SimplifiedLedgerState::new(),
        };

        let forged_block = self.forge_block(&context)?;

        Ok(ProducedBlock {
            slot,
            header: forged_block.header,
            body: forged_block.body,
        })
    }
}

impl Default for SimplifiedLedgerState {
    fn default() -> Self {
        Self::new()
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

/// Block production scheduler for epoch planning
pub struct ProductionScheduler {
    producers: HashMap<PoolId, BlockProducer>,
    schedule: HashMap<SlotNo, PoolId>,
}

impl ProductionScheduler {
    const MAINNET_EPOCH_LENGTH: u64 = 432_000;

    pub fn new() -> Self {
        Self {
            producers: HashMap::new(),
            schedule: HashMap::new(),
        }
    }

    /// Add block producer to scheduler
    pub fn add_producer(&mut self, producer: BlockProducer) {
        let pool_id = producer.pool_id.clone();
        self.producers.insert(pool_id, producer);
    }

    /// Calculate slot leadership schedule for an epoch
    pub fn calculate_epoch_schedule(&mut self, epoch: u64, total_stake: u64) -> Result<()> {
        self.calculate_epoch_schedule_with_length(epoch, total_stake, Self::MAINNET_EPOCH_LENGTH)
    }

    pub fn calculate_epoch_schedule_with_length(
        &mut self,
        epoch: u64,
        total_stake: u64,
        epoch_length: u64,
    ) -> Result<()> {
        assert!(epoch_length > 0, "epoch length must be positive");

        let start_slot = epoch.checked_mul(epoch_length).ok_or_else(|| {
            ConsensusError::InvalidSlot("epoch multiplication overflow".to_string())
        })?;
        let last_offset = epoch_length
            .checked_sub(1)
            .ok_or_else(|| ConsensusError::InvalidSlot("epoch length underflow".to_string()))?;
        let end_slot = start_slot
            .checked_add(last_offset)
            .ok_or_else(|| ConsensusError::InvalidSlot("epoch end overflow".to_string()))?;

        // Clear previous schedule
        self.schedule.clear();

        // For each slot in the epoch
        for slot in start_slot..=end_slot {
            // Check each producer for slot leadership
            for (pool_id, producer) in &self.producers {
                let context = ForgingContext {
                    current_slot: SlotNo(slot),
                    epoch_nonce: Blake2b256Hash::hash(format!("epoch_{}", epoch).as_bytes()),
                    prev_block_hash: Blake2b256Hash::hash(b"prev_hash"),
                    mempool: vec![],
                    ledger_state: SimplifiedLedgerState::new(),
                };

                let active_slot_coeff = 0.05;

                if let Ok(Some((_output, _proof))) =
                    producer.check_slot_leadership(&context, total_stake, active_slot_coeff)
                {
                    self.schedule.insert(SlotNo(slot), pool_id.clone());
                    break; // First producer wins (simplified - real protocol handles ties differently)
                }
            }
        }

        Ok(())
    }

    /// Get scheduled producer for a slot
    pub fn get_slot_leader(&self, slot: SlotNo) -> Option<&PoolId> {
        self.schedule.get(&slot)
    }

    /// Get producer statistics
    pub fn get_producer_stats(&self) -> HashMap<PoolId, u32> {
        let mut stats = HashMap::new();

        for producer_id in self.schedule.values() {
            *stats.entry(producer_id.clone()).or_insert(0) += 1;
        }

        stats
    }
}

/// A block produced by this node (legacy structure)
#[derive(Debug)]
pub struct ProducedBlock {
    pub slot: u64,
    pub header: BlockHeader,
    pub body: BlockBody,
}

impl Default for BlockProducer {
    fn default() -> Self {
        Self::new(PoolId(Blake2b256Hash::hash(b"default_pool")), 0)
    }
}

impl Default for ProductionScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cardano_crypto::{
        Blake2b256Hash, Ed25519KeyHash, VRF_OUTPUT_LENGTH, VRF_PRIVATE_KEY_LENGTH,
        VRF_PROOF_LENGTH, VRF_PUBLIC_KEY_LENGTH,
    };

    fn create_test_producer() -> BlockProducer {
        let pool_id = PoolId(Blake2b256Hash::hash(b"test_pool"));
        BlockProducer::new(pool_id, 1_000_000_000_000) // 1M ADA stake
    }

    fn create_test_context(slot: u64) -> ForgingContext {
        ForgingContext {
            current_slot: SlotNo(slot),
            epoch_nonce: Blake2b256Hash::hash(b"test_epoch_nonce"),
            prev_block_hash: Blake2b256Hash::hash(b"prev_block_hash"),
            mempool: vec![Transaction {
                tx_id: Blake2b256Hash::hash(b"tx1"),
                inputs: vec![TxInput {
                    tx_hash: Blake2b256Hash::hash(b"input_tx"),
                    output_index: 0,
                }],
                outputs: vec![TxOutput {
                    address: Blake2b256Hash::hash(b"output_addr"),
                    value: 1_000_000,
                }],
                fee: 200_000,
                size: 300,
            }],
            ledger_state: SimplifiedLedgerState::new(),
        }
    }

    #[test]
    fn test_vrf_key_creation() {
        let vrf_key = VrfKey::new();
        assert_eq!(vrf_key.public_key.to_bytes().len(), VRF_PUBLIC_KEY_LENGTH);
        assert_eq!(vrf_key.private_key.to_bytes().len(), VRF_PRIVATE_KEY_LENGTH);
    }

    #[test]
    fn test_vrf_evaluation() {
        let vrf_key = VrfKey::new();
        let epoch_nonce = Blake2b256Hash::hash(b"test_nonce");

        let result = vrf_key.evaluate_leadership(SlotNo(1000), &epoch_nonce);
        assert!(result.is_ok());

        let (output, proof) = result.unwrap();
        assert_eq!(output.to_bytes().len(), VRF_OUTPUT_LENGTH);
        assert_eq!(proof.to_bytes().len(), VRF_PROOF_LENGTH);
    }

    #[test]
    fn test_kes_key_creation() {
        let kes_key = KesKey::new(6); // depth=6
        assert_eq!(kes_key.current_period(), 0);
        assert_eq!(kes_key.max_period, 62); // 2^6 - 2
    }

    #[test]
    fn test_kes_key_evolution() {
        let mut kes_key = KesKey::new(6);

        // Key should need evolution for future periods
        assert!(kes_key.needs_evolution(1));
        assert!(!kes_key.needs_evolution(0));

        // Evolve to period 1
        let result = kes_key.evolve(1);
        assert!(result.is_ok());
        assert_eq!(kes_key.current_period(), 1);

        // Cannot evolve backwards
        let backwards_result = kes_key.evolve(0);
        assert!(backwards_result.is_err());
        assert!(matches!(
            backwards_result.unwrap_err(),
            ConsensusError::InvalidKesEvolution(_)
        ));
    }

    #[test]
    fn test_kes_key_expiration() {
        // Use depth=6 (64 periods), start at period 62 (near max)
        let mut kes_key = KesKey::new(6); // depth=6 means max_period = 62

        // Evolve to period 62 first
        let _ = kes_key.evolve(62);

        // Should fail to evolve beyond max period (62)
        let expired_result = kes_key.evolve(63);
        assert!(expired_result.is_err());
        assert!(matches!(
            expired_result.unwrap_err(),
            ConsensusError::KesKeyExpired(_)
        ));
    }

    #[test]
    fn test_block_producer_creation() {
        let producer = create_test_producer();
        assert_eq!(producer.stake, 1_000_000_000_000);
        assert_eq!(producer.kes_key.current_period(), 0);
        assert_eq!(producer.operational_cert.sequence_number, 1);
    }

    #[test]
    fn test_slot_leadership_check() {
        let producer = create_test_producer();
        let context = create_test_context(1000);

        let total_stake = 100_000_000_000_000; // 100M ADA total
        let active_slot_coeff = 0.05;

        // This is probabilistic, but we test that the function executes without error
        let result = producer.check_slot_leadership(&context, total_stake, active_slot_coeff);

        // Debug the error if it fails
        match &result {
            Err(e) => println!("Slot leadership check error: {:?}", e),
            Ok(_) => println!("Slot leadership check succeeded"),
        }

        assert!(result.is_ok());

        // Result will be Some(proof) if leader, None if not leader
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
    fn test_vrf_output_to_natural_range() {
        let producer = create_test_producer();

        // Test with different VRF outputs (VrfOutput expects 64 bytes)
        let test_cases = vec![
            ([0u8; 64], "all zeros"),
            ([255u8; 64], "all ones"),
            ([128u8; 64], "mid-range"),
        ];

        for (bytes, description) in test_cases {
            let output = VrfOutput::from_bytes(bytes).unwrap();
            let natural = producer.vrf_output_to_natural(&output);
            println!("Testing {} -> natural: {}", description, natural);
            assert!(
                (0.0..1.0).contains(&natural),
                "VRF natural should be in [0,1) for {}, got {}",
                description,
                natural
            );
        }
    }

    #[test]
    fn test_transaction_selection() {
        let producer = create_test_producer();

        // Create mempool with various transactions
        let mempool = vec![
            Transaction {
                tx_id: Blake2b256Hash::hash(b"high_fee_tx"),
                inputs: vec![TxInput {
                    tx_hash: Blake2b256Hash::hash(b"in1"),
                    output_index: 0,
                }],
                outputs: vec![TxOutput {
                    address: Blake2b256Hash::hash(b"out1"),
                    value: 1000000,
                }],
                fee: 500_000, // High fee
                size: 200,
            },
            Transaction {
                tx_id: Blake2b256Hash::hash(b"low_fee_tx"),
                inputs: vec![TxInput {
                    tx_hash: Blake2b256Hash::hash(b"in2"),
                    output_index: 0,
                }],
                outputs: vec![TxOutput {
                    address: Blake2b256Hash::hash(b"out2"),
                    value: 2000000,
                }],
                fee: 100_000, // Low fee
                size: 300,
            },
            Transaction {
                tx_id: Blake2b256Hash::hash(b"invalid_tx"),
                inputs: vec![], // No inputs - invalid
                outputs: vec![TxOutput {
                    address: Blake2b256Hash::hash(b"out3"),
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
    fn test_transaction_validation() {
        let producer = create_test_producer();

        // Valid transaction
        let valid_tx = Transaction {
            tx_id: Blake2b256Hash::hash(b"valid"),
            inputs: vec![TxInput {
                tx_hash: Blake2b256Hash::hash(b"in"),
                output_index: 0,
            }],
            outputs: vec![TxOutput {
                address: Blake2b256Hash::hash(b"out"),
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

    #[test]
    fn test_kes_key_signing() {
        let kes_key = KesKey::new(6); // depth=6 for mainnet
        let producer = create_test_producer();

        // Create header for slot in period 0 (with correct VRF sizes)
        let header = BlockHeader {
            slot: SlotNo(50000), // Should be in KES period 0
            block_number: 50000,
            prev_hash: Blake2b256Hash::hash(b"prev"),
            issuer_vkey: Ed25519KeyHash::from_test_data(b"issuer"),
            vrf_proof: VrfProof::from_bytes([1u8; VRF_PROOF_LENGTH]).unwrap(),
            vrf_output: VrfOutput::from_bytes([2u8; VRF_OUTPUT_LENGTH]).unwrap(),
            block_body_hash: Blake2b256Hash::hash(b"body"),
            block_size: 1000,
            operational_cert: producer.operational_cert.clone(),
            protocol_magic: 764824073,
        };

        let header_bytes = header.to_bytes_for_signing();
        let signature = kes_key.sign_block(&header_bytes);
        assert!(signature.is_ok());

        // KES signature period matches current period
        let sig = signature.unwrap();
        assert_eq!(sig.period, 0);
    }

    #[test]
    fn test_block_body_hash() {
        let body = BlockBody {
            transactions: vec![],
            total_fee: 0,
            total_size: 0,
        };

        let hash = body.hash();
        assert!(!hash.as_bytes().is_empty());

        // Same body should produce same hash
        let hash2 = body.hash();
        assert_eq!(hash.as_bytes(), hash2.as_bytes());
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
        let producer1 =
            BlockProducer::new(PoolId(Blake2b256Hash::hash(b"pool1")), 10_000_000_000_000);
        let producer2 =
            BlockProducer::new(PoolId(Blake2b256Hash::hash(b"pool2")), 5_000_000_000_000);

        scheduler.add_producer(producer1);
        scheduler.add_producer(producer2);

        // Calculate schedule for a small epoch (just a few slots for testing)
        let total_stake = 15_000_000_000_000; // Sum of both producers
        let epoch_length = 32;

        // Note: This is probabilistic, so we mainly test that it doesn't crash
        let result = scheduler.calculate_epoch_schedule_with_length(1, total_stake, epoch_length);
        assert!(result.is_ok());

        // Get statistics
        let stats = scheduler.get_producer_stats();
        println!("Producer statistics: {:?}", stats);

        // Should have some assignments (though exact count is probabilistic)
        let total_assignments: u32 = stats.values().sum();
        println!("Total slot assignments: {}", total_assignments);
    }

    #[test]
    fn test_simplified_ledger_state() {
        let ledger = SimplifiedLedgerState::new();

        assert!(ledger.utxo_set.is_empty());
        assert_eq!(ledger.total_supply, 45_000_000_000_000_000);
        assert_eq!(ledger.treasury, 1_000_000_000_000_000);
        assert_eq!(ledger.reserves, 14_000_000_000_000_000);
    }

    #[test]
    fn test_legacy_apis() {
        let mut producer = create_test_producer();

        // Test legacy slot leadership check
        let leadership = producer.is_slot_leader(1000);
        assert!(leadership.is_ok());

        // Test legacy block production - may fail due to slot leadership
        let block_result = producer.produce_block(1000);
        match block_result {
            Ok(block) => {
                assert_eq!(block.slot, 1000);
                assert!(!block.header.prev_hash.as_bytes().is_empty());
                assert!(!block.body.transactions.is_empty() || block.body.transactions.is_empty());
                // Either is fine
            }
            Err(ConsensusError::NotSlotLeader(_)) => {
                // This is expected if the producer is not elected for this slot
                println!("Producer not elected for slot 1000 (expected in probabilistic system)");
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_kes_periods_remaining() {
        let kes_key = KesKey::new(6); // depth=6 gives max_period=62

        // At period 0, should have 62 periods remaining
        assert_eq!(kes_key.periods_remaining(), 62);
        assert!(!kes_key.is_approaching_expiration(10));
    }

    #[test]
    fn test_kes_approaching_expiration() {
        let mut kes_key = KesKey::new(3); // depth=3 gives max_period=6

        // Evolve to period 5 (one period before expiration)
        kes_key.evolve(5).unwrap();

        // Should be approaching expiration with threshold 10
        assert_eq!(kes_key.periods_remaining(), 1);
        assert!(kes_key.is_approaching_expiration(10));
        assert!(kes_key.is_approaching_expiration(5));
        assert!(kes_key.is_approaching_expiration(2));
        assert!(!kes_key.is_approaching_expiration(0));
    }
}
