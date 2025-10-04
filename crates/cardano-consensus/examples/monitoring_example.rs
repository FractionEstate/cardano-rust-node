//! Example: Complete Block Production Pipeline with Monitoring
//!
//! This example demonstrates how to set up the complete block production pipeline
//! with comprehensive metrics collection and monitoring.

use cardano_consensus::{
    BlockBroadcaster, BlockBroadcasterConfig, BlockProductionConfig,
    ForgingConfig, LedgerState, LedgerProtocolParameters,
    MetricsAggregator, PoolId, PrometheusExporter, SlotNotifier, SlotNotifierConfig,
};
use cardano_crypto::hash::Blake2b256Hash;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Cardano Block Production Pipeline with Monitoring");

    // 1. Create metrics aggregator
    let metrics_aggregator = Arc::new(MetricsAggregator::new(1000));
    info!("✓ Metrics aggregator initialized (history: 1000 data points)");

    // 2. Initialize ledger state
    let protocol_params = LedgerProtocolParameters {
        min_fee_a: 44,
        min_fee_b: 155_381,
        max_tx_size: 16384,
        max_block_size: 90112,
        min_utxo_value: 1_000_000,
        max_tx_per_block: 10000,
    };
    let ledger_state = Arc::new(LedgerState::with_protocol_params(protocol_params));
    info!("✓ Ledger state initialized with Conway era parameters");

    // 3. Create slot notifier
    let slot_config = SlotNotifierConfig {
        slot_length_secs: 1,
        genesis_time: std::time::SystemTime::now(),
        max_drift_ms: 100,
        channel_size: 100,
    };
    let slot_notifier = Arc::new(SlotNotifier::new(slot_config));
    info!("✓ Slot notifier created (1s slots, 5-day epochs)");

    // 4. Create block forger
    let forging_config = ForgingConfig::default();
    let pool_id = PoolId(Blake2b256Hash::from_bytes(&[1u8; 32]).unwrap());

    // Note: In production, load real keys from secure storage
    info!("✓ Block forger configuration prepared");

    // 5. Create block production service
    let _production_config = BlockProductionConfig {
        pool_id: pool_id.clone(),
        pool_stake: 5_000_000_000_000,    // 5M ADA
        total_stake: 35_000_000_000_000_000, // 35B ADA (mainnet total)
        active_slot_coeff: 0.05,           // 5% active slot coefficient
        epoch: cardano_consensus::EpochNo(500),
        epoch_nonce: Blake2b256Hash::from_bytes(&[2u8; 32]).unwrap(),
        forging_config,
    };
    info!("✓ Block production service configured");

    // 6. Create block broadcaster
    let broadcast_config = BlockBroadcasterConfig::default();
    let broadcaster = Arc::new(BlockBroadcaster::new(broadcast_config));
    info!("✓ Block broadcaster initialized");

    // 7. Start metrics collection task
    let metrics_task = {
        let metrics = metrics_aggregator.clone();
        let notifier = slot_notifier.clone();
        let ledger = ledger_state.clone();
        let broad = broadcaster.clone();

        tokio::spawn(async move {
            let mut collection_interval = interval(Duration::from_secs(60));

            info!("Starting metrics collection (60s interval)");

            loop {
                collection_interval.tick().await;

                // Collect stats from all components
                let slot_stats = notifier.stats();

                // In a real implementation, you would get these from the actual services
                let production_stats = cardano_consensus::BlockProductionStats {
                    slots_checked: 3600,
                    leadership_won: 5,
                    blocks_forged: 4,
                    forging_failures: 1,
                    kes_evolutions: 0,
                    total_transactions: 200,
                };

                let broadcast_stats = broad.stats().await;

                // Get ledger stats
                let ledger_epoch = ledger.current_epoch().await;
                let ledger_slot = ledger.current_slot().await;

                // Collect metrics
                let pipeline_metrics = metrics
                    .collect_metrics(
                        slot_stats,
                        production_stats,
                        broadcast_stats,
                        50000,              // utxo_count
                        1_000_000_000_000,  // total_value (1M ADA)
                        200,                // transactions_processed
                        12.5,               // avg_validation_time_ms
                        ledger_epoch,
                        ledger_slot,
                    )
                    .await;

                info!(
                    "Metrics collected - Slot: {}, Health: {:.1}, Blocks: {}",
                    pipeline_metrics.slot_metrics.current_slot,
                    pipeline_metrics.performance_metrics.health_score,
                    pipeline_metrics.production_metrics.blocks_forged,
                );

                // Log warnings for degraded health
                if pipeline_metrics.performance_metrics.health_score < 80.0 {
                    error!(
                        "⚠️  Pipeline health degraded: {:.1}/100",
                        pipeline_metrics.performance_metrics.health_score
                    );
                }

                // Export to Prometheus format
                let prometheus_text = PrometheusExporter::export(&pipeline_metrics);

                // In production, serve this via HTTP endpoint or write to file
                if std::env::var("METRICS_OUTPUT").is_ok() {
                    if let Err(e) = std::fs::write("/tmp/cardano_metrics.prom", &prometheus_text) {
                        error!("Failed to write metrics: {}", e);
                    }
                }
            }
        })
    };

    // 8. Start metrics summary reporting task
    let summary_task = {
        let metrics = metrics_aggregator.clone();

        tokio::spawn(async move {
            let mut report_interval = interval(Duration::from_secs(3600)); // 1 hour

            loop {
                report_interval.tick().await;

                // Get summary for last hour
                if let Some(summary) = metrics.get_summary(60).await {
                    info!(
                        "📊 Hourly Summary: {} blocks produced, {} transactions, health: {:.1}/100",
                        summary.blocks_produced,
                        summary.total_transactions,
                        summary.avg_health_score,
                    );
                }
            }
        })
    };

    // 9. Start HTTP metrics endpoint (optional)
    let metrics_server = {
        let metrics = metrics_aggregator.clone();

        tokio::spawn(async move {
            use axum::{extract::State, routing::get, Router};

            async fn metrics_handler(
                State(aggregator): State<Arc<MetricsAggregator>>,
            ) -> String {
                let history = aggregator.get_history().await;
                if let Some(latest) = history.last() {
                    PrometheusExporter::export(latest)
                } else {
                    String::new()
                }
            }

            async fn health_handler(
                State(aggregator): State<Arc<MetricsAggregator>>,
            ) -> String {
                let history = aggregator.get_history().await;
                if let Some(latest) = history.last() {
                    format!(
                        "{{\"status\":\"ok\",\"health_score\":{:.1},\"blocks\":{},\"slot\":{}}}",
                        latest.performance_metrics.health_score,
                        latest.production_metrics.blocks_forged,
                        latest.slot_metrics.current_slot,
                    )
                } else {
                    "{\"status\":\"starting\"}".to_string()
                }
            }

            let app = Router::new()
                .route("/metrics", get(metrics_handler))
                .route("/health", get(health_handler))
                .with_state(metrics);

            let addr = "0.0.0.0:9090";
            info!("🌐 Metrics endpoint available at http://{}/metrics", addr);
            info!("🏥 Health endpoint available at http://{}/health", addr);

            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            if let Err(e) = axum::serve(listener, app.into_make_service()).await {
                error!("Metrics server error: {}", e);
            }
        })
    };

    info!("✅ All components started successfully");
    info!("");
    info!("📈 Monitoring endpoints:");
    info!("   - Metrics: http://localhost:9090/metrics (Prometheus format)");
    info!("   - Health:  http://localhost:9090/health  (JSON status)");
    info!("");
    info!("Press Ctrl+C to stop");

    // Wait for all tasks
    tokio::select! {
        _ = metrics_task => error!("Metrics collection stopped"),
        _ = summary_task => error!("Summary reporting stopped"),
        _ = metrics_server => error!("Metrics server stopped"),
        _ = tokio::signal::ctrl_c() => {
            info!("Shutdown signal received");
        }
    }

    // Final metrics report
    if let Some(summary) = metrics_aggregator.get_summary(60).await {
        info!("📊 Final Report:");
        info!("   Blocks produced: {}", summary.blocks_produced);
        info!("   Transactions: {}", summary.total_transactions);
        info!("   Avg health: {:.1}/100", summary.avg_health_score);
    }

    info!("Shutdown complete");
    Ok(())
}
