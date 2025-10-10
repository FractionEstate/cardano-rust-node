//! KES (Key Evolving Signature) Evolution Tracker
//!
//! This module provides automatic KES key evolution management for block producers.
//! KES keys must be evolved at specific period boundaries and will eventually expire,
//! requiring operational certificate renewal.
//!
//! ## KES Key Lifecycle
//!
//! ```text
//! Period 0 ──> Period 1 ──> Period 2 ──> ... ──> Period 126 ──> Period 127 [EXPIRED]
//!    │            │            │                      │            │
//!    │            │            │                      │            └─> WARNING: Generate new key!
//!    │            │            │                      └──────> WARNING: Approaching expiration
//!    │            │            └──────────────────> Automatic evolution
//!    │            └────────────────────────> Automatic evolution
//!    └──────────────────────────────────> Initial key
//! ```
//!
//! With CompactSum7 KES there are 2^7 = 128 periods (0-127).
//! Each period is approximately 36 hours (129,600 slots).

use anyhow::{anyhow, Result};
use cardano_consensus::KesKey;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Configuration for KES evolution tracking
#[derive(Debug, Clone)]
pub struct KesEvolutionConfig {
    /// Slots per KES period (mainnet: 129,600 slots = ~36 hours)
    pub slots_per_period: u64,

    /// Number of periods to warn before expiration
    pub warning_threshold: u64,

    /// Whether to enable automatic evolution
    pub auto_evolve: bool,

    /// Maximum KES period (2^depth)
    pub max_kes_period: u64,
}

impl Default for KesEvolutionConfig {
    fn default() -> Self {
        Self {
            slots_per_period: 129_600, // Mainnet default
            warning_threshold: 10,     // Warn 10 periods before expiration (~15 days)
            auto_evolve: true,
            max_kes_period: 127, // CompactSum7 KES supports periods 0-127
        }
    }
}

impl KesEvolutionConfig {
    /// Create configuration for mainnet
    pub fn mainnet() -> Self {
        Self {
            slots_per_period: 129_600,
            warning_threshold: 10,
            auto_evolve: true,
            max_kes_period: 127,
        }
    }

    /// Create configuration for testnet
    pub fn testnet() -> Self {
        Self {
            slots_per_period: 129_600,
            warning_threshold: 5,
            auto_evolve: true,
            max_kes_period: 127,
        }
    }

    /// Create configuration for preview testnet
    pub fn preview() -> Self {
        Self {
            slots_per_period: 129_600,
            warning_threshold: 5,
            auto_evolve: true,
            max_kes_period: 127,
        }
    }
}

/// KES key evolution tracker
///
/// Monitors slot progression and automatically evolves KES keys at period boundaries.
/// Emits warnings when keys approach expiration.
pub struct KesEvolutionTracker {
    /// KES key being tracked
    kes_key: Arc<RwLock<KesKey>>,

    /// Configuration
    config: KesEvolutionConfig,

    /// Current slot (for period calculation)
    current_slot: Arc<RwLock<u64>>,

    /// Last period when evolution was performed
    last_evolved_period: Arc<RwLock<u64>>,

    /// Whether warning has been emitted for current expiration window
    warning_emitted: Arc<RwLock<bool>>,
}

impl KesEvolutionTracker {
    /// Create a new KES evolution tracker
    pub fn new(kes_key: Arc<RwLock<KesKey>>, config: KesEvolutionConfig) -> Self {
        Self {
            kes_key,
            config,
            current_slot: Arc::new(RwLock::new(0)),
            last_evolved_period: Arc::new(RwLock::new(0)),
            warning_emitted: Arc::new(RwLock::new(false)),
        }
    }

    /// Update current slot and check if evolution is needed
    ///
    /// This should be called on every slot notification.
    /// Returns true if evolution occurred.
    pub async fn update_slot(&self, slot: u64) -> Result<bool> {
        // Update current slot
        {
            let mut current = self.current_slot.write().await;
            *current = slot;
        }

        // Calculate current KES period from slot
        let current_period = self.calculate_period(slot);

        // Check key status
        let kes_key = self.kes_key.read().await;
        let key_period = kes_key.current_period();
        let max_period = kes_key.max_period;
        drop(kes_key); // Release read lock

        // Check for expiration
        if current_period >= max_period {
            error!(
                "❌ KES KEY EXPIRED! Current period {} >= max period {}. \
                 Block production will FAIL until operational certificate is renewed.",
                current_period, max_period
            );
            return Ok(false);
        }

        // Check if approaching expiration
        let periods_remaining = max_period.saturating_sub(current_period);
        if periods_remaining <= self.config.warning_threshold {
            let mut warned = self.warning_emitted.write().await;
            if !*warned {
                warn!(
                    "⚠️  KES KEY APPROACHING EXPIRATION! {} periods remaining ({} days). \
                     Generate new key and operational certificate soon!",
                    periods_remaining,
                    (periods_remaining * self.config.slots_per_period) / (24 * 60 * 60) // Convert to days
                );
                *warned = true;
            }
        }

        // Check if evolution is needed
        if current_period > key_period {
            info!(
                "KES key needs evolution: current_period={}, key_period={}",
                current_period, key_period
            );

            if self.config.auto_evolve {
                return self.evolve_to_period(current_period).await;
            } else {
                warn!(
                    "KES key needs evolution but auto_evolve is disabled. \
                     Manual evolution required."
                );
            }
        }

        Ok(false)
    }

    /// Manually evolve KES key to a specific period
    pub async fn evolve_to_period(&self, target_period: u64) -> Result<bool> {
        let mut kes_key = self.kes_key.write().await;

        // Check if already at target period
        if kes_key.current_period() >= target_period {
            debug!(
                "KES key already at period {} (target: {})",
                kes_key.current_period(),
                target_period
            );
            return Ok(false);
        }

        // Perform evolution
        let old_period = kes_key.current_period();
        match kes_key.evolve(target_period) {
            Ok(()) => {
                let new_period = kes_key.current_period();
                info!(
                    "✅ KES key evolved: period {} -> {} ({} periods remaining)",
                    old_period,
                    new_period,
                    kes_key.periods_remaining()
                );

                // Update last evolved period
                let mut last = self.last_evolved_period.write().await;
                *last = new_period;

                // Reset warning flag on successful evolution
                let mut warned = self.warning_emitted.write().await;
                *warned = false;

                Ok(true)
            }
            Err(e) => {
                error!("Failed to evolve KES key: {}", e);
                Err(anyhow!("KES evolution failed: {}", e))
            }
        }
    }

    /// Calculate KES period from absolute slot number
    pub fn calculate_period(&self, slot: u64) -> u64 {
        slot / self.config.slots_per_period
    }

    /// Get current KES period based on last known slot
    pub async fn current_period(&self) -> u64 {
        let slot = *self.current_slot.read().await;
        self.calculate_period(slot)
    }

    /// Get KES key's current period
    pub async fn key_period(&self) -> u64 {
        self.kes_key.read().await.current_period()
    }

    /// Get number of periods remaining until expiration
    pub async fn periods_remaining(&self) -> u64 {
        self.kes_key.read().await.periods_remaining()
    }

    /// Check if KES key is expired
    pub async fn is_expired(&self) -> bool {
        let kes_key = self.kes_key.read().await;
        kes_key.is_expired()
    }

    /// Check if KES key is approaching expiration
    pub async fn is_approaching_expiration(&self) -> bool {
        let kes_key = self.kes_key.read().await;
        kes_key.is_approaching_expiration(self.config.warning_threshold)
    }

    /// Get expiration status message
    pub async fn status_message(&self) -> String {
        let kes_key = self.kes_key.read().await;
        let current_slot = *self.current_slot.read().await;
        let current_period = self.calculate_period(current_slot);
        let key_period = kes_key.current_period();
        let periods_remaining = kes_key.periods_remaining();
        let max_period = kes_key.max_period;

        if kes_key.is_expired() {
            format!("❌ EXPIRED (period {}/{})", key_period, max_period)
        } else if kes_key.is_approaching_expiration(self.config.warning_threshold) {
            format!(
                "⚠️  APPROACHING EXPIRATION ({} periods remaining)",
                periods_remaining
            )
        } else if key_period < current_period {
            format!(
                "⚠️  NEEDS EVOLUTION (key at period {}, current period {})",
                key_period, current_period
            )
        } else {
            format!(
                "✅ OK (period {}/{}, {} periods remaining)",
                key_period, max_period, periods_remaining
            )
        }
    }

    /// Run the evolution tracker in background
    ///
    /// This spawns a background task that monitors slots and evolves the key automatically.
    /// The task runs until dropped.
    pub async fn run(
        self: Arc<Self>,
        mut slot_rx: tokio::sync::broadcast::Receiver<u64>,
    ) -> Result<()> {
        info!("Starting KES evolution tracker");

        loop {
            match slot_rx.recv().await {
                Ok(slot) => {
                    if let Err(e) = self.update_slot(slot).await {
                        error!("Error updating KES tracker for slot {}: {}", slot, e);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!("KES tracker lagged by {} slots", skipped);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    info!("Slot receiver closed, stopping KES tracker");
                    break;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cardano_consensus::KesKey;
    use cardano_crypto::KesSecretKey;

    #[tokio::test]
    async fn test_period_calculation() {
        let kes_key = Arc::new(RwLock::new(KesKey::new()));
        let config = KesEvolutionConfig::mainnet();
        let tracker = KesEvolutionTracker::new(kes_key, config.clone());

        // Period 0: slots 0 - 129,599
        assert_eq!(tracker.calculate_period(0), 0);
        assert_eq!(tracker.calculate_period(129_599), 0);

        // Period 1: slots 129,600 - 259,199
        assert_eq!(tracker.calculate_period(129_600), 1);
        assert_eq!(tracker.calculate_period(259_199), 1);

        // Period 2: slots 259,200 - 388,799
        assert_eq!(tracker.calculate_period(259_200), 2);
    }

    #[tokio::test]
    async fn test_evolution_detection() {
        let kes_key = Arc::new(RwLock::new(KesKey::new()));
        let config = KesEvolutionConfig {
            slots_per_period: 100,
            warning_threshold: 5,
            auto_evolve: true,
            max_kes_period: 127,
        };
        let tracker = KesEvolutionTracker::new(kes_key.clone(), config);

        // Initial period
        assert_eq!(tracker.key_period().await, 0);

        // Move to period 1 (slot 100)
        let evolved = tracker.update_slot(100).await.unwrap();
        assert!(evolved, "Should have evolved to period 1");
        assert_eq!(tracker.key_period().await, 1);

        // Move to period 2 (slot 200)
        let evolved = tracker.update_slot(200).await.unwrap();
        assert!(evolved, "Should have evolved to period 2");
        assert_eq!(tracker.key_period().await, 2);
    }

    #[tokio::test]
    async fn test_expiration_warning() {
        let kes_key = Arc::new(RwLock::new(KesKey::new()));
        let config = KesEvolutionConfig {
            slots_per_period: 100,
            warning_threshold: 5,
            auto_evolve: true,
            max_kes_period: 10, // Small max for testing
        };
        let tracker = KesEvolutionTracker::new(kes_key.clone(), config);

        // Move the KES key near expiration (remaining periods = 4)
        {
            let mut key = kes_key.write().await;
            let target_period = KesSecretKey::MAX_PERIOD.saturating_sub(4);
            key.evolve(target_period).expect("evolve near expiration");
        }

        // Should be approaching expiration (4 < 5)
        assert!(tracker.is_approaching_expiration().await);
    }

    #[tokio::test]
    async fn test_status_message() {
        let kes_key = Arc::new(RwLock::new(KesKey::new()));
        let config = KesEvolutionConfig::mainnet();
        let tracker = KesEvolutionTracker::new(kes_key, config);

        let status = tracker.status_message().await;
        assert!(status.contains("OK"));
    }
}
