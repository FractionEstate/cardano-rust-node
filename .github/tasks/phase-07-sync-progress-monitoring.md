# N1 Roadmap - Task 7: Sync Progress Monitoring

**Task 7:** Sync Progress Monitoring and Dashboard

- **Status**: Not Started
- **Files**:
  - Create: `crates/cardano-node/src/dashboard.rs` (~300 lines)
  - Create: `crates/cardano-node/src/metrics_collector.rs` (~200 lines)
  - Create: `crates/cardano-consensus/src/sync_monitor.rs` (~250 lines)
  - Modify: `crates/cardano-node/src/run/mod.rs` (integrate dashboard)
- **Description**: Implement real-time sync progress monitoring with dashboard display, metrics collection, and logging infrastructure for tracking node synchronization status.

## Task Checklist

### Metrics Collection

- [ ] Create metrics collector service
- [ ] Track blocks synced count
- [ ] Track current slot number
- [ ] Calculate distance from network tip
- [ ] Monitor peer connection status
- [ ] Track sync rate (blocks/sec, headers/sec)
- [ ] Measure validation throughput
- [ ] Monitor ledger database size
- [ ] Track memory usage
- [ ] Monitor network bandwidth usage

### Sync State Monitoring

- [ ] Implement SyncMonitor service
- [ ] Track sync phases (connecting, syncing, in-sync)
- [ ] Calculate ETA to sync completion
- [ ] Monitor sync health (stalls, errors)
- [ ] Track chain quality metrics
- [ ] Detect sync regressions
- [ ] Alert on sync failures

### Dashboard Display

- [ ] Create terminal-based dashboard (TUI)
- [ ] Display current sync status
- [ ] Show progress bar for sync
- [ ] Display peer connection table
- [ ] Show real-time metrics (TPS, bandwidth)
- [ ] Display recent log entries
- [ ] Add keyboard controls (pause/resume, quit)
- [ ] Implement auto-refresh (1 sec interval)

### Logging Infrastructure

- [ ] Configure structured logging
- [ ] Add log levels (trace, debug, info, warn, error)
- [ ] Implement log rotation
- [ ] Add log filtering by component
- [ ] Create log file output
- [ ] Add JSON log format option
- [ ] Implement performance logging

### Alerting System

- [ ] Alert on sync stalls (no progress for 5 min)
- [ ] Alert on validation failures
- [ ] Alert on peer disconnections
- [ ] Alert on memory threshold exceeded
- [ ] Alert on disk space low
- [ ] Send alerts to dashboard and logs

## Detailed Implementation

### File 1: `crates/cardano-consensus/src/sync_monitor.rs`

```rust
/// Sync monitoring service
pub struct SyncMonitor {
    metrics: Arc<RwLock<SyncMetrics>>,
    start_time: Instant,
    last_update: Arc<RwLock<Instant>>,
}

#[derive(Clone, Debug)]
pub struct SyncMetrics {
    pub current_slot: u64,
    pub current_block: u64,
    pub current_epoch: u32,
    pub network_tip_slot: u64,
    pub slots_behind: i64,
    pub sync_percentage: f64,
    pub blocks_per_second: f64,
    pub validation_rate: f64,
    pub connected_peers: usize,
    pub active_peers: usize,
    pub total_headers_received: u64,
    pub total_headers_validated: u64,
    pub validation_failures: u64,
    pub sync_state: SyncState,
}

impl SyncMonitor {
    pub fn new() -> Self
    pub async fn update_slot(&self, slot: u64)
    pub async fn update_network_tip(&self, tip_slot: u64)
    pub async fn record_header_received(&self)
    pub async fn record_header_validated(&self)
    pub async fn record_validation_failure(&self)
    pub async fn update_peer_count(&self, connected: usize, active: usize)
    pub async fn get_metrics(&self) -> SyncMetrics
    pub async fn calculate_eta(&self) -> Option<Duration>
    pub async fn is_stalled(&self) -> bool
}
```

### File 2: `crates/cardano-node/src/dashboard.rs`

```rust
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Gauge, Paragraph, Table},
    Terminal,
};

/// Terminal dashboard for node monitoring
pub struct Dashboard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    sync_monitor: Arc<SyncMonitor>,
    refresh_rate: Duration,
}

impl Dashboard {
    pub fn new(sync_monitor: Arc<SyncMonitor>) -> Result<Self>

    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;

        loop {
            self.draw().await?;

            // Check for quit command
            if event::poll(self.refresh_rate)? {
                if let Event::Key(key) = event::read()? {
                    if key.code == KeyCode::Char('q') {
                        break;
                    }
                }
            }
        }

        disable_raw_mode()?;
        Ok(())
    }

    async fn draw(&mut self) -> Result<()> {
        let metrics = self.sync_monitor.get_metrics().await;

        self.terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Header
                    Constraint::Length(5),  // Sync progress
                    Constraint::Length(8),  // Metrics table
                    Constraint::Length(6),  // Peer table
                    Constraint::Min(0),     // Logs
                ])
                .split(f.size());

            // Header
            let header = Paragraph::new(format!(
                "Cardano Rust Node - Sync Monitor | Slot: {} | Epoch: {}",
                metrics.current_slot, metrics.current_epoch
            ))
            .block(Block::default().borders(Borders::ALL));
            f.render_widget(header, chunks[0]);

            // Sync progress bar
            let progress = Gauge::default()
                .block(Block::default().title("Sync Progress").borders(Borders::ALL))
                .gauge_style(Style::default().fg(Color::Green))
                .percent(metrics.sync_percentage as u16);
            f.render_widget(progress, chunks[1]);

            // Metrics table
            // ... render metrics ...

            // Peer table
            // ... render peer info ...

            // Logs
            // ... render recent logs ...
        })?;

        Ok(())
    }
}
```

### File 3: `crates/cardano-node/src/metrics_collector.rs`

```rust
/// Central metrics collection service
pub struct MetricsCollector {
    sync_metrics: Arc<RwLock<SyncMetrics>>,
    system_metrics: Arc<RwLock<SystemMetrics>>,
    network_metrics: Arc<RwLock<NetworkMetrics>>,
}

#[derive(Clone, Debug)]
pub struct SystemMetrics {
    pub memory_usage_mb: u64,
    pub cpu_usage_percent: f64,
    pub disk_usage_gb: f64,
    pub uptime: Duration,
}

#[derive(Clone, Debug)]
pub struct NetworkMetrics {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub average_latency_ms: f64,
}

impl MetricsCollector {
    pub fn new() -> Self
    pub async fn collect_system_metrics(&self)
    pub async fn collect_network_metrics(&self)
    pub async fn export_prometheus(&self) -> String
    pub async fn export_json(&self) -> String
    pub async fn log_metrics(&self)
}
```

### Integration with Node Runtime

Modify `crates/cardano-node/src/run/mod.rs`:

```rust
pub async fn run_node(config: NodeConfig) -> Result<()> {
    // Initialize components
    let ledger = create_ledger(&config).await?;
    let validator = BlockValidator::new(ledger.clone());
    let sync_service = ChainSyncService::default();

    // Create monitoring
    let sync_monitor = Arc::new(SyncMonitor::new());
    let metrics_collector = Arc::new(MetricsCollector::new());

    // Create dashboard
    let dashboard = Dashboard::new(sync_monitor.clone())?;

    // Spawn monitoring tasks
    let monitor_task = tokio::spawn(async move {
        loop {
            sync_monitor.update_from_services(&sync_service, &validator).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    let metrics_task = tokio::spawn(async move {
        loop {
            metrics_collector.collect_system_metrics().await;
            metrics_collector.log_metrics().await;
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });

    // Run dashboard (blocks until quit)
    dashboard.run().await?;

    // Cleanup
    monitor_task.abort();
    metrics_task.abort();

    Ok(())
}
```

## Success Criteria

- [ ] Dashboard displays in terminal
- [ ] Real-time metrics update every second
- [ ] Sync progress accurately calculated
- [ ] ETA to completion displayed
- [ ] Peer status visible
- [ ] Logs displayed in real-time
- [ ] Keyboard controls work (q to quit)
- [ ] Metrics exported to Prometheus format
- [ ] Logging to file works
- [ ] Alerts trigger on sync issues

## Dependencies

- Phase 05: BlockValidator metrics
- Phase 06: ChainSync integration metrics
- External: `crossterm` crate for TUI
- External: `tui` crate for dashboard UI
- External: `sysinfo` crate for system metrics

## Testing Commands

```bash
# Run node with dashboard
cargo run --bin cardano-node -- --config config/preview.yaml

# Test metrics collection
cargo test -p cardano-node metrics_collector

# Test sync monitor
cargo test -p cardano-consensus sync_monitor

# Export metrics to file
cargo run --bin cardano-node -- --export-metrics metrics.json
```

## Estimated Effort

- Metrics collection: 2-3 hours
- Sync monitor: 2-3 hours
- TUI dashboard: 4-5 hours
- Integration: 1-2 hours
- Testing: 1-2 hours
- **Total: 10-15 hours**

## Notes

- Dashboard requires terminal with ANSI color support
- May need to disable dashboard for CI/CD environments
- Consider web-based dashboard as future enhancement
- Prometheus metrics enable integration with Grafana
- Log rotation prevents disk space exhaustion
