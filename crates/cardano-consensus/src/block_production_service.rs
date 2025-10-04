//! Block Production Runtime Service
//!
//! Coordinates block production by subscribing to slot notifications and
//! orchestrating the complete block forging flow.

#[cfg_attr(not(test), allow(unused_imports))]
use crate::{
    BlockForger, EpochNo, ForgedBlock, ForgingConfig, ForgingContext, PoolId, Result,
    SimplifiedLedgerState, SlotEvent, SlotNotifier, StakeDistribution, Transaction,
};
use cardano_crypto::Blake2b256Hash;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Events emitted by the block production service
#[derive(Debug, Clone)]
pub enum BlockProductionEvent {
    /// A slot started
    SlotStarted {
        slot: u64,
        timestamp: std::time::SystemTime,
    },
    /// Leadership check performed
    LeadershipChecked { slot: u64, is_leader: bool },
    /// Block forging started
    ForgingStarted { slot: u64, tx_count: usize },
    /// Block successfully forged
    BlockForged {
        slot: u64,
        block_number: u64,
        tx_count: usize,
        block_size: usize,
    },
    /// Block forging failed
    ForgingFailed { slot: u64, error: String },
    /// KES key evolved
    KesEvolved { from_period: u64, to_period: u64 },
}

/// Configuration for block production service
#[derive(Debug, Clone)]
pub struct BlockProductionConfig {
    /// Pool ID
    pub pool_id: PoolId,
    /// Pool stake amount
    pub pool_stake: u64,
    /// Total network stake
    pub total_stake: u64,
    /// Active slot coefficient (typically 0.05 for 5%)
    pub active_slot_coeff: f64,
    /// Current epoch
    pub epoch: EpochNo,
    /// Epoch nonce for VRF
    pub epoch_nonce: Blake2b256Hash,
    /// Block forging configuration
    pub forging_config: ForgingConfig,
}

/// Block production runtime service
pub struct BlockProductionService {
    config: Arc<RwLock<BlockProductionConfig>>,
    forger: Arc<RwLock<BlockForger>>,
    stats: Arc<RwLock<BlockProductionStats>>,
    event_tx: broadcast::Sender<BlockProductionEvent>,
}

/// Statistics for block production
#[derive(Debug, Clone, Default)]
pub struct BlockProductionStats {
    pub slots_checked: u64,
    pub leadership_won: u64,
    pub blocks_forged: u64,
    pub forging_failures: u64,
    pub kes_evolutions: u64,
    pub total_transactions: u64,
}

impl BlockProductionService {
    /// Create a new block production service
    pub fn new(config: BlockProductionConfig, forger: BlockForger) -> Self {
        let (event_tx, _) = broadcast::channel(100);

        Self {
            config: Arc::new(RwLock::new(config)),
            forger: Arc::new(RwLock::new(forger)),
            stats: Arc::new(RwLock::new(BlockProductionStats::default())),
            event_tx,
        }
    }

    /// Subscribe to block production events
    pub fn subscribe(&self) -> broadcast::Receiver<BlockProductionEvent> {
        self.event_tx.subscribe()
    }

    /// Get current statistics
    pub async fn stats(&self) -> BlockProductionStats {
        self.stats.read().await.clone()
    }

    /// Update configuration (e.g., for epoch transitions)
    pub async fn update_config(&self, config: BlockProductionConfig) {
        let mut cfg = self.config.write().await;
        *cfg = config;
        info!("Block production configuration updated");
    }

    /// Run the block production service
    ///
    /// Subscribes to slot notifications and attempts block production
    /// for each slot. Should be spawned as a background task.
    pub async fn run(
        self: Arc<Self>,
        slot_notifier: Arc<SlotNotifier>,
        mempool_rx: mpsc::Receiver<Vec<Transaction>>,
        block_tx: mpsc::Sender<ForgedBlock>,
    ) -> Result<()> {
        info!("Starting block production service");

        let mut slot_receiver = slot_notifier.subscribe();
        let mut mempool_rx = mempool_rx;
        let mut current_mempool: Vec<Transaction> = Vec::new();

        loop {
            tokio::select! {
                // Handle slot events
                slot_event = slot_receiver.recv() => {
                    match slot_event {
                        Ok(event) => {
                            if let Err(e) = self.handle_slot_event(
                                event,
                                &current_mempool,
                                &block_tx
                            ).await {
                                error!("Error handling slot event: {}", e);
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!("Block production service lagged by {} slots", skipped);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            error!("Slot notification channel closed");
                            break;
                        }
                    }
                }

                // Handle mempool updates
                mempool_update = mempool_rx.recv() => {
                    if let Some(txs) = mempool_update {
                        debug!("Mempool updated with {} transactions", txs.len());
                        current_mempool = txs;
                    } else {
                        debug!("Mempool channel closed");
                        break;
                    }
                }
            }
        }

        info!("Block production service stopped");
        Ok(())
    }

    /// Handle a slot event
    async fn handle_slot_event(
        &self,
        event: SlotEvent,
        mempool: &[Transaction],
        block_tx: &mpsc::Sender<ForgedBlock>,
    ) -> Result<()> {
        let slot = event.slot;

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.slots_checked += 1;
        }

        // Emit slot started event
        self.emit_event(BlockProductionEvent::SlotStarted {
            slot: slot.0,
            timestamp: event.timestamp,
        });

        debug!("Checking slot {} for leadership", slot.0);

        // Note: KES evolution is handled automatically inside try_forge_block

        // Build forging context
        let context = self.build_forging_context(slot, mempool).await;

        // Try to forge block
        let mut forger = self.forger.write().await;

        match forger.try_forge_block(&context) {
            Ok(Some(forged_block)) => {
                // We're the leader and forged a block!
                let tx_count = forged_block.body.transactions.len();
                let block_size = forged_block.header.block_size;
                let block_number = forged_block.header.block_number;

                info!(
                    "✓ Forged block for slot {} (block #{}, {} txs, {} bytes)",
                    slot.0, block_number, tx_count, block_size
                );

                // Update stats
                {
                    let mut stats = self.stats.write().await;
                    stats.leadership_won += 1;
                    stats.blocks_forged += 1;
                    stats.total_transactions += tx_count as u64;
                }

                // Emit events
                self.emit_event(BlockProductionEvent::LeadershipChecked {
                    slot: slot.0,
                    is_leader: true,
                });

                self.emit_event(BlockProductionEvent::BlockForged {
                    slot: slot.0,
                    block_number,
                    tx_count,
                    block_size: block_size as usize,
                });

                // Send block for broadcasting
                if let Err(e) = block_tx.send(forged_block).await {
                    error!("Failed to send forged block for broadcasting: {}", e);
                }
            }
            Ok(None) => {
                // Not elected as leader for this slot
                debug!("Not elected for slot {}", slot.0);

                self.emit_event(BlockProductionEvent::LeadershipChecked {
                    slot: slot.0,
                    is_leader: false,
                });
            }
            Err(e) => {
                // Error during forging
                error!("Failed to forge block for slot {}: {}", slot.0, e);

                {
                    let mut stats = self.stats.write().await;
                    stats.forging_failures += 1;
                }

                self.emit_event(BlockProductionEvent::ForgingFailed {
                    slot: slot.0,
                    error: e.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Build forging context for current slot
    async fn build_forging_context(
        &self,
        slot: crate::SlotNo,
        mempool: &[Transaction],
    ) -> ForgingContext {
        let config = self.config.read().await;

        ForgingContext {
            current_slot: slot,
            epoch_nonce: config.epoch_nonce.clone(),
            prev_block_hash: Blake2b256Hash::hash(b"prev_block"), // TODO: Get from chain tip
            mempool: mempool.to_vec(),
            ledger_state: SimplifiedLedgerState::new(), // TODO: Get actual ledger state
        }
    }

    /// Emit an event to subscribers
    fn emit_event(&self, event: BlockProductionEvent) {
        if self.event_tx.send(event).is_err() {
            // No subscribers, which is fine
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockProductionOperationalCertificate, KesKey, LeadershipCalculator, VrfKey};
    use cardano_crypto::Ed25519KeyHash;

    fn create_test_config() -> BlockProductionConfig {
        BlockProductionConfig {
            pool_id: PoolId(Blake2b256Hash::hash(b"test_pool")),
            pool_stake: 1_000_000_000_000,
            total_stake: 10_000_000_000_000,
            active_slot_coeff: 0.05,
            epoch: EpochNo(1),
            epoch_nonce: Blake2b256Hash::hash(b"test_nonce"),
            forging_config: ForgingConfig::default(),
        }
    }

    fn create_test_forger() -> BlockForger {
        let pool_id = PoolId(Blake2b256Hash::hash(b"test_pool"));
        let vrf_key = VrfKey::for_pool(&pool_id);
        let kes_key = KesKey::new(6);

        let operational_cert = BlockProductionOperationalCertificate {
            hot_vkey: Ed25519KeyHash::from_test_data(b"hot_key"),
            sequence_number: 0,
            kes_period: 0,
            sigma: Blake2b256Hash::hash(b"cold_sig"),
        };

        let stake_dist = StakeDistribution {
            total_stake: 10_000_000_000_000,
            pools: vec![(pool_id.clone(), 1_000_000_000_000)]
                .into_iter()
                .collect(),
        };

        let leadership_calc = LeadershipCalculator::new(
            stake_dist,
            crate::ProtocolParameters::testnet(),
            Blake2b256Hash::hash(b"test_nonce"),
            EpochNo(1),
        );

        BlockForger::new(
            pool_id,
            vrf_key,
            kes_key,
            operational_cert,
            1_000_000_000_000,
            leadership_calc,
        )
    }

    #[test]
    fn test_service_creation() {
        let config = create_test_config();
        let forger = create_test_forger();

        let service = BlockProductionService::new(config, forger);
        let _rx = service.subscribe();
    }

    #[tokio::test]
    async fn test_stats() {
        let config = create_test_config();
        let forger = create_test_forger();

        let service = BlockProductionService::new(config, forger);
        let stats = service.stats().await;

        assert_eq!(stats.slots_checked, 0);
        assert_eq!(stats.blocks_forged, 0);
    }

    #[tokio::test]
    async fn test_config_update() {
        let config = create_test_config();
        let forger = create_test_forger();

        let service = BlockProductionService::new(config.clone(), forger);

        let mut new_config = config;
        new_config.epoch = EpochNo(2);

        service.update_config(new_config).await;

        let updated_config = service.config.read().await;
        assert_eq!(updated_config.epoch.0, 2);
    }
}
