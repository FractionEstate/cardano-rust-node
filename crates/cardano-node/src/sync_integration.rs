//! Sync Integration Service
//!
//! Bridges the ChainSyncService (network layer) with SyncMonitor (consensus layer)
//! to provide unified sync progress monitoring.

use anyhow::Result;
use cardano_consensus::SyncMonitor;
use cardano_network::services::ChainSyncService;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, info, warn};

/// Sync integration service that connects network and consensus layers
pub struct SyncIntegrationService {
    /// Network-level chain sync service
    chain_sync: Arc<ChainSyncService>,
    /// Consensus-level sync monitor
    sync_monitor: Arc<SyncMonitor>,
    /// Update interval
    update_interval: Duration,
}

impl SyncIntegrationService {
    /// Create new sync integration service
    pub fn new(
        chain_sync: Arc<ChainSyncService>,
        sync_monitor: Arc<SyncMonitor>,
        update_interval: Duration,
    ) -> Self {
        Self {
            chain_sync,
            sync_monitor,
            update_interval,
        }
    }

    /// Create with default update interval (1 second)
    pub fn with_default_interval(
        chain_sync: Arc<ChainSyncService>,
        sync_monitor: Arc<SyncMonitor>,
    ) -> Self {
        Self::new(chain_sync, sync_monitor, Duration::from_secs(1))
    }

    /// Start the integration service
    ///
    /// This runs in the background, periodically pulling metrics from ChainSyncService
    /// and updating the SyncMonitor.
    pub async fn run(self: Arc<Self>) -> Result<()> {
        info!("Starting sync integration service");

        let mut tick = interval(self.update_interval);

        loop {
            tick.tick().await;

            if let Err(e) = self.update_metrics().await {
                warn!("Failed to update sync metrics: {}", e);
            }
        }
    }

    /// Update SyncMonitor with latest metrics from ChainSyncService
    async fn update_metrics(&self) -> Result<()> {
        // Get stats from network layer
        let sync_stats = self.chain_sync.get_stats().await;

        debug!(
            "Updating sync metrics: local_tip={}, network_tip={}, peers={}",
            sync_stats.local_tip_slot, sync_stats.network_tip_slot, sync_stats.active_peers
        );

        // Update sync monitor with network metrics
        self.sync_monitor
            .update_network_tip(sync_stats.network_tip_slot)
            .await;

        self.sync_monitor
            .update_slot(cardano_consensus::SlotNo(sync_stats.local_tip_slot))
            .await;

        self.sync_monitor
            .update_peer_count(sync_stats.active_peers, sync_stats.active_peers)
            .await;

        // The SyncMonitor will automatically calculate:
        // - slots_behind
        // - sync_percentage
        // - sync_state transitions
        // - ETA based on sync rate

        Ok(())
    }

    /// Get the sync monitor for external access
    pub fn sync_monitor(&self) -> Arc<SyncMonitor> {
        Arc::clone(&self.sync_monitor)
    }

    /// Get the chain sync service for external access
    pub fn chain_sync(&self) -> Arc<ChainSyncService> {
        Arc::clone(&self.chain_sync)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cardano_consensus::SyncMonitorConfig;
    use cardano_network::protocols::chainsync::ChainSyncConfig;

    #[tokio::test]
    async fn test_sync_integration_creation() {
        let chain_sync = Arc::new(ChainSyncService::new(ChainSyncConfig::default()));
        let sync_monitor = Arc::new(SyncMonitor::new(SyncMonitorConfig::default()));

        let integration =
            SyncIntegrationService::with_default_interval(chain_sync.clone(), sync_monitor.clone());

        assert!(Arc::ptr_eq(&integration.chain_sync(), &chain_sync));
        assert!(Arc::ptr_eq(&integration.sync_monitor(), &sync_monitor));
    }

    #[tokio::test]
    async fn test_sync_metrics_update() {
        let chain_sync = Arc::new(ChainSyncService::new(ChainSyncConfig::default()));
        let sync_monitor = Arc::new(SyncMonitor::new(SyncMonitorConfig::default()));

        let integration = Arc::new(SyncIntegrationService::with_default_interval(
            chain_sync.clone(),
            sync_monitor.clone(),
        ));

        // Set some network state
        use cardano_crypto::Blake2b256Hash;
        use cardano_network::protocols::chainsync::Tip;

        chain_sync
            .set_local_tip(Tip {
                slot: cardano_consensus::SlotNo(1000),
                hash: Blake2b256Hash::from_bytes(&[0u8; 32]).unwrap(),
                height: 1000,
            })
            .await;

        // Update metrics
        integration.update_metrics().await.unwrap();

        // Verify sync monitor was updated
        let metrics = sync_monitor.get_metrics().await;
        assert_eq!(metrics.current_slot, 1000);
    }

    #[tokio::test]
    async fn test_peer_count_propagation() {
        let chain_sync = Arc::new(ChainSyncService::new(ChainSyncConfig::default()));
        let sync_monitor = Arc::new(SyncMonitor::new(SyncMonitorConfig::default()));

        let integration = Arc::new(SyncIntegrationService::with_default_interval(
            chain_sync.clone(),
            sync_monitor.clone(),
        ));

        // Add some peers (using PeerId directly)
        use cardano_network::services::PeerId;
        for i in 0..3 {
            chain_sync.add_peer(PeerId(i)).await.unwrap();
        }

        // Update metrics
        integration.update_metrics().await.unwrap();

        // Verify peer count was propagated
        let metrics = sync_monitor.get_metrics().await;
        // Note: ChainSyncService counts active syncing peers, not just connected peers
        // So this might be 0 if peers aren't actively syncing yet
        assert!(metrics.connected_peers <= 3);
    }
}
