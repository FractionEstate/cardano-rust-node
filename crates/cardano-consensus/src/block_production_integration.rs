//! Block Production Integration Layer
//!
//! This module provides integration between the block production service and
//! the storage layer (ChainDB and LedgerDB). It replaces mock data with real
//! chain state for production block forging.
//!
//! ## Key Features
//!
//! - **Chain Tip Integration**: Provides current chain tip hash for block headers
//! - **Ledger State Integration**: Provides current UTxO set and ledger state
//! - **Automatic Updates**: Keeps block production in sync with chain state
//!
//! ## Usage
//!
//! ```rust,ignore
//! use cardano_consensus::block_production_integration::BlockProductionIntegrator;
//! use cardano_storage::{ChainDatabase, LedgerDatabase};
//!
//! let integrator = BlockProductionIntegrator::new(chaindb, ledgerdb);
//! let mut service = BlockProductionService::new(config, forger);
//!
//! // Wire up the integrator
//! integrator.wire_to_service(&mut service).await?;
//!
//! // Now service uses real chain data instead of mocks!
//! ```

use crate::{Result, SimplifiedLedgerState, TxInput, TxOutput};
use cardano_crypto::Blake2b256Hash;
use cardano_storage::{
    chaindb::ChainDatabase,
    ledgerdb::{LedgerDatabase, TransactionOutput},
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Integrator that bridges storage layer with block production
pub struct BlockProductionIntegrator<C, L>
where
    C: ChainDatabase,
    L: LedgerDatabase,
{
    chaindb: Arc<C>,
    ledgerdb: Arc<L>,
    /// Cached chain tip (updated periodically)
    cached_tip: Arc<RwLock<Option<Blake2b256Hash>>>,
    /// Cached ledger state (updated periodically)
    cached_state: Arc<RwLock<Option<SimplifiedLedgerState>>>,
}

impl<C, L> BlockProductionIntegrator<C, L>
where
    C: ChainDatabase + 'static,
    L: LedgerDatabase + 'static,
{
    /// Create a new integrator
    pub fn new(chaindb: Arc<C>, ledgerdb: Arc<L>) -> Self {
        Self {
            chaindb,
            ledgerdb,
            cached_tip: Arc::new(RwLock::new(None)),
            cached_state: Arc::new(RwLock::new(None)),
        }
    }

    /// Wire this integrator to a block production service
    ///
    /// This sets up the callbacks so the service uses real chain data
    pub async fn wire_to_service(&self, service: &mut crate::BlockProductionService) -> Result<()> {
        // Set up chain tip provider
        let chaindb = Arc::clone(&self.chaindb);
        let cached_tip = Arc::clone(&self.cached_tip);

        service.set_chain_tip_provider(move || {
            // Try to get from cache first (fast path)
            if let Some(tip) = cached_tip.try_read().ok().and_then(|c| *c) {
                debug!("Using cached chain tip for block production");
                return Some(tip);
            }

            // Cache miss or lock contention - fetch from DB
            match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    match handle
                        .block_on(async { Self::get_chain_tip_impl(Arc::clone(&chaindb)).await })
                    {
                        Ok(Some(tip)) => {
                            debug!("Fetched chain tip from ChainDB: {:?}", tip);
                            // Update cache
                            if let Ok(mut cache) = cached_tip.try_write() {
                                *cache = Some(tip);
                            }
                            Some(tip)
                        }
                        Ok(None) => {
                            warn!("No chain tip found in ChainDB");
                            None
                        }
                        Err(e) => {
                            warn!("Failed to fetch chain tip: {}", e);
                            None
                        }
                    }
                }
                Err(_) => {
                    warn!("No tokio runtime available for chain tip fetch");
                    None
                }
            }
        });

        // Set up ledger state provider
        let ledgerdb = Arc::clone(&self.ledgerdb);
        let cached_state = Arc::clone(&self.cached_state);

        service.set_ledger_state_provider(move || {
            // Try to get from cache first (fast path)
            if let Some(state) = cached_state.try_read().ok().and_then(|c| c.clone()) {
                debug!("Using cached ledger state for block production");
                return Some(state);
            }

            // Cache miss or lock contention - fetch from DB
            match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    match handle.block_on(async {
                        Self::get_ledger_state_impl(Arc::clone(&ledgerdb)).await
                    }) {
                        Ok(Some(state)) => {
                            debug!("Fetched ledger state from LedgerDB");
                            // Update cache
                            if let Ok(mut cache) = cached_state.try_write() {
                                *cache = Some(state.clone());
                            }
                            Some(state)
                        }
                        Ok(None) => {
                            warn!("No ledger state found in LedgerDB");
                            None
                        }
                        Err(e) => {
                            warn!("Failed to fetch ledger state: {}", e);
                            None
                        }
                    }
                }
                Err(_) => {
                    warn!("No tokio runtime available for ledger state fetch");
                    None
                }
            }
        });

        debug!("Block production service wired to ChainDB and LedgerDB");
        Ok(())
    }

    /// Refresh the cached chain tip
    ///
    /// Should be called periodically (e.g., on new blocks)
    pub async fn refresh_chain_tip(&self) -> Result<()> {
        match Self::get_chain_tip_impl(Arc::clone(&self.chaindb)).await? {
            Some(tip) => {
                *self.cached_tip.write().await = Some(tip);
                debug!("Refreshed chain tip cache: {:?}", tip);
                Ok(())
            }
            None => {
                warn!("No chain tip available to refresh");
                Ok(())
            }
        }
    }

    /// Refresh the cached ledger state
    ///
    /// Should be called after applying new blocks
    pub async fn refresh_ledger_state(&self) -> Result<()> {
        match Self::get_ledger_state_impl(Arc::clone(&self.ledgerdb)).await? {
            Some(state) => {
                *self.cached_state.write().await = Some(state);
                debug!("Refreshed ledger state cache");
                Ok(())
            }
            None => {
                warn!("No ledger state available to refresh");
                Ok(())
            }
        }
    }

    /// Internal implementation to fetch chain tip from database
    async fn get_chain_tip_impl(chaindb: Arc<C>) -> Result<Option<Blake2b256Hash>> {
        match chaindb.get_chain_metadata().await {
            Ok(Some(metadata)) => {
                debug!(
                    "Chain tip from metadata: height={}, hash={:?}",
                    metadata.tip_height, metadata.tip_hash
                );
                Ok(Some(metadata.tip_hash))
            }
            Ok(None) => {
                debug!("No chain metadata found - likely genesis or uninitialized");
                Ok(None)
            }
            Err(e) => {
                warn!("Failed to get chain metadata: {}", e);
                Ok(None)
            }
        }
    }

    /// Internal implementation to fetch ledger state from database
    async fn get_ledger_state_impl(ledgerdb: Arc<L>) -> Result<Option<SimplifiedLedgerState>> {
        // Get ledger stats from LedgerDB
        let stats = match ledgerdb.get_ledger_stats().await {
            Ok(stats) => stats,
            Err(e) => {
                warn!("Failed to get ledger stats: {}", e);
                return Ok(None);
            }
        };

        debug!(
            "Current ledger stats: {} UTxOs, {} total value, {} active pools",
            stats.total_utxos, stats.total_value, stats.active_pools
        );

        let utxo_entries = match ledgerdb.export_utxos(None).await {
            Ok(entries) => entries,
            Err(e) => {
                warn!("Failed to export UTxOs: {}", e);
                Vec::new()
            }
        };

        let mut utxo_set = HashMap::with_capacity(utxo_entries.len());
        for (input, output) in utxo_entries {
            let tx_input = TxInput {
                tx_hash: input.transaction_id,
                output_index: input.index,
            };

            let address = derive_output_address(&output);
            utxo_set.insert(
                tx_input,
                TxOutput {
                    address,
                    value: output.amount,
                },
            );
        }

        if utxo_set.is_empty() {
            debug!("Ledger snapshot returned no UTxOs");
        } else {
            debug!("Ledger snapshot loaded {} UTxOs", utxo_set.len());
        }

        let total_supply = stats
            .total_value
            .saturating_add(stats.treasury)
            .saturating_add(stats.reserves);

        Ok(Some(SimplifiedLedgerState {
            utxo_set,
            total_supply,
            treasury: stats.treasury,
            reserves: stats.reserves,
        }))
    }
}

/// Helper to create an integrator with automatic cache refresh
pub struct AutoRefreshIntegrator<C, L>
where
    C: ChainDatabase,
    L: LedgerDatabase,
{
    integrator: Arc<BlockProductionIntegrator<C, L>>,
    refresh_interval: std::time::Duration,
}

impl<C, L> AutoRefreshIntegrator<C, L>
where
    C: ChainDatabase + 'static,
    L: LedgerDatabase + 'static,
{
    /// Create a new auto-refresh integrator
    ///
    /// # Arguments
    ///
    /// * `chaindb` - Chain database instance
    /// * `ledgerdb` - Ledger database instance
    /// * `refresh_interval` - How often to refresh caches (e.g., every 20 seconds)
    pub fn new(chaindb: Arc<C>, ledgerdb: Arc<L>, refresh_interval: std::time::Duration) -> Self {
        let integrator = BlockProductionIntegrator::new(chaindb, ledgerdb);
        Self {
            integrator: Arc::new(integrator),
            refresh_interval,
        }
    }

    /// Get the underlying integrator
    pub fn integrator(&self) -> Arc<BlockProductionIntegrator<C, L>> {
        Arc::clone(&self.integrator)
    }

    /// Start the automatic cache refresh task
    ///
    /// Returns a join handle for the background task
    pub fn start_auto_refresh(&self) -> tokio::task::JoinHandle<()> {
        let integrator = Arc::clone(&self.integrator);
        let interval = self.refresh_interval;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;

                // Refresh chain tip
                if let Err(e) = integrator.refresh_chain_tip().await {
                    warn!("Failed to refresh chain tip: {}", e);
                }

                // Refresh ledger state
                if let Err(e) = integrator.refresh_ledger_state().await {
                    warn!("Failed to refresh ledger state: {}", e);
                }

                debug!("Cache refresh complete");
            }
        })
    }
}

// Tests will be added in integration tests once mock implementations are updated
// to match the current trait signatures

fn derive_output_address(output: &TransactionOutput) -> Blake2b256Hash {
    if output.address_data.len() == 32 {
        Blake2b256Hash::from_bytes(&output.address_data)
            .unwrap_or_else(|_| Blake2b256Hash::hash(&output.address_data))
    } else {
        Blake2b256Hash::hash(&output.address_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cardano_storage::backends::MemoryBackend;
    use cardano_storage::chaindb::ChainDatabaseImpl;
    use cardano_storage::ledgerdb::{
        LedgerDatabaseImpl, StakeCredential, TransactionInput as LedgerTxInput,
        TransactionOutput as LedgerTxOutput,
    };
    use std::sync::Arc;

    #[tokio::test]
    async fn test_get_ledger_state_exports_utxos() {
        let chain_backend = Arc::new(MemoryBackend::new());
        let chaindb = Arc::new(ChainDatabaseImpl::new(chain_backend));

        let ledger_backend = Arc::new(MemoryBackend::new());
        let ledgerdb = Arc::new(LedgerDatabaseImpl::new(Arc::clone(&ledger_backend)));

        let tx_input = LedgerTxInput {
            transaction_id: Blake2b256Hash::hash(b"tx_hash_example"),
            index: 0,
        };
        let tx_output = LedgerTxOutput {
            amount: 1_500_000,
            address_data: Blake2b256Hash::hash(b"addr_example").as_bytes().to_vec(),
        };

        ledgerdb.store_utxo(&tx_input, &tx_output).await.unwrap();

        let stake_credential = StakeCredential {
            credential_data: vec![0u8, 1, 2, 3],
        };
        ledgerdb
            .store_rewards(&stake_credential, 250_000)
            .await
            .unwrap();

        ledgerdb
            .store_stake(&stake_credential, 3_000_000)
            .await
            .unwrap();

        let state = BlockProductionIntegrator::<ChainDatabaseImpl<MemoryBackend>, _>::get_ledger_state_impl(ledgerdb.clone())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(state.utxo_set.len(), 1);

        let expected_input = TxInput {
            tx_hash: tx_input.transaction_id,
            output_index: tx_input.index,
        };
        let consensus_output = state.utxo_set.get(&expected_input).unwrap();
        assert_eq!(consensus_output.value, tx_output.amount);

        assert_eq!(state.treasury, 250_000);
        assert!(state.total_supply >= state.treasury + consensus_output.value);
        assert!(state.reserves > 0);

        // Ensure integrator wiring does not panic with concrete types
        let _integrator = BlockProductionIntegrator::new(chaindb, ledgerdb);
    }
}
