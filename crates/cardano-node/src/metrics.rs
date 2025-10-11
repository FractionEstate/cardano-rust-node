//! Metrics Conversion Utilities
//!
//! Provides conversion functions between different metric types across crates.

use cardano_consensus::{SyncMetrics, SyncState as ConsensusSyncState};
use cardano_network::services::SyncStats;
use std::time::Duration;

use crate::dashboard::NodeStats;

/// Convert SyncMetrics from consensus layer to Dashboard NodeStats
///
/// Maps comprehensive sync monitoring metrics to the dashboard's display format.
pub fn sync_metrics_to_node_stats(metrics: &SyncMetrics) -> NodeStats {
    NodeStats {
        // Blockchain state
        chain_tip: metrics.current_slot,
        total_blocks: metrics.network_tip_slot,
        sync_progress: metrics.sync_percentage / 100.0, // Dashboard uses 0.0-1.0 range
        epoch: metrics.current_epoch as u64,
        slot: metrics.current_slot,

        // Peer information
        peer_count: metrics.connected_peers,

        // Performance metrics
        blocks_forged: 0, // Not tracked during sync, only during block production
        tx_processed: 0,  // Not available from SyncMetrics
        mempool_size: 0,  // Not available from SyncMetrics

        // System resources (not tracked by SyncMetrics - would need separate system monitor)
        cpu_usage: 0.0,
        memory_usage: 0,
        disk_usage: 0,
        network_in: 0,
        network_out: 0,

        // Uptime calculation
        uptime: metrics
            .last_update
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO),

        // Stake information (not available during sync)
        active_stake: 0,
    }
}

/// Update SyncMetrics fields from ChainSyncService stats
///
/// This is used by the integration service to keep SyncMonitor updated
/// with data from the network layer.
pub fn apply_sync_stats_to_metrics(current_metrics: &mut SyncMetrics, sync_stats: &SyncStats) {
    // Update peer information
    current_metrics.connected_peers = sync_stats.active_peers;
    current_metrics.active_peers = sync_stats.active_peers;

    // Update sync state
    current_metrics.sync_state = match &sync_stats.state {
        cardano_network::services::SyncState::Idle => ConsensusSyncState::Starting,
        cardano_network::services::SyncState::FindingIntersection => ConsensusSyncState::Connecting,
        cardano_network::services::SyncState::Syncing => ConsensusSyncState::Syncing,
        cardano_network::services::SyncState::InSync => ConsensusSyncState::InSync,
        cardano_network::services::SyncState::Failed(_) => ConsensusSyncState::Error,
    };

    // Update tip information
    current_metrics.network_tip_slot = sync_stats.network_tip_slot;
    current_metrics.current_slot = sync_stats.local_tip_slot;
    current_metrics.slots_behind = sync_stats.slots_behind;

    // Update header count
    current_metrics.total_headers_received = sync_stats.total_headers;

    // Recalculate sync percentage
    if current_metrics.network_tip_slot > 0 {
        current_metrics.sync_percentage =
            (current_metrics.current_slot as f64 / current_metrics.network_tip_slot as f64 * 100.0)
                .min(100.0);
    }

    // Update timestamp
    current_metrics.last_update = std::time::SystemTime::now();
}

/// Format duration for display in dashboard
pub fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let days = total_seconds / 86400;
    let hours = (total_seconds % 86400) / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

/// Format ETA for display
pub fn format_eta(eta: Option<Duration>) -> String {
    match eta {
        Some(duration) => {
            let total_seconds = duration.as_secs();
            if total_seconds > 86400 {
                let days = total_seconds / 86400;
                let hours = (total_seconds % 86400) / 3600;
                format!("~{}d {}h", days, hours)
            } else if total_seconds > 3600 {
                let hours = total_seconds / 3600;
                let minutes = (total_seconds % 3600) / 60;
                format!("~{}h {}m", hours, minutes)
            } else if total_seconds > 60 {
                let minutes = total_seconds / 60;
                format!("~{} minutes", minutes)
            } else {
                format!("~{} seconds", total_seconds)
            }
        }
        None => "Calculating...".to_string(),
    }
}

/// Format sync percentage with color coding
pub fn format_sync_percentage(percentage: f64) -> String {
    format!("{:.2}%", percentage)
}

/// Format blocks per second rate
pub fn format_rate(rate: f64) -> String {
    if rate >= 1.0 {
        format!("{:.2} blocks/s", rate)
    } else if rate > 0.0 {
        let seconds_per_block = 1.0 / rate;
        format!("{:.1} s/block", seconds_per_block)
    } else {
        "---".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_metrics_to_node_stats() {
        let metrics = SyncMetrics {
            current_slot: 1000,
            current_block: 1000,
            current_epoch: 100,
            network_tip_slot: 2000,
            slots_behind: 1000,
            sync_percentage: 50.0,
            blocks_per_second: 2.5,
            validation_rate: 2.5,
            connected_peers: 5,
            active_peers: 3,
            total_headers_received: 1000,
            total_headers_validated: 950,
            validation_failures: 50,
            sync_state: ConsensusSyncState::Syncing,
            eta: Some(Duration::from_secs(400)),
            last_update: std::time::SystemTime::now(),
        };

        let node_stats = sync_metrics_to_node_stats(&metrics);

        assert_eq!(node_stats.chain_tip, 1000);
        assert_eq!(node_stats.total_blocks, 2000);
        assert_eq!(node_stats.sync_progress, 0.5); // 50% as decimal
        assert_eq!(node_stats.epoch, 100);
        assert_eq!(node_stats.slot, 1000);
        assert_eq!(node_stats.peer_count, 5);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3661)), "1h 1m 1s");
        assert_eq!(format_duration(Duration::from_secs(90000)), "1d 1h 0m");
    }

    #[test]
    fn test_format_eta() {
        assert_eq!(format_eta(Some(Duration::from_secs(30))), "~30 seconds");
        assert_eq!(format_eta(Some(Duration::from_secs(120))), "~2 minutes");
        assert_eq!(format_eta(Some(Duration::from_secs(7200))), "~2h 0m");
        assert_eq!(format_eta(Some(Duration::from_secs(90000))), "~1d 1h");
        assert_eq!(format_eta(None), "Calculating...");
    }

    #[test]
    fn test_format_rate() {
        assert_eq!(format_rate(2.5), "2.50 blocks/s");
        assert_eq!(format_rate(0.5), "2.0 s/block");
        assert_eq!(format_rate(0.0), "---");
    }

    #[test]
    fn test_apply_sync_stats_to_metrics() {
        let mut metrics = SyncMetrics::default();
        let sync_stats = SyncStats {
            active_peers: 5,
            total_headers: 1000,
            state: cardano_network::services::SyncState::Syncing,
            local_tip_slot: 500,
            network_tip_slot: 1000,
            slots_behind: 500,
        };

        apply_sync_stats_to_metrics(&mut metrics, &sync_stats);

        assert_eq!(metrics.connected_peers, 5);
        assert_eq!(metrics.active_peers, 5);
        assert_eq!(metrics.network_tip_slot, 1000);
        assert_eq!(metrics.current_slot, 500);
        assert_eq!(metrics.slots_behind, 500);
        assert_eq!(metrics.total_headers_received, 1000);
        assert_eq!(metrics.sync_percentage, 50.0);
        assert_eq!(metrics.sync_state, ConsensusSyncState::Syncing);
    }
}
