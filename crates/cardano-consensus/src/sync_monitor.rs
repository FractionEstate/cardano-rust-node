//! Sync Progress Monitoring Service
//!
//! Tracks synchronization progress and provides metrics for monitoring sync health.

use crate::SlotNo;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;

/// Synchronization state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    /// Node is starting up
    Starting,
    /// Connecting to peers
    Connecting,
    /// Actively synchronizing blocks
    Syncing,
    /// Synchronized with network tip
    InSync,
    /// Sync has stalled
    Stalled,
    /// Sync encountered an error
    Error,
}

impl std::fmt::Display for SyncState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncState::Starting => write!(f, "Starting"),
            SyncState::Connecting => write!(f, "Connecting"),
            SyncState::Syncing => write!(f, "Syncing"),
            SyncState::InSync => write!(f, "In Sync"),
            SyncState::Stalled => write!(f, "Stalled"),
            SyncState::Error => write!(f, "Error"),
        }
    }
}

/// Comprehensive sync metrics
#[derive(Debug, Clone)]
pub struct SyncMetrics {
    /// Current slot number
    pub current_slot: u64,
    /// Current block number
    pub current_block: u64,
    /// Current epoch
    pub current_epoch: u32,
    /// Network tip slot (highest seen from peers)
    pub network_tip_slot: u64,
    /// Number of slots behind tip
    pub slots_behind: i64,
    /// Sync completion percentage (0.0 - 100.0)
    pub sync_percentage: f64,
    /// Blocks synced per second
    pub blocks_per_second: f64,
    /// Headers validated per second
    pub validation_rate: f64,
    /// Number of connected peers
    pub connected_peers: usize,
    /// Number of actively syncing peers
    pub active_peers: usize,
    /// Total headers received
    pub total_headers_received: u64,
    /// Total headers validated
    pub total_headers_validated: u64,
    /// Total validation failures
    pub validation_failures: u64,
    /// Current sync state
    pub sync_state: SyncState,
    /// Estimated time to completion
    pub eta: Option<Duration>,
    /// Timestamp of last update
    pub last_update: SystemTime,
}

impl Default for SyncMetrics {
    fn default() -> Self {
        Self {
            current_slot: 0,
            current_block: 0,
            current_epoch: 0,
            network_tip_slot: 0,
            slots_behind: 0,
            sync_percentage: 0.0,
            blocks_per_second: 0.0,
            validation_rate: 0.0,
            connected_peers: 0,
            active_peers: 0,
            total_headers_received: 0,
            total_headers_validated: 0,
            validation_failures: 0,
            sync_state: SyncState::Starting,
            eta: None,
            last_update: SystemTime::now(),
        }
    }
}

/// Sync monitoring service
///
/// Tracks sync progress, calculates metrics, and detects sync health issues.
pub struct SyncMonitor {
    /// Current metrics
    metrics: Arc<RwLock<SyncMetrics>>,
    /// Start time of sync
    start_time: Instant,
    /// Last progress update time
    last_progress: Arc<RwLock<Instant>>,
    /// Historical block counts for rate calculation
    block_history: Arc<RwLock<Vec<(Instant, u64)>>>,
    /// Configuration
    config: SyncMonitorConfig,
}

/// Configuration for sync monitor
#[derive(Debug, Clone)]
pub struct SyncMonitorConfig {
    /// How long without progress before considering sync stalled
    pub stall_timeout: Duration,
    /// Distance from tip (in slots) to consider "in sync"
    pub in_sync_threshold: u64,
    /// Sample window for rate calculation (number of samples)
    pub rate_sample_window: usize,
}

impl Default for SyncMonitorConfig {
    fn default() -> Self {
        Self {
            stall_timeout: Duration::from_secs(300), // 5 minutes
            in_sync_threshold: 100,                  // Within 100 slots
            rate_sample_window: 10,                  // Last 10 samples
        }
    }
}

impl SyncMonitor {
    /// Create new sync monitor
    pub fn new(config: SyncMonitorConfig) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(SyncMetrics::default())),
            start_time: Instant::now(),
            last_progress: Arc::new(RwLock::new(Instant::now())),
            block_history: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(SyncMonitorConfig::default())
    }

    /// Update current slot
    pub async fn update_slot(&self, slot: SlotNo) {
        let mut metrics = self.metrics.write().await;
        metrics.current_slot = slot.0;
        metrics.last_update = SystemTime::now();

        // Update progress timestamp
        let mut last_progress = self.last_progress.write().await;
        *last_progress = Instant::now();

        // Calculate sync percentage
        if metrics.network_tip_slot > 0 {
            metrics.sync_percentage =
                (metrics.current_slot as f64 / metrics.network_tip_slot as f64 * 100.0).min(100.0);
            metrics.slots_behind = metrics.network_tip_slot as i64 - metrics.current_slot as i64;
        }

        // Update sync state
        self.update_sync_state(&mut metrics).await;
    }

    /// Update network tip slot
    pub async fn update_network_tip(&self, tip_slot: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.network_tip_slot = metrics.network_tip_slot.max(tip_slot);

        // Recalculate slots behind
        metrics.slots_behind = metrics.network_tip_slot as i64 - metrics.current_slot as i64;

        // Recalculate sync percentage
        if metrics.network_tip_slot > 0 {
            metrics.sync_percentage =
                (metrics.current_slot as f64 / metrics.network_tip_slot as f64 * 100.0).min(100.0);
        }
    }

    /// Record header received from peer
    pub async fn record_header_received(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.total_headers_received += 1;
    }

    /// Record header validated
    pub async fn record_header_validated(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.total_headers_validated += 1;

        // Update block history for rate calculation
        let mut history = self.block_history.write().await;
        history.push((Instant::now(), metrics.total_headers_validated));

        // Keep only recent samples
        if history.len() > self.config.rate_sample_window {
            history.remove(0);
        }

        // Calculate validation rate
        if history.len() >= 2 {
            let first = history.first().unwrap();
            let last = history.last().unwrap();
            let elapsed = last.0.duration_since(first.0).as_secs_f64();
            let blocks = last.1 - first.1;
            metrics.validation_rate = blocks as f64 / elapsed;
        }

        // Update progress timestamp
        let mut last_progress = self.last_progress.write().await;
        *last_progress = Instant::now();
    }

    /// Record validation failure
    pub async fn record_validation_failure(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.validation_failures += 1;
    }

    /// Update peer count
    pub async fn update_peer_count(&self, connected: usize, active: usize) {
        let mut metrics = self.metrics.write().await;
        metrics.connected_peers = connected;
        metrics.active_peers = active;
    }

    /// Update current block number
    pub async fn update_block(&self, block_number: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.current_block = block_number;

        // Update block history
        let mut history = self.block_history.write().await;
        history.push((Instant::now(), block_number));

        // Keep only recent samples
        if history.len() > self.config.rate_sample_window {
            history.remove(0);
        }

        // Calculate blocks per second
        if history.len() >= 2 {
            let first = history.first().unwrap();
            let last = history.last().unwrap();
            let elapsed = last.0.duration_since(first.0).as_secs_f64();
            let blocks = last.1 - first.1;
            metrics.blocks_per_second = blocks as f64 / elapsed;
        }
    }

    /// Update current epoch
    pub async fn update_epoch(&self, epoch: u32) {
        let mut metrics = self.metrics.write().await;
        metrics.current_epoch = epoch;
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> SyncMetrics {
        let mut metrics = self.metrics.read().await.clone();

        // Calculate ETA
        metrics.eta = self.calculate_eta().await;

        metrics
    }

    /// Calculate estimated time to completion
    pub async fn calculate_eta(&self) -> Option<Duration> {
        let metrics = self.metrics.read().await;

        // Need to be syncing with positive rate
        if metrics.sync_state != SyncState::Syncing || metrics.validation_rate <= 0.0 {
            return None;
        }

        // Need to know how far behind we are
        if metrics.slots_behind <= 0 {
            return None;
        }

        // Calculate ETA based on current sync rate
        let slots_remaining = metrics.slots_behind as f64;
        let seconds_remaining = slots_remaining / metrics.validation_rate;

        if seconds_remaining > 0.0 && seconds_remaining.is_finite() {
            Some(Duration::from_secs_f64(seconds_remaining))
        } else {
            None
        }
    }

    /// Check if sync is stalled
    pub async fn is_stalled(&self) -> bool {
        let last_progress = self.last_progress.read().await;
        last_progress.elapsed() > self.config.stall_timeout
    }

    /// Get sync state
    pub async fn sync_state(&self) -> SyncState {
        self.metrics.read().await.sync_state
    }

    /// Update sync state based on current conditions
    async fn update_sync_state(&self, metrics: &mut SyncMetrics) {
        // Check if stalled
        let is_stalled = {
            let last_progress = self.last_progress.read().await;
            last_progress.elapsed() > self.config.stall_timeout
        };

        if is_stalled {
            metrics.sync_state = SyncState::Stalled;
            return;
        }

        // Check if in sync
        if metrics.slots_behind >= 0 && metrics.slots_behind <= self.config.in_sync_threshold as i64
        {
            metrics.sync_state = SyncState::InSync;
            return;
        }

        // Check if connecting
        if metrics.connected_peers == 0 {
            metrics.sync_state = SyncState::Connecting;
            return;
        }

        // Otherwise syncing
        if metrics.slots_behind > 0 {
            metrics.sync_state = SyncState::Syncing;
        }
    }

    /// Reset monitor (e.g., after reconnection)
    pub async fn reset(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = SyncMetrics::default();

        let mut last_progress = self.last_progress.write().await;
        *last_progress = Instant::now();

        let mut history = self.block_history.write().await;
        history.clear();
    }

    /// Get uptime
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Format sync progress as string
    pub async fn format_progress(&self) -> String {
        let metrics = self.get_metrics().await;

        format!(
            "Slot: {} | Epoch: {} | Progress: {:.2}% | Peers: {}/{} | Rate: {:.1} blocks/s | State: {}",
            metrics.current_slot,
            metrics.current_epoch,
            metrics.sync_percentage,
            metrics.active_peers,
            metrics.connected_peers,
            metrics.blocks_per_second,
            metrics.sync_state
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sync_monitor_creation() {
        let monitor = SyncMonitor::default();
        let metrics = monitor.get_metrics().await;

        assert_eq!(metrics.current_slot, 0);
        assert_eq!(metrics.sync_state, SyncState::Starting);
        assert_eq!(metrics.sync_percentage, 0.0);
    }

    #[tokio::test]
    async fn test_slot_update() {
        let monitor = SyncMonitor::default();

        monitor.update_network_tip(1000).await;
        monitor.update_slot(SlotNo(500)).await;

        let metrics = monitor.get_metrics().await;
        assert_eq!(metrics.current_slot, 500);
        assert_eq!(metrics.network_tip_slot, 1000);
        assert_eq!(metrics.slots_behind, 500);
        assert_eq!(metrics.sync_percentage, 50.0);
    }

    #[tokio::test]
    async fn test_sync_state_transitions() {
        let monitor = SyncMonitor::default();

        // Initially starting
        let metrics = monitor.get_metrics().await;
        assert_eq!(metrics.sync_state, SyncState::Starting);

        // No peers -> Connecting
        monitor.update_peer_count(0, 0).await;
        monitor.update_slot(SlotNo(100)).await;
        let metrics = monitor.get_metrics().await;
        assert_eq!(metrics.sync_state, SyncState::Connecting);

        // Has peers, behind -> Syncing
        monitor.update_peer_count(5, 3).await;
        monitor.update_network_tip(1000).await;
        monitor.update_slot(SlotNo(500)).await;
        let metrics = monitor.get_metrics().await;
        assert_eq!(metrics.sync_state, SyncState::Syncing);

        // Caught up -> InSync
        monitor.update_slot(SlotNo(950)).await;
        let metrics = monitor.get_metrics().await;
        assert_eq!(metrics.sync_state, SyncState::InSync);
    }

    #[tokio::test]
    async fn test_validation_rate_calculation() {
        let monitor = SyncMonitor::default();

        // Simulate validating 100 blocks over 10 seconds
        for i in 0..10 {
            monitor.record_header_validated().await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let metrics = monitor.get_metrics().await;
        // Should be around 10 blocks/sec (100 blocks in 1 second)
        assert!(metrics.validation_rate > 5.0);
    }

    #[tokio::test]
    async fn test_eta_calculation() {
        let monitor = SyncMonitor::default();

        // Setup: 1000 slots to sync, rate of 10 slots/sec
        monitor.update_network_tip(2000).await;
        monitor.update_slot(SlotNo(1000)).await;

        // Simulate consistent validation rate
        for i in 0..5 {
            monitor.record_header_validated().await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let metrics = monitor.get_metrics().await;
        if metrics.sync_state == SyncState::Syncing {
            assert!(metrics.eta.is_some());
        }
    }

    #[tokio::test]
    async fn test_stall_detection() {
        let mut config = SyncMonitorConfig::default();
        config.stall_timeout = Duration::from_millis(100);

        let monitor = SyncMonitor::new(config);

        // Initially not stalled
        assert!(!monitor.is_stalled().await);

        // Wait for stall timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be stalled now
        assert!(monitor.is_stalled().await);

        // Make progress
        monitor.update_slot(SlotNo(100)).await;

        // No longer stalled
        assert!(!monitor.is_stalled().await);
    }

    #[tokio::test]
    async fn test_metrics_history() {
        let monitor = SyncMonitor::default();

        // Record validation failures
        monitor.record_validation_failure().await;
        monitor.record_validation_failure().await;

        let metrics = monitor.get_metrics().await;
        assert_eq!(metrics.validation_failures, 2);
    }

    #[tokio::test]
    async fn test_reset() {
        let monitor = SyncMonitor::default();

        // Set some state
        monitor.update_slot(SlotNo(1000)).await;
        monitor.update_peer_count(5, 3).await;
        monitor.record_header_validated().await;

        // Reset
        monitor.reset().await;

        let metrics = monitor.get_metrics().await;
        assert_eq!(metrics.current_slot, 0);
        assert_eq!(metrics.connected_peers, 0);
        assert_eq!(metrics.total_headers_validated, 0);
    }

    #[tokio::test]
    async fn test_format_progress() {
        let monitor = SyncMonitor::default();

        monitor.update_network_tip(1000).await;
        monitor.update_slot(SlotNo(500)).await;
        monitor.update_epoch(100).await;
        monitor.update_peer_count(10, 5).await;

        let progress = monitor.format_progress().await;
        assert!(progress.contains("Slot: 500"));
        assert!(progress.contains("Epoch: 100"));
        assert!(progress.contains("50.00%"));
        assert!(progress.contains("Peers: 5/10"));
    }
}
