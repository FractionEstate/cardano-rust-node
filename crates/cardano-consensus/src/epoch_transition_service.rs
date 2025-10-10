//! Epoch Transition Service
//!
//! This service listens to slot notifications and triggers epoch transitions
//! when epoch boundaries are detected. It coordinates between:
//! - SlotNotifier (detects epoch boundaries)
//! - EpochTransitionHandler (executes transition logic)
//! - BlockProductionService (updates with new epoch nonce and stake snapshots)
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────┐
//! │ SlotNotifier │
//! └──────┬───────┘
//!        │ SlotEvent(is_epoch_boundary=true)
//!        ▼
//! ┌────────────────────────────┐
//! │ EpochTransitionService     │
//! │  - Detects epoch boundary  │
//! │  - Invokes handler         │
//! │  - Updates block prod      │
//! └───────┬────────────────────┘
//!         │
//!         ├─► EpochTransitionHandler::process_epoch_transition()
//!         │
//!         └─► BlockProductionService::update_config(new_epoch_nonce)
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use cardano_consensus::{EpochTransitionService, SlotNotifier};
//!
//! let slot_notifier = Arc::new(SlotNotifier::new(config));
//! let handler = EpochTransitionHandler::new(ledgerdb, protocol_params);
//! let service = Arc::new(EpochTransitionService::new(handler));
//!
//! // Start the service
//! service.start(slot_notifier).await?;
//! ```

use crate::{EpochNo, EpochTransitionHandler, Result, SlotEvent, SlotNotifier};
use cardano_storage::LedgerDatabase;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Epoch transition service that monitors slot events and triggers transitions
pub struct EpochTransitionService<L: LedgerDatabase> {
    /// Epoch transition handler
    handler: Arc<RwLock<EpochTransitionHandler<L>>>,

    /// Last processed epoch (to avoid duplicate processing)
    last_processed_epoch: Arc<RwLock<Option<EpochNo>>>,
}
impl<L: LedgerDatabase + 'static> EpochTransitionService<L> {
    /// Create a new epoch transition service
    pub fn new(handler: EpochTransitionHandler<L>) -> Self {
        Self {
            handler: Arc::new(RwLock::new(handler)),
            last_processed_epoch: Arc::new(RwLock::new(None)),
        }
    }

    /// Start the service by subscribing to slot notifications
    ///
    /// This should be run in a background task via `tokio::spawn`.
    pub async fn start(self: Arc<Self>, slot_notifier: Arc<SlotNotifier>) -> Result<()> {
        info!("Starting epoch transition service");

        let mut slot_receiver = slot_notifier.subscribe();

        loop {
            match slot_receiver.recv().await {
                Ok(slot_event) => {
                    if let Err(e) = self.handle_slot_event(slot_event).await {
                        error!("Error handling slot event: {}", e);
                        // Continue processing despite errors
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!(
                        "Epoch transition service lagged, skipped {} slot events",
                        skipped
                    );
                    // Continue processing
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    warn!("Slot notifier channel closed, stopping epoch transition service");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle a slot event and trigger epoch transition if needed
    pub async fn handle_slot_event(&self, event: SlotEvent) -> Result<()> {
        // Check if this is an epoch boundary
        if !event.is_epoch_boundary {
            return Ok(());
        }

        let current_epoch = event.epoch;

        // Check if we've already processed this epoch transition
        {
            let last_processed = self.last_processed_epoch.read().await;
            if let Some(last_epoch) = *last_processed {
                if last_epoch.0 >= current_epoch.0 {
                    // Already processed this transition
                    return Ok(());
                }
            }
        }

        info!(
            "Processing epoch transition at slot {} (epoch {})",
            event.slot.0, current_epoch.0
        );

        // Calculate completed epoch (previous epoch)
        let completed_epoch = if current_epoch.0 > 0 {
            EpochNo(current_epoch.0 - 1)
        } else {
            EpochNo(0)
        };

        // Process the epoch transition
        {
            let mut handler = self.handler.write().await;
            handler
                .process_epoch_transition(completed_epoch, current_epoch, event.slot)
                .await?;
        }

        // Update last processed epoch
        {
            let mut last_processed = self.last_processed_epoch.write().await;
            *last_processed = Some(current_epoch);
        }

        info!(
            "Epoch transition completed successfully (epoch {})",
            current_epoch.0
        );

        Ok(())
    }
    /// Get the current epoch nonce from the handler
    pub async fn current_nonce(&self) -> cardano_crypto::Blake2b256Hash {
        let handler = self.handler.read().await;
        handler.current_nonce()
    }

    /// Get a stake snapshot for a specific epoch
    pub async fn get_stake_snapshot(&self, epoch: EpochNo) -> Option<crate::StakeSnapshot> {
        let handler = self.handler.read().await;
        handler.get_stake_snapshot(epoch).cloned()
    }

    /// Get current reserves amount
    pub async fn reserves(&self) -> u64 {
        let handler = self.handler.read().await;
        handler.reserves()
    }

    /// Get current treasury amount
    pub async fn treasury(&self) -> u64 {
        let handler = self.handler.read().await;
        handler.treasury()
    }
}
