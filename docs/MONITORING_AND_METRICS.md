# Block Production Monitoring and Metrics

This document describes the comprehensive monitoring and metrics system for the Cardano Node Rust block production pipeline.

## Overview

The monitoring system collects metrics from all components of the block production pipeline and exports them in Prometheus format for dashboards and alerting.

## Architecture

```text
┌─────────────────┐
│ SlotNotifier    │─┐
└─────────────────┘ │
┌─────────────────┐ │
│ BlockProduction │─┤
└─────────────────┘ │    ┌─────────────────┐    ┌──────────────┐
┌─────────────────┐ ├───▶│ MetricsAggregator│───▶│ Prometheus   │
│ BlockBroadcaster│─┤    └─────────────────┘    │ Exporter     │
└─────────────────┘ │                            └──────────────┘
┌─────────────────┐ │
│ LedgerState     │─┘
└─────────────────┘
```

## Components

### MetricsAggregator

The central metrics collection component that:
- Gathers metrics from all pipeline components
- Maintains historical data for trend analysis
- Calculates aggregated performance metrics
- Computes overall health scores

### PrometheusExporter

Exports metrics in Prometheus text format for integration with monitoring systems.

## Metrics Categories

### 1. Slot Metrics

- **current_slot**: Current blockchain slot number
- **active_subscribers**: Number of components listening to slot events
- **avg_drift_ms**: Average slot timing drift in milliseconds
- **max_drift_ms**: Maximum observed drift
- **high_drift_count**: Number of slots with excessive drift

### 2. Block Production Metrics

- **slots_checked**: Total slots where leadership was evaluated
- **leadership_won**: Number of times this node was slot leader
- **blocks_forged**: Successfully produced blocks
- **forging_failures**: Failed block production attempts
- **kes_evolutions**: KES key evolution events
- **total_transactions**: Total transactions included in blocks
- **success_rate**: Percentage of successful block productions (blocks_forged / leadership_won)

### 3. Block Broadcasting Metrics

- **blocks_broadcast**: Total blocks sent to network
- **blocks_dropped**: Blocks dropped due to queue overflow
- **broadcast_failures**: Failed broadcast attempts
- **broadcast_retries**: Number of retry attempts
- **total_peer_broadcasts**: Total peer broadcast operations
- **current_queue_size**: Current broadcast queue depth
- **avg_broadcast_latency_ms**: Average time to broadcast a block
- **success_rate**: Percentage of successful broadcasts

### 4. Ledger State Metrics

- **utxo_count**: Current size of UTxO set
- **total_value**: Total ADA in UTxO set (lovelace)
- **transactions_processed**: Cumulative transactions validated
- **avg_validation_time_ms**: Average transaction validation time
- **current_epoch**: Current epoch number
- **current_slot**: Current slot in ledger view

### 5. Performance Metrics

- **blocks_per_hour**: Block production rate
- **avg_transactions_per_block**: Average transaction count per block
- **health_score**: Overall pipeline health (0-100)
- **time_since_last_block_secs**: Seconds since last block production

## Usage

### Basic Setup

```rust
use cardano_consensus::{
    MetricsAggregator, PrometheusExporter,
    SlotNotifier, BlockProductionService, BlockBroadcaster, LedgerState,
};

#[tokio::main]
async fn main() {
    // Create metrics aggregator (keeps last 1000 data points)
    let metrics = MetricsAggregator::new(1000);

    // Create pipeline components
    let slot_notifier = SlotNotifier::new(config);
    let production_service = BlockProductionService::new(prod_config, forger);
    let broadcaster = BlockBroadcaster::new(broadcast_config);
    let ledger_state = LedgerState::new();

    // Start metrics collection loop
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));

        loop {
            interval.tick().await;

            // Collect stats from all components
            let slot_stats = slot_notifier.stats();
            let prod_stats = production_service.stats().await;
            let broadcast_stats = broadcaster.stats().await;

            // Collect metrics
            let pipeline_metrics = metrics.collect_metrics(
                slot_stats,
                prod_stats,
                broadcast_stats,
                ledger_utxo_count,
                ledger_total_value,
                ledger_transactions,
                ledger_validation_time,
                ledger_epoch,
                ledger_slot,
            ).await;

            // Export to Prometheus format
            let prometheus_text = PrometheusExporter::export(&pipeline_metrics);

            // Write to metrics endpoint or file
            write_metrics(&prometheus_text).await;
        }
    });
}
```

### Recording Block Production Events

```rust
// When a block is successfully produced
metrics.record_block_production().await;
```

### Getting Metrics Summary

```rust
// Get summary for last 60 minutes
if let Some(summary) = metrics.get_summary(60).await {
    println!("Blocks in last hour: {}", summary.blocks_produced);
    println!("Avg health score: {:.1}", summary.avg_health_score);
    println!("Total transactions: {}", summary.total_transactions);
}
```

### Accessing Historical Data

```rust
let history = metrics.get_history().await;
for metric in history {
    println!("Slot {}: {} blocks, health: {:.1}",
        metric.slot_metrics.current_slot,
        metric.production_metrics.blocks_forged,
        metric.performance_metrics.health_score
    );
}
```

## Prometheus Integration

### Metrics Endpoint

Expose metrics via HTTP endpoint:

```rust
use axum::{routing::get, Router};

async fn metrics_handler(
    State(aggregator): State<Arc<MetricsAggregator>>,
) -> String {
    // Get latest metrics
    let history = aggregator.get_history().await;
    if let Some(latest) = history.last() {
        PrometheusExporter::export(latest)
    } else {
        String::new()
    }
}

let app = Router::new()
    .route("/metrics", get(metrics_handler))
    .with_state(Arc::new(metrics_aggregator));

axum::Server::bind(&"0.0.0.0:9090".parse().unwrap())
    .serve(app.into_make_service())
    .await
    .unwrap();
```

### Prometheus Configuration

Add to `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'cardano-node'
    scrape_interval: 60s
    static_configs:
      - targets: ['localhost:9090']
        labels:
          instance: 'pool-1'
```

## Grafana Dashboard

### Key Panels

1. **Block Production Rate**
   - Query: `rate(cardano_blocks_forged_total[1h])`
   - Shows blocks per hour production rate

2. **Health Score**
   - Query: `cardano_health_score`
   - Overall pipeline health indicator

3. **Broadcast Latency**
   - Query: `cardano_broadcast_latency_ms`
   - Network propagation performance

4. **UTxO Set Size**
   - Query: `cardano_utxo_count`
   - Ledger state growth

5. **Leadership Success Rate**
   - Query: `cardano_production_success_rate`
   - Block production reliability

### Example Dashboard JSON

```json
{
  "dashboard": {
    "title": "Cardano Block Production",
    "panels": [
      {
        "title": "Blocks Per Hour",
        "targets": [
          {
            "expr": "rate(cardano_blocks_forged_total[1h])"
          }
        ]
      },
      {
        "title": "Pipeline Health",
        "targets": [
          {
            "expr": "cardano_health_score"
          }
        ]
      }
    ]
  }
}
```

## Health Score Calculation

The health score (0-100) is calculated based on:

- **Production Success Rate**: Penalized if < 95%
- **Broadcast Success Rate**: Penalized if < 95%
- **Block Staleness**: Penalized if >1 hour since last block
- **Queue Depth**: Penalized if broadcast queue > 50
- **Broadcast Failures**: Penalized if > 10 failures

Formula:
```
score = 100
- (95 - production_success_rate) * 0.5 (if < 95%)
- (95 - broadcast_success_rate) * 0.5 (if < 95%)
- 20 (if last_block > 1 hour)
- 10 (if queue_size > 50)
- 10 (if broadcast_failures > 10)
score = max(0, min(100, score))
```

## Alerting Rules

### Critical Alerts

```yaml
groups:
  - name: cardano_critical
    rules:
      - alert: BlockProductionStalled
        expr: cardano_time_since_last_block_seconds > 3600
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "No blocks produced in over 1 hour"

      - alert: LowHealthScore
        expr: cardano_health_score < 50
        for: 10m
        labels:
          severity: critical
        annotations:
          summary: "Pipeline health critically low"

      - alert: HighBroadcastFailures
        expr: rate(cardano_broadcast_failures_total[5m]) > 0.1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High rate of broadcast failures"
```

### Warning Alerts

```yaml
  - name: cardano_warnings
    rules:
      - alert: LowProductionRate
        expr: rate(cardano_blocks_forged_total[1h]) < 0.5
        for: 30m
        labels:
          severity: warning
        annotations:
          summary: "Block production rate below expected"

      - alert: HighBroadcastLatency
        expr: cardano_broadcast_latency_ms > 1000
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "High broadcast latency detected"
```

## Troubleshooting

### Low Health Score

1. Check production success rate
2. Verify broadcast success rate
3. Review recent errors in logs
4. Check KES key validity
5. Verify network connectivity

### High Broadcast Latency

1. Check network bandwidth
2. Verify peer connectivity
3. Review broadcast queue size
4. Check for network congestion

### Stalled Block Production

1. Verify slot notifier is running
2. Check VRF key validity
3. Review leadership calculation logs
4. Verify stake pool registration

## Performance Tuning

### Metrics Collection Frequency

- **High Frequency** (10-30s): Development, debugging
- **Medium Frequency** (60s): Production standard
- **Low Frequency** (5m): Resource-constrained environments

### History Retention

```rust
// Keep last 1000 data points (default)
let metrics = MetricsAggregator::new(1000);

// Extended history for trend analysis
let metrics = MetricsAggregator::new(10000);

// Minimal history for memory-constrained systems
let metrics = MetricsAggregator::new(100);
```

## JSON Export

Metrics can also be exported as JSON:

```rust
use serde_json;

let json = serde_json::to_string_pretty(&pipeline_metrics)?;
println!("{}", json);
```

## Testing

Run metrics tests:

```bash
cargo test --package cardano-consensus metrics
```

Expected output:
```
running 4 tests
test metrics::tests::test_metrics_aggregator_creation ... ok
test metrics::tests::test_collect_metrics ... ok
test metrics::tests::test_prometheus_export ... ok
test metrics::tests::test_health_score_calculation ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured
```

## References

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Cardano Node Metrics](https://docs.cardano.org/cardano-node/monitoring/)

## Future Enhancements

1. **Histogram Metrics**: Distribution of block production times
2. **Trend Analysis**: Automatic detection of performance degradation
3. **Predictive Alerts**: ML-based anomaly detection
4. **Cost Tracking**: Pool operation cost metrics
5. **Reward Metrics**: Block reward tracking and predictions
