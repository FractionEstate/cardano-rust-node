//! Monitoring and Metrics Collection for Block Production Pipeline
//!
//! This module provides comprehensive metrics collection and aggregation for
//! the entire block production pipeline. It collects metrics from all components
//! (SlotNotifier, BlockProductionService, BlockForger, BlockBroadcaster, LedgerState)
//! and exports them in Prometheus format for monitoring dashboards.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐
//! │ SlotNotifier    │─┐
//! └─────────────────┘ │
//! ┌─────────────────┐ │
//! │ BlockProduction │─┤
//! └─────────────────┘ │    ┌─────────────────┐    ┌──────────────┐
//! ┌─────────────────┐ ├───▶│ MetricsAggregator│───▶│ Prometheus   │
//! │ BlockBroadcaster│─┤    └─────────────────┘    │ Exporter     │
//! └─────────────────┘ │                            └──────────────┘
//! ┌─────────────────┐ │
//! │ LedgerState     │─┘
//! └─────────────────┘
//! ```

use crate::{
    block_broadcaster::BroadcastStats, block_production_service::BlockProductionStats,
    slot_notifier::SlotNotifierStats,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

/// Comprehensive metrics for the entire block production pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineMetrics {
    /// Timestamp when metrics were collected
    pub timestamp: u64,
    /// Slot notification metrics
    pub slot_metrics: SlotMetrics,
    /// Block production metrics
    pub production_metrics: ProductionMetrics,
    /// Block broadcasting metrics
    pub broadcast_metrics: BroadcastMetrics,
    /// Ledger state metrics
    pub ledger_metrics: LedgerMetrics,
    /// Aggregated performance metrics
    pub performance_metrics: PerformanceMetrics,
}

/// Slot notification metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotMetrics {
    /// Current slot number
    pub current_slot: u64,
    /// Number of active slot event subscribers
    pub active_subscribers: usize,
    /// Average slot drift in milliseconds
    pub avg_drift_ms: f64,
    /// Maximum observed drift in milliseconds
    pub max_drift_ms: i64,
    /// Slots with drift exceeding threshold
    pub high_drift_count: u64,
}

/// Block production metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionMetrics {
    /// Total slots where leadership was checked
    pub slots_checked: u64,
    /// Number of times leadership was won
    pub leadership_won: u64,
    /// Total blocks successfully forged
    pub blocks_forged: u64,
    /// Block forging failures
    pub forging_failures: u64,
    /// KES key evolution events
    pub kes_evolutions: u64,
    /// Total transactions included in blocks
    pub total_transactions: u64,
    /// Success rate (blocks_forged / leadership_won)
    pub success_rate: f64,
}

/// Block broadcasting metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastMetrics {
    /// Total blocks broadcast to network
    pub blocks_broadcast: u64,
    /// Blocks dropped due to queue overflow
    pub blocks_dropped: u64,
    /// Failed broadcast attempts
    pub broadcast_failures: u64,
    /// Broadcast retry attempts
    pub broadcast_retries: u64,
    /// Total peer broadcast operations
    pub total_peer_broadcasts: u64,
    /// Current broadcast queue size
    pub current_queue_size: usize,
    /// Average broadcast latency in milliseconds
    pub avg_broadcast_latency_ms: u64,
    /// Broadcast success rate
    pub success_rate: f64,
}

/// Ledger state metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerMetrics {
    /// Current UTxO set size
    pub utxo_count: u64,
    /// Total value in UTxO set (lovelace)
    pub total_value: u64,
    /// Total transactions processed
    pub transactions_processed: u64,
    /// Average transaction validation time (ms)
    pub avg_validation_time_ms: f64,
    /// Current epoch
    pub current_epoch: u64,
    /// Current slot
    pub current_slot: u64,
}

/// Aggregated performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Blocks per hour production rate
    pub blocks_per_hour: f64,
    /// Average transactions per block
    pub avg_transactions_per_block: f64,
    /// Overall pipeline health score (0-100)
    pub health_score: f64,
    /// Time since last block produced (seconds)
    pub time_since_last_block_secs: u64,
}

/// Metrics aggregator that collects and combines metrics from all components
pub struct MetricsAggregator {
    /// Historical metrics for trend analysis
    history: Arc<RwLock<Vec<PipelineMetrics>>>,
    /// Maximum number of historical entries to keep
    max_history: usize,
    /// Timestamp of last block production
    last_block_time: Arc<RwLock<Option<SystemTime>>>,
}

impl MetricsAggregator {
    /// Create a new metrics aggregator
    pub fn new(max_history: usize) -> Self {
        Self {
            history: Arc::new(RwLock::new(Vec::with_capacity(max_history))),
            max_history,
            last_block_time: Arc::new(RwLock::new(None)),
        }
    }

    /// Record a block production event (updates timing metrics)
    pub async fn record_block_production(&self) {
        *self.last_block_time.write().await = Some(SystemTime::now());
    }

    /// Collect metrics from all pipeline components
    pub async fn collect_metrics(
        &self,
        slot_stats: SlotNotifierStats,
        production_stats: BlockProductionStats,
        broadcast_stats: BroadcastStats,
        ledger_utxo_count: u64,
        ledger_total_value: u64,
        ledger_transactions_processed: u64,
        ledger_avg_validation_time_ms: f64,
        ledger_current_epoch: u64,
        ledger_current_slot: u64,
    ) -> PipelineMetrics {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Calculate time since last block
        let time_since_last_block_secs = self
            .last_block_time
            .read()
            .await
            .and_then(|last_time| SystemTime::now().duration_since(last_time).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Slot metrics
        let slot_metrics = SlotMetrics {
            current_slot: slot_stats.current_slot.map(|s| s.0).unwrap_or(0),
            active_subscribers: slot_stats.active_subscribers,
            avg_drift_ms: 0.0, // TODO: Track drift history
            max_drift_ms: 0,
            high_drift_count: 0,
        };

        // Production metrics with success rate
        let success_rate = if production_stats.leadership_won > 0 {
            (production_stats.blocks_forged as f64) / (production_stats.leadership_won as f64)
                * 100.0
        } else {
            100.0
        };

        let production_metrics = ProductionMetrics {
            slots_checked: production_stats.slots_checked,
            leadership_won: production_stats.leadership_won,
            blocks_forged: production_stats.blocks_forged,
            forging_failures: production_stats.forging_failures,
            kes_evolutions: production_stats.kes_evolutions,
            total_transactions: production_stats.total_transactions,
            success_rate,
        };

        // Broadcast metrics with success rate
        let broadcast_success_rate = if broadcast_stats.blocks_broadcast > 0 {
            let failed = broadcast_stats.broadcast_failures;
            let total = broadcast_stats.blocks_broadcast;
            ((total - failed) as f64 / total as f64) * 100.0
        } else {
            100.0
        };

        let broadcast_metrics = BroadcastMetrics {
            blocks_broadcast: broadcast_stats.blocks_broadcast,
            blocks_dropped: broadcast_stats.blocks_dropped,
            broadcast_failures: broadcast_stats.broadcast_failures,
            broadcast_retries: broadcast_stats.broadcast_retries,
            total_peer_broadcasts: broadcast_stats.total_peer_broadcasts,
            current_queue_size: broadcast_stats.current_queue_size,
            avg_broadcast_latency_ms: broadcast_stats.avg_broadcast_latency_ms,
            success_rate: broadcast_success_rate,
        };

        // Ledger metrics
        let ledger_metrics = LedgerMetrics {
            utxo_count: ledger_utxo_count,
            total_value: ledger_total_value,
            transactions_processed: ledger_transactions_processed,
            avg_validation_time_ms: ledger_avg_validation_time_ms,
            current_epoch: ledger_current_epoch,
            current_slot: ledger_current_slot,
        };

        // Performance metrics
        let blocks_per_hour = if production_stats.blocks_forged > 0 {
            // Estimate based on recent activity
            let hours = 1.0; // TODO: Calculate from actual time window
            production_stats.blocks_forged as f64 / hours
        } else {
            0.0
        };

        let avg_transactions_per_block = if production_stats.blocks_forged > 0 {
            production_stats.total_transactions as f64 / production_stats.blocks_forged as f64
        } else {
            0.0
        };

        // Calculate health score (0-100)
        let health_score = self.calculate_health_score(
            &production_metrics,
            &broadcast_metrics,
            time_since_last_block_secs,
        );

        let performance_metrics = PerformanceMetrics {
            blocks_per_hour,
            avg_transactions_per_block,
            health_score,
            time_since_last_block_secs,
        };

        let metrics = PipelineMetrics {
            timestamp,
            slot_metrics,
            production_metrics,
            broadcast_metrics,
            ledger_metrics,
            performance_metrics,
        };

        // Store in history
        let mut history = self.history.write().await;
        history.push(metrics.clone());

        // Trim history if too large
        if history.len() > self.max_history {
            history.remove(0);
        }

        metrics
    }

    /// Calculate overall pipeline health score (0-100)
    fn calculate_health_score(
        &self,
        production: &ProductionMetrics,
        broadcast: &BroadcastMetrics,
        time_since_last_block: u64,
    ) -> f64 {
        let mut score = 100.0;

        // Penalize for low production success rate
        if production.success_rate < 95.0 {
            score -= (95.0 - production.success_rate) * 0.5;
        }

        // Penalize for low broadcast success rate
        if broadcast.success_rate < 95.0 {
            score -= (95.0 - broadcast.success_rate) * 0.5;
        }

        // Penalize for stale block production (>1 hour)
        if time_since_last_block > 3600 {
            score -= 20.0;
        }

        // Penalize for high broadcast queue
        if broadcast.current_queue_size > 50 {
            score -= 10.0;
        }

        // Penalize for broadcast failures
        if broadcast.broadcast_failures > 10 {
            score -= 10.0;
        }

        score.max(0.0).min(100.0)
    }

    /// Get historical metrics
    pub async fn get_history(&self) -> Vec<PipelineMetrics> {
        self.history.read().await.clone()
    }

    /// Get metrics summary for the last N minutes
    pub async fn get_summary(&self, minutes: u64) -> Option<MetricsSummary> {
        let history = self.history.read().await;
        if history.is_empty() {
            return None;
        }

        let cutoff_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - (minutes * 60);

        let recent: Vec<_> = history
            .iter()
            .filter(|m| m.timestamp >= cutoff_time)
            .collect();

        if recent.is_empty() {
            return None;
        }

        let total_blocks = recent.last().unwrap().production_metrics.blocks_forged
            - recent.first().unwrap().production_metrics.blocks_forged;

        let avg_health = recent
            .iter()
            .map(|m| m.performance_metrics.health_score)
            .sum::<f64>()
            / recent.len() as f64;

        Some(MetricsSummary {
            time_window_minutes: minutes,
            blocks_produced: total_blocks,
            avg_health_score: avg_health,
            total_transactions: recent.last().unwrap().production_metrics.total_transactions
                - recent
                    .first()
                    .unwrap()
                    .production_metrics
                    .total_transactions,
        })
    }
}

/// Summary of metrics over a time window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    /// Time window in minutes
    pub time_window_minutes: u64,
    /// Total blocks produced in window
    pub blocks_produced: u64,
    /// Average health score
    pub avg_health_score: f64,
    /// Total transactions processed
    pub total_transactions: u64,
}

/// Prometheus exporter for metrics
pub struct PrometheusExporter;

impl PrometheusExporter {
    /// Export metrics in Prometheus text format
    pub fn export(metrics: &PipelineMetrics) -> String {
        let mut output = String::new();

        // Slot metrics
        output.push_str(&format!(
            "# HELP cardano_slot_current Current slot number\n"
        ));
        output.push_str(&format!("# TYPE cardano_slot_current gauge\n"));
        output.push_str(&format!(
            "cardano_slot_current {}\n",
            metrics.slot_metrics.current_slot
        ));

        output.push_str(&format!(
            "# HELP cardano_slot_subscribers Number of active slot event subscribers\n"
        ));
        output.push_str(&format!("# TYPE cardano_slot_subscribers gauge\n"));
        output.push_str(&format!(
            "cardano_slot_subscribers {}\n",
            metrics.slot_metrics.active_subscribers
        ));

        // Production metrics
        output.push_str(&format!(
            "# HELP cardano_blocks_forged_total Total blocks successfully forged\n"
        ));
        output.push_str(&format!("# TYPE cardano_blocks_forged_total counter\n"));
        output.push_str(&format!(
            "cardano_blocks_forged_total {}\n",
            metrics.production_metrics.blocks_forged
        ));

        output.push_str(&format!(
            "# HELP cardano_leadership_won_total Total times leadership was won\n"
        ));
        output.push_str(&format!("# TYPE cardano_leadership_won_total counter\n"));
        output.push_str(&format!(
            "cardano_leadership_won_total {}\n",
            metrics.production_metrics.leadership_won
        ));

        output.push_str(&format!(
            "# HELP cardano_production_success_rate Block production success rate percentage\n"
        ));
        output.push_str(&format!("# TYPE cardano_production_success_rate gauge\n"));
        output.push_str(&format!(
            "cardano_production_success_rate {}\n",
            metrics.production_metrics.success_rate
        ));

        // Broadcast metrics
        output.push_str(&format!(
            "# HELP cardano_blocks_broadcast_total Total blocks broadcast to network\n"
        ));
        output.push_str(&format!("# TYPE cardano_blocks_broadcast_total counter\n"));
        output.push_str(&format!(
            "cardano_blocks_broadcast_total {}\n",
            metrics.broadcast_metrics.blocks_broadcast
        ));

        output.push_str(&format!(
            "# HELP cardano_broadcast_latency_ms Average broadcast latency in milliseconds\n"
        ));
        output.push_str(&format!("# TYPE cardano_broadcast_latency_ms gauge\n"));
        output.push_str(&format!(
            "cardano_broadcast_latency_ms {}\n",
            metrics.broadcast_metrics.avg_broadcast_latency_ms
        ));

        output.push_str(&format!(
            "# HELP cardano_broadcast_queue_size Current broadcast queue size\n"
        ));
        output.push_str(&format!("# TYPE cardano_broadcast_queue_size gauge\n"));
        output.push_str(&format!(
            "cardano_broadcast_queue_size {}\n",
            metrics.broadcast_metrics.current_queue_size
        ));

        // Ledger metrics
        output.push_str(&format!(
            "# HELP cardano_utxo_count Current UTxO set size\n"
        ));
        output.push_str(&format!("# TYPE cardano_utxo_count gauge\n"));
        output.push_str(&format!(
            "cardano_utxo_count {}\n",
            metrics.ledger_metrics.utxo_count
        ));

        output.push_str(&format!(
            "# HELP cardano_total_value_lovelace Total value in UTxO set (lovelace)\n"
        ));
        output.push_str(&format!("# TYPE cardano_total_value_lovelace gauge\n"));
        output.push_str(&format!(
            "cardano_total_value_lovelace {}\n",
            metrics.ledger_metrics.total_value
        ));

        output.push_str(&format!(
            "# HELP cardano_transactions_processed_total Total transactions processed\n"
        ));
        output.push_str(&format!(
            "# TYPE cardano_transactions_processed_total counter\n"
        ));
        output.push_str(&format!(
            "cardano_transactions_processed_total {}\n",
            metrics.ledger_metrics.transactions_processed
        ));

        // Performance metrics
        output.push_str(&format!(
            "# HELP cardano_health_score Overall pipeline health score (0-100)\n"
        ));
        output.push_str(&format!("# TYPE cardano_health_score gauge\n"));
        output.push_str(&format!(
            "cardano_health_score {}\n",
            metrics.performance_metrics.health_score
        ));

        output.push_str(&format!(
            "# HELP cardano_blocks_per_hour Block production rate per hour\n"
        ));
        output.push_str(&format!("# TYPE cardano_blocks_per_hour gauge\n"));
        output.push_str(&format!(
            "cardano_blocks_per_hour {}\n",
            metrics.performance_metrics.blocks_per_hour
        ));

        output.push_str(&format!(
            "# HELP cardano_time_since_last_block_seconds Time since last block was produced\n"
        ));
        output.push_str(&format!(
            "# TYPE cardano_time_since_last_block_seconds gauge\n"
        ));
        output.push_str(&format!(
            "cardano_time_since_last_block_seconds {}\n",
            metrics.performance_metrics.time_since_last_block_secs
        ));

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ouroboros::SlotNo;

    #[tokio::test]
    async fn test_metrics_aggregator_creation() {
        let aggregator = MetricsAggregator::new(100);
        assert!(aggregator.get_history().await.is_empty());
    }

    #[tokio::test]
    async fn test_collect_metrics() {
        let aggregator = MetricsAggregator::new(100);

        let slot_stats = SlotNotifierStats {
            active_subscribers: 5,
            current_slot: Some(SlotNo(12345)),
        };

        let production_stats = BlockProductionStats {
            slots_checked: 1000,
            leadership_won: 10,
            blocks_forged: 9,
            forging_failures: 1,
            kes_evolutions: 2,
            total_transactions: 450,
        };

        let broadcast_stats = BroadcastStats {
            blocks_broadcast: 9,
            blocks_dropped: 0,
            broadcast_failures: 1,
            broadcast_retries: 2,
            total_peer_broadcasts: 45,
            current_queue_size: 0,
            avg_broadcast_latency_ms: 150,
        };

        let metrics = aggregator
            .collect_metrics(
                slot_stats,
                production_stats,
                broadcast_stats,
                50000,
                1_000_000_000_000,
                450,
                12.5,
                100,
                12345,
            )
            .await;

        assert_eq!(metrics.slot_metrics.current_slot, 12345);
        assert_eq!(metrics.production_metrics.blocks_forged, 9);
        assert_eq!(metrics.broadcast_metrics.blocks_broadcast, 9);
        assert_eq!(metrics.ledger_metrics.utxo_count, 50000);
        assert!(metrics.performance_metrics.health_score > 0.0);
    }

    #[tokio::test]
    async fn test_prometheus_export() {
        let metrics = PipelineMetrics {
            timestamp: 1000000,
            slot_metrics: SlotMetrics {
                current_slot: 12345,
                active_subscribers: 5,
                avg_drift_ms: 2.5,
                max_drift_ms: 10,
                high_drift_count: 3,
            },
            production_metrics: ProductionMetrics {
                slots_checked: 1000,
                leadership_won: 10,
                blocks_forged: 9,
                forging_failures: 1,
                kes_evolutions: 2,
                total_transactions: 450,
                success_rate: 90.0,
            },
            broadcast_metrics: BroadcastMetrics {
                blocks_broadcast: 9,
                blocks_dropped: 0,
                broadcast_failures: 1,
                broadcast_retries: 2,
                total_peer_broadcasts: 45,
                current_queue_size: 0,
                avg_broadcast_latency_ms: 150,
                success_rate: 88.9,
            },
            ledger_metrics: LedgerMetrics {
                utxo_count: 50000,
                total_value: 1_000_000_000_000,
                transactions_processed: 450,
                avg_validation_time_ms: 12.5,
                current_epoch: 100,
                current_slot: 12345,
            },
            performance_metrics: PerformanceMetrics {
                blocks_per_hour: 9.0,
                avg_transactions_per_block: 50.0,
                health_score: 85.0,
                time_since_last_block_secs: 300,
            },
        };

        let prometheus_output = PrometheusExporter::export(&metrics);

        assert!(prometheus_output.contains("cardano_slot_current 12345"));
        assert!(prometheus_output.contains("cardano_blocks_forged_total 9"));
        assert!(prometheus_output.contains("cardano_health_score 85"));
    }

    #[tokio::test]
    async fn test_health_score_calculation() {
        let aggregator = MetricsAggregator::new(100);

        let production = ProductionMetrics {
            slots_checked: 100,
            leadership_won: 10,
            blocks_forged: 10,
            forging_failures: 0,
            kes_evolutions: 0,
            total_transactions: 500,
            success_rate: 100.0,
        };

        let broadcast = BroadcastMetrics {
            blocks_broadcast: 10,
            blocks_dropped: 0,
            broadcast_failures: 0,
            broadcast_retries: 0,
            total_peer_broadcasts: 100,
            current_queue_size: 5,
            avg_broadcast_latency_ms: 100,
            success_rate: 100.0,
        };

        let score = aggregator.calculate_health_score(&production, &broadcast, 300);
        assert_eq!(score, 100.0);

        // Test with degraded performance
        let bad_broadcast = BroadcastMetrics {
            success_rate: 80.0,
            current_queue_size: 60,
            broadcast_failures: 15,
            ..broadcast
        };

        let degraded_score = aggregator.calculate_health_score(&production, &bad_broadcast, 300);
        assert!(degraded_score < 100.0);
    }
}
