//! Block Forger - Orchestrates block production
//!
//! The BlockForger combines:
//! - Leadership calculation (VRF-based slot election)
//! - Block body construction (transaction selection)
//! - Block header construction (with VRF proof)
//! - KES signing (block authentication)
//! - Block validation before broadcasting

use crate::block_production::{
    BlockBody, BlockHeader, ForgedBlock, ForgingContext, KesKey, OperationalCertificate,
    Transaction, VrfKey,
};
use crate::leadership::{LeadershipCalculator, LeadershipCheck, LeadershipProof};
use crate::ouroboros::PoolId;
use crate::{ConsensusError, Result};
use cardano_crypto::{Ed25519KeyHash, KesSignature};

/// Configuration for block forging
#[derive(Debug, Clone)]
pub struct ForgingConfig {
    /// Maximum block size in bytes (mainnet: ~88KB)
    pub max_block_size: u32,
    /// Maximum number of transactions per block
    pub max_transactions: usize,
    /// Protocol magic number
    pub protocol_magic: u32,
    /// KES evolution period length (slots per KES period)
    pub kes_period_length: u64,
}

impl Default for ForgingConfig {
    fn default() -> Self {
        Self {
            max_block_size: 90112, // ~88KB
            max_transactions: 1000,
            protocol_magic: 764824073, // Mainnet
            kes_period_length: 129600, // ~36 hours
        }
    }
}

/// Block Forger orchestrates the entire block production process
pub struct BlockForger {
    /// Pool ID
    pool_id: PoolId,
    /// VRF key for leadership
    vrf_key: VrfKey,
    /// KES key for signing
    kes_key: KesKey,
    /// Operational certificate
    operational_cert: OperationalCertificate,
    /// Pool stake
    pool_stake: u64,
    /// Leadership calculator
    leadership_calculator: LeadershipCalculator,
    /// Forging configuration
    config: ForgingConfig,
}

impl BlockForger {
    /// Create a new block forger
    pub fn new(
        pool_id: PoolId,
        vrf_key: VrfKey,
        kes_key: KesKey,
        operational_cert: OperationalCertificate,
        pool_stake: u64,
        leadership_calculator: LeadershipCalculator,
    ) -> Self {
        Self {
            pool_id,
            vrf_key,
            kes_key,
            operational_cert,
            pool_stake,
            leadership_calculator,
            config: ForgingConfig::default(),
        }
    }

    /// Set custom forging configuration
    pub fn with_config(mut self, config: ForgingConfig) -> Self {
        self.config = config;
        self
    }

    /// Attempt to forge a block for the given slot
    ///
    /// Returns `Ok(Some(block))` if elected leader and block forged successfully
    /// Returns `Ok(None)` if not elected leader for this slot
    /// Returns `Err` if an error occurs during block production
    pub fn try_forge_block(&mut self, context: &ForgingContext) -> Result<Option<ForgedBlock>> {
        // Check if we're elected leader for this slot
        let leadership_check = self.leadership_calculator.check_slot_leadership(
            &self.pool_id,
            self.pool_stake,
            &self.vrf_key.private_key,
            context.current_slot,
        )?;

        match leadership_check {
            LeadershipCheck::Leader(proof) => {
                // We are leader - forge the block
                let block = self.forge_block_as_leader(context, proof)?;
                Ok(Some(block))
            }
            LeadershipCheck::NotLeader { .. } => {
                // Not leader for this slot
                Ok(None)
            }
        }
    }

    /// Forge a block when we know we're the leader
    fn forge_block_as_leader(
        &mut self,
        context: &ForgingContext,
        leadership_proof: LeadershipProof,
    ) -> Result<ForgedBlock> {
        // Calculate KES period and evolve key if needed
        let kes_period = context.current_slot.0 / self.config.kes_period_length;

        if let Some((from, to)) = self.evolve_kes_if_needed(kes_period)? {
            tracing::info!("✓ KES key evolved from period {} to {}", from, to);

            // Check if approaching expiration and warn
            let remaining = self.kes_key.periods_remaining();
            if remaining <= 10 {
                tracing::warn!(
                    "⚠️  KES key approaching expiration! Only {} periods remaining. Generate new keys soon.",
                    remaining
                );
            }
        }

        // Select transactions for the block
        let body = self.construct_block_body(&context.mempool)?;

        // Build block header
        let header = self.construct_block_header(context, &leadership_proof, &body)?;

        // Sign block header with KES
        let header_bytes = header.to_bytes_for_signing();
        let kes_signature = self.kes_key.sign_block(&header_bytes)?;

        // Validate block before returning
        self.validate_forged_block(&header, &body, &kes_signature)?;

        Ok(ForgedBlock {
            header,
            body,
            proof_of_leadership: leadership_proof,
            kes_signature,
        })
    }

    /// Construct block body by selecting transactions from mempool
    fn construct_block_body(&self, mempool: &[Transaction]) -> Result<BlockBody> {
        if mempool.is_empty() {
            // Empty block is valid
            return Ok(BlockBody::empty());
        }

        let mut selected = Vec::new();
        let mut total_size = 0u32;

        // Sort transactions by fee density (fee per byte)
        let mut sorted_txs: Vec<_> = mempool.iter().collect();
        sorted_txs.sort_by(|a, b| {
            let density_a = a.fee as f64 / a.size.max(1) as f64;
            let density_b = b.fee as f64 / b.size.max(1) as f64;
            density_b
                .partial_cmp(&density_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Greedily select transactions
        for tx in sorted_txs {
            // Check transaction count limit
            if selected.len() >= self.config.max_transactions {
                break;
            }

            // Check size limit
            if total_size + tx.size > self.config.max_block_size {
                continue; // Try next transaction
            }

            // Basic validation (in production, this would be more thorough)
            if self.validate_transaction(tx).is_ok() {
                total_size += tx.size;
                selected.push(tx.clone());
            }
        }

        Ok(BlockBody::from_transactions(selected))
    }

    /// Construct block header
    fn construct_block_header(
        &self,
        context: &ForgingContext,
        leadership_proof: &LeadershipProof,
        body: &BlockBody,
    ) -> Result<BlockHeader> {
        // Calculate block number from slot (simplified - should come from chain state)
        let block_number = context.current_slot.0;

        // Convert PoolId to Ed25519KeyHash for issuer_vkey
        let issuer_vkey = Ed25519KeyHash::from_test_data(self.pool_id.0.as_bytes());

        Ok(BlockHeader {
            slot: context.current_slot,
            block_number,
            prev_hash: context.prev_block_hash,
            issuer_vkey,
            vrf_proof: leadership_proof.vrf_proof.clone(),
            vrf_output: leadership_proof.vrf_output.clone(),
            block_body_hash: body.hash(),
            block_size: body.total_size,
            operational_cert: self.operational_cert.clone(),
            protocol_magic: self.config.protocol_magic,
        })
    }

    /// Evolve KES key if current period has advanced
    ///
    /// Returns (evolved, from_period, to_period) tuple
    /// - evolved: true if evolution occurred
    /// - from_period: the period before evolution (if evolved)
    /// - to_period: the period after evolution (if evolved)
    fn evolve_kes_if_needed(&mut self, current_kes_period: u64) -> Result<Option<(u64, u64)>> {
        if self.kes_key.needs_evolution(current_kes_period) {
            let from_period = self.kes_key.current_period();

            // Check if key is expired before attempting evolution
            if self.kes_key.is_expired() {
                return Err(ConsensusError::KesKeyExpired(format!(
                    "KES key expired at period {}",
                    from_period
                )));
            }

            // Evolve the KES key
            self.kes_key.evolve(current_kes_period)?;

            // Update operational certificate
            self.operational_cert.kes_period = current_kes_period;
            self.operational_cert.sequence_number += 1;

            return Ok(Some((from_period, current_kes_period)));
        }

        Ok(None)
    }

    /// Basic transaction validation
    fn validate_transaction(&self, tx: &Transaction) -> Result<()> {
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

        Ok(())
    }

    /// Validate the forged block before broadcasting
    fn validate_forged_block(
        &self,
        header: &BlockHeader,
        body: &BlockBody,
        kes_signature: &KesSignature,
    ) -> Result<()> {
        // Check block size
        if body.exceeds_size_limit(self.config.max_block_size) {
            return Err(ConsensusError::InvalidBlock(
                "Block exceeds size limit".to_string(),
            ));
        }

        // Check transaction count
        if body.transactions.len() > self.config.max_transactions {
            return Err(ConsensusError::InvalidBlock(
                "Too many transactions in block".to_string(),
            ));
        }

        // Verify body hash matches
        let computed_hash = body.hash();
        if computed_hash != header.block_body_hash {
            return Err(ConsensusError::InvalidBlock(
                "Block body hash mismatch".to_string(),
            ));
        }

        // Verify KES signature period matches
        if kes_signature.period != self.kes_key.current_period() {
            return Err(ConsensusError::InvalidKesSignature(
                "KES signature period mismatch".to_string(),
            ));
        }

        // In production: verify VRF proof, verify KES signature, check operational cert

        Ok(())
    }

    /// Get current KES period
    pub fn current_kes_period(&self) -> u64 {
        self.kes_key.current_period()
    }

    /// Check if KES key is expired
    pub fn is_kes_expired(&self) -> bool {
        self.kes_key.is_expired()
    }

    /// Get number of KES periods remaining until expiration
    pub fn kes_periods_remaining(&self) -> u64 {
        self.kes_key.periods_remaining()
    }

    /// Check if KES key is approaching expiration
    pub fn is_kes_approaching_expiration(&self, threshold: u64) -> bool {
        self.kes_key.is_approaching_expiration(threshold)
    }

    /// Get maximum KES period
    pub fn kes_max_period(&self) -> u64 {
        self.kes_key.max_period
    }

    /// Get pool ID
    pub fn pool_id(&self) -> &PoolId {
        &self.pool_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ouroboros::{EpochNo, PoolId, ProtocolParameters, StakeDistribution};
    use cardano_crypto::{Blake2b256Hash, Ed25519KeyHash};
    use std::collections::HashMap;

    #[test]
    fn test_forging_config_defaults() {
        let config = ForgingConfig::default();
        assert_eq!(config.max_block_size, 90112);
        assert_eq!(config.max_transactions, 1000);
        assert_eq!(config.protocol_magic, 764824073);
    }

    #[test]
    fn test_block_forger_creation() {
        let pool_id = PoolId(Blake2b256Hash::hash(b"test_pool"));
        let vrf_key = VrfKey::new();
        let kes_key = KesKey::new();
        let operational_cert = OperationalCertificate {
            hot_vkey: Ed25519KeyHash::from_test_data(b"hot_key"),
            sequence_number: 1,
            kes_period: 0,
            sigma: Blake2b256Hash::hash(b"cold_sig"),
        };

        let stake_dist = StakeDistribution {
            pools: HashMap::new(),
            total_stake: 1_000_000,
        };
        let protocol_params = ProtocolParameters::mainnet();
        let epoch_nonce = Blake2b256Hash::hash(b"epoch_nonce");
        let current_epoch = EpochNo(100);

        let leadership_calc =
            LeadershipCalculator::new(stake_dist, protocol_params, epoch_nonce, current_epoch);

        let forger = BlockForger::new(
            pool_id,
            vrf_key,
            kes_key,
            operational_cert,
            1_000_000,
            leadership_calc,
        );

        assert_eq!(forger.current_kes_period(), 0);
        assert!(!forger.is_kes_expired());
    }

    #[test]
    fn test_empty_block_body_construction() {
        let body = BlockBody::empty();
        assert_eq!(body.transactions.len(), 0);
        assert_eq!(body.total_fee, 0);
        assert_eq!(body.total_size, 0);
    }

    #[test]
    fn test_transaction_validation_no_inputs() {
        let pool_id = PoolId(Blake2b256Hash::hash(b"test_pool"));
        let vrf_key = VrfKey::new();
        let kes_key = KesKey::new();
        let operational_cert = OperationalCertificate {
            hot_vkey: Ed25519KeyHash::from_test_data(b"hot_key"),
            sequence_number: 1,
            kes_period: 0,
            sigma: Blake2b256Hash::hash(b"cold_sig"),
        };

        let stake_dist = StakeDistribution {
            pools: HashMap::new(),
            total_stake: 1_000_000,
        };
        let protocol_params = ProtocolParameters::mainnet();
        let epoch_nonce = Blake2b256Hash::hash(b"epoch_nonce");
        let current_epoch = EpochNo(100);

        let leadership_calc =
            LeadershipCalculator::new(stake_dist, protocol_params, epoch_nonce, current_epoch);

        let forger = BlockForger::new(
            pool_id,
            vrf_key,
            kes_key,
            operational_cert,
            1_000_000,
            leadership_calc,
        );

        let tx = Transaction {
            tx_id: Blake2b256Hash::hash(b"tx1"),
            inputs: vec![],
            outputs: vec![],
            fee: 1000,
            size: 200,
        };

        let result = forger.validate_transaction(&tx);
        assert!(result.is_err());
    }
}
