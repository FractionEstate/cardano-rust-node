//! Slot Notification System
//!
//! Provides real-time slot notifications for block production and other
//! time-sensitive consensus operations.

use crate::{ConsensusError, EpochNo, Result, SlotNo};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Slot event emitted at the start of each slot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotEvent {
    /// The slot number
    pub slot: SlotNo,
    /// Actual time the event was emitted
    pub timestamp: SystemTime,
    /// Expected slot start time
    pub expected_time: SystemTime,
    /// Drift from expected time (positive = late, negative = early)
    pub drift_ms: i64,
    /// Current epoch number
    pub epoch: EpochNo,
    /// True if this slot marks an epoch boundary
    pub is_epoch_boundary: bool,
}

/// Configuration for slot notifications
#[derive(Debug, Clone)]
pub struct SlotNotifierConfig {
    /// Length of each slot in seconds
    pub slot_length_secs: u64,
    /// Genesis time (start of slot 0)
    pub genesis_time: SystemTime,
    /// Maximum acceptable drift before warning (milliseconds)
    pub max_drift_ms: i64,
    /// Size of the broadcast channel
    pub channel_size: usize,
    /// Number of slots per epoch
    pub epoch_length: u64,
}

impl Default for SlotNotifierConfig {
    fn default() -> Self {
        Self {
            slot_length_secs: 1,      // 1 second slots for Cardano
            genesis_time: UNIX_EPOCH, // Will be overridden with actual genesis
            max_drift_ms: 100,        // Warn if drift exceeds 100ms
            channel_size: 100,
            epoch_length: 432000, // Cardano mainnet epoch length (5 days)
        }
    }
}

/// Slot notifier service
///
/// Emits slot events at the start of each slot. Multiple subscribers
/// can listen for slot events via the broadcast channel.
pub struct SlotNotifier {
    config: SlotNotifierConfig,
    sender: broadcast::Sender<SlotEvent>,
}

impl SlotNotifier {
    /// Create a new slot notifier
    pub fn new(config: SlotNotifierConfig) -> Self {
        let (sender, _) = broadcast::channel(config.channel_size);
        Self { config, sender }
    }

    /// Subscribe to slot events
    ///
    /// Returns a receiver that will receive slot events. Each subscriber
    /// gets their own independent receiver.
    pub fn subscribe(&self) -> broadcast::Receiver<SlotEvent> {
        self.sender.subscribe()
    }

    /// Get the current slot number
    pub fn current_slot(&self) -> Result<SlotNo> {
        let now = SystemTime::now();
        let elapsed = now
            .duration_since(self.config.genesis_time)
            .map_err(|e| ConsensusError::SlotError(format!("Time calculation error: {}", e)))?;

        let slot = elapsed.as_secs() / self.config.slot_length_secs;
        Ok(SlotNo(slot))
    }

    /// Calculate the epoch number for a given slot
    pub fn slot_to_epoch(&self, slot: SlotNo) -> EpochNo {
        EpochNo(slot.0 / self.config.epoch_length)
    }

    /// Check if a slot is at an epoch boundary
    pub fn is_epoch_boundary(&self, slot: SlotNo) -> bool {
        slot.0 % self.config.epoch_length == 0 && slot.0 > 0
    }

    /// Get the first slot of an epoch
    pub fn epoch_first_slot(&self, epoch: EpochNo) -> SlotNo {
        SlotNo(epoch.0 * self.config.epoch_length)
    }

    /// Get the last slot of an epoch
    pub fn epoch_last_slot(&self, epoch: EpochNo) -> SlotNo {
        SlotNo((epoch.0 + 1) * self.config.epoch_length - 1)
    }

    /// Get the expected start time for a slot
    pub fn slot_start_time(&self, slot: SlotNo) -> SystemTime {
        self.config.genesis_time + Duration::from_secs(slot.0 * self.config.slot_length_secs)
    }

    /// Get the expected end time for a slot
    pub fn slot_end_time(&self, slot: SlotNo) -> SystemTime {
        self.slot_start_time(SlotNo(slot.0 + 1))
    }

    /// Calculate drift between actual and expected time
    fn calculate_drift(&self, actual: SystemTime, expected: SystemTime) -> i64 {
        match actual.duration_since(expected) {
            Ok(duration) => duration.as_millis() as i64,
            Err(e) => -(e.duration().as_millis() as i64),
        }
    }

    /// Run the slot notifier
    ///
    /// This will continuously emit slot events. It should be run in a
    /// background task using `tokio::spawn`.
    pub async fn run(self: Arc<Self>) -> Result<()> {
        info!(
            "Starting slot notifier (slot_length={}s, genesis={:?})",
            self.config.slot_length_secs, self.config.genesis_time
        );

        // Get initial slot
        let mut current_slot = self.current_slot()?;
        info!("Current slot: {}", current_slot.0);

        // Wait until the start of the next slot
        let next_slot = SlotNo(current_slot.0 + 1);
        let next_slot_time = self.slot_start_time(next_slot);

        if let Ok(wait_duration) = next_slot_time.duration_since(SystemTime::now()) {
            debug!("Waiting {:?} until slot {}", wait_duration, next_slot.0);
            sleep(wait_duration).await;
        }

        current_slot = next_slot;

        loop {
            let actual_time = SystemTime::now();
            let expected_time = self.slot_start_time(current_slot);
            let drift_ms = self.calculate_drift(actual_time, expected_time);

            // Calculate epoch information
            let epoch = self.slot_to_epoch(current_slot);
            let is_epoch_boundary = self.is_epoch_boundary(current_slot);

            // Log epoch transitions
            if is_epoch_boundary {
                info!(
                    "Epoch boundary detected! Transitioning to epoch {} at slot {}",
                    epoch.0, current_slot.0
                );
            }

            // Log drift warnings
            if drift_ms.abs() > self.config.max_drift_ms {
                warn!(
                    "Slot {} notification drift: {}ms (max: {}ms)",
                    current_slot.0, drift_ms, self.config.max_drift_ms
                );
            }

            // Create and send slot event
            let event = SlotEvent {
                slot: current_slot,
                timestamp: actual_time,
                expected_time,
                drift_ms,
                epoch,
                is_epoch_boundary,
            };

            debug!(
                "Slot {} start (epoch: {}, drift: {}ms, boundary: {})",
                current_slot.0, epoch.0, drift_ms, is_epoch_boundary
            );

            // Send to all subscribers
            match self.sender.send(event) {
                Ok(receiver_count) => {
                    debug!("Slot event sent to {} subscribers", receiver_count);
                }
                Err(_) => {
                    // No active receivers, but that's okay
                    debug!("No active slot event subscribers");
                }
            }

            // Calculate next slot
            current_slot = SlotNo(current_slot.0 + 1);
            let next_slot_time = self.slot_start_time(current_slot);

            // Calculate precise sleep duration to stay aligned
            let now = SystemTime::now();
            match next_slot_time.duration_since(now) {
                Ok(sleep_duration) => {
                    // Add a small buffer to ensure we don't wake up too early
                    let buffered_duration =
                        sleep_duration.saturating_sub(Duration::from_millis(10));
                    sleep(buffered_duration).await;

                    // Busy wait for the precise moment
                    while SystemTime::now() < next_slot_time {
                        tokio::task::yield_now().await;
                    }
                }
                Err(_) => {
                    // We're already past the next slot time
                    error!("Slot notifier is lagging! Missed slot {}", current_slot.0);
                    // Catch up to current time
                    current_slot = self.current_slot()?;
                }
            }
        }
    }

    /// Get notifier statistics
    pub fn stats(&self) -> SlotNotifierStats {
        SlotNotifierStats {
            active_subscribers: self.sender.receiver_count(),
            current_slot: self.current_slot().ok(),
        }
    }
}

/// Statistics about the slot notifier
#[derive(Debug, Clone)]
pub struct SlotNotifierStats {
    /// Number of active subscribers
    pub active_subscribers: usize,
    /// Current slot number
    pub current_slot: Option<SlotNo>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn test_slot_notifier_creation() {
        let config = SlotNotifierConfig::default();
        let notifier = SlotNotifier::new(config);

        let stats = notifier.stats();
        assert_eq!(stats.active_subscribers, 0);
    }

    #[test]
    fn test_slot_calculation() {
        let genesis = UNIX_EPOCH + Duration::from_secs(1000);
        let config = SlotNotifierConfig {
            slot_length_secs: 1,
            genesis_time: genesis,
            ..Default::default()
        };

        let notifier = SlotNotifier::new(config);

        // Slot 0 should start at genesis
        let slot_0_time = notifier.slot_start_time(SlotNo(0));
        assert_eq!(slot_0_time, genesis);

        // Slot 1 should start 1 second after genesis
        let slot_1_time = notifier.slot_start_time(SlotNo(1));
        assert_eq!(slot_1_time, genesis + Duration::from_secs(1));

        // Slot end should be next slot start
        let slot_0_end = notifier.slot_end_time(SlotNo(0));
        assert_eq!(slot_0_end, slot_1_time);
    }

    #[test]
    fn test_drift_calculation() {
        let genesis = UNIX_EPOCH;
        let config = SlotNotifierConfig {
            genesis_time: genesis,
            ..Default::default()
        };
        let notifier = SlotNotifier::new(config);

        let expected = genesis + Duration::from_secs(100);
        let actual_late = expected + Duration::from_millis(50);
        let actual_early = expected - Duration::from_millis(30);

        let drift_late = notifier.calculate_drift(actual_late, expected);
        assert_eq!(drift_late, 50);

        let drift_early = notifier.calculate_drift(actual_early, expected);
        assert_eq!(drift_early, -30);
    }

    #[test]
    fn test_subscribe() {
        let config = SlotNotifierConfig::default();
        let notifier = SlotNotifier::new(config);

        let _rx1 = notifier.subscribe();
        let _rx2 = notifier.subscribe();

        let stats = notifier.stats();
        assert_eq!(stats.active_subscribers, 2);
    }

    #[test]
    fn test_epoch_calculation() {
        let config = SlotNotifierConfig {
            epoch_length: 100, // 100 slots per epoch for testing
            ..Default::default()
        };
        let notifier = SlotNotifier::new(config);

        // Slot 0 is in epoch 0
        assert_eq!(notifier.slot_to_epoch(SlotNo(0)).0, 0);

        // Slot 99 is still in epoch 0
        assert_eq!(notifier.slot_to_epoch(SlotNo(99)).0, 0);

        // Slot 100 is in epoch 1
        assert_eq!(notifier.slot_to_epoch(SlotNo(100)).0, 1);

        // Slot 250 is in epoch 2
        assert_eq!(notifier.slot_to_epoch(SlotNo(250)).0, 2);
    }

    #[test]
    fn test_epoch_boundary_detection() {
        let config = SlotNotifierConfig {
            epoch_length: 100, // 100 slots per epoch for testing
            ..Default::default()
        };
        let notifier = SlotNotifier::new(config);

        // Slot 0 is NOT a boundary (genesis)
        assert!(!notifier.is_epoch_boundary(SlotNo(0)));

        // Slot 99 is NOT a boundary
        assert!(!notifier.is_epoch_boundary(SlotNo(99)));

        // Slot 100 IS a boundary (first slot of epoch 1)
        assert!(notifier.is_epoch_boundary(SlotNo(100)));

        // Slot 200 IS a boundary (first slot of epoch 2)
        assert!(notifier.is_epoch_boundary(SlotNo(200)));

        // Slot 201 is NOT a boundary
        assert!(!notifier.is_epoch_boundary(SlotNo(201)));
    }

    #[test]
    fn test_epoch_slot_ranges() {
        let config = SlotNotifierConfig {
            epoch_length: 100,
            ..Default::default()
        };
        let notifier = SlotNotifier::new(config);

        // Epoch 0: slots 0-99
        assert_eq!(notifier.epoch_first_slot(EpochNo(0)).0, 0);
        assert_eq!(notifier.epoch_last_slot(EpochNo(0)).0, 99);

        // Epoch 1: slots 100-199
        assert_eq!(notifier.epoch_first_slot(EpochNo(1)).0, 100);
        assert_eq!(notifier.epoch_last_slot(EpochNo(1)).0, 199);

        // Epoch 5: slots 500-599
        assert_eq!(notifier.epoch_first_slot(EpochNo(5)).0, 500);
        assert_eq!(notifier.epoch_last_slot(EpochNo(5)).0, 599);
    }

    #[tokio::test]
    async fn test_slot_events() {
        // Use a past genesis so we can test immediately
        let genesis = SystemTime::now() - Duration::from_secs(5);
        let config = SlotNotifierConfig {
            slot_length_secs: 1,
            genesis_time: genesis,
            channel_size: 10,
            ..Default::default()
        };

        let notifier = Arc::new(SlotNotifier::new(config));
        let mut receiver = notifier.subscribe();

        // Spawn the notifier task
        let notifier_clone = notifier.clone();
        let handle = tokio::spawn(async move {
            let _ = notifier_clone.run().await;
        });

        // Receive a few slot events
        let event1 = receiver.recv().await.unwrap();
        let event2 = receiver.recv().await.unwrap();

        // Events should be sequential
        assert_eq!(event2.slot.0, event1.slot.0 + 1);

        // Clean up
        handle.abort();
    }
}
