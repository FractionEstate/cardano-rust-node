//! Connection Health Monitoring
//!
//! This module provides connection health monitoring, keepalive functionality,
//! timeout handling, and automatic reconnection logic for the Cardano network.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::task::JoinHandle;
use tokio::time::{interval, sleep, timeout};

use super::{ConnectionId, ConnectionError, ProtocolId};
use crate::diffusion::PeerId;
use crate::Result;

/// Connection health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Connection is healthy
    Healthy,
    /// Connection is degraded but functional
    Degraded,
    /// Connection is unhealthy and should be closed
    Unhealthy,
    /// Connection status unknown
    Unknown,
}

impl HealthStatus {
    /// Check if status indicates connection should be maintained
    pub fn is_acceptable(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }

    /// Check if connection requires attention
    pub fn needs_attention(&self) -> bool {
        matches!(self, Self::Degraded | Self::Unhealthy)
    }
}

/// Connection monitoring metrics
#[derive(Debug, Clone)]
pub struct ConnectionMetrics {
    /// Connection identifier
    pub connection_id: ConnectionId,
    /// Associated peer
    pub peer_id: PeerId,
    /// Last successful message timestamp
    pub last_message_at: Instant,
    /// Last keepalive sent
    pub last_keepalive_sent: Option<Instant>,
    /// Last keepalive response received
    pub last_keepalive_response: Option<Instant>,
    /// Number of consecutive failed keepalives
    pub failed_keepalives: u32,
    /// Round-trip time for recent messages
    pub recent_rtt: Option<Duration>,
    /// Number of protocol errors
    pub protocol_errors: u32,
    /// Bytes sent/received counters
    pub bytes_sent: u64,
    pub bytes_received: u64,
    /// Health status
    pub health: HealthStatus,
}

impl ConnectionMetrics {
    /// Create new metrics for connection
    pub fn new(connection_id: ConnectionId, peer_id: PeerId) -> Self {
        Self {
            connection_id,
            peer_id,
            last_message_at: Instant::now(),
            last_keepalive_sent: None,
            last_keepalive_response: None,
            failed_keepalives: 0,
            recent_rtt: None,
            protocol_errors: 0,
            bytes_sent: 0,
            bytes_received: 0,
            health: HealthStatus::Healthy,
        }
    }

    /// Update activity timestamp
    pub fn update_activity(&mut self) {
        self.last_message_at = Instant::now();
    }

    /// Record keepalive sent
    pub fn record_keepalive_sent(&mut self) {
        self.last_keepalive_sent = Some(Instant::now());
    }

    /// Record keepalive response
    pub fn record_keepalive_response(&mut self, sent_at: Instant) {
        self.last_keepalive_response = Some(Instant::now());
        self.failed_keepalives = 0;
        self.recent_rtt = Some(Instant::now().duration_since(sent_at));
    }

    /// Record failed keepalive
    pub fn record_keepalive_failure(&mut self) {
        self.failed_keepalives += 1;
    }

    /// Record protocol error
    pub fn record_protocol_error(&mut self) {
        self.protocol_errors += 1;
    }

    /// Get time since last activity
    pub fn idle_time(&self) -> Duration {
        Instant::now().duration_since(self.last_message_at)
    }

    /// Update health status based on metrics
    pub fn update_health(&mut self, config: &MonitorConfig) {
        let idle_time = self.idle_time();

        self.health = if self.failed_keepalives >= config.max_failed_keepalives {
            HealthStatus::Unhealthy
        } else if idle_time > config.unhealthy_threshold {
            HealthStatus::Unhealthy
        } else if idle_time > config.degraded_threshold || self.failed_keepalives > 0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };
    }
}

/// Keepalive message for connection health monitoring
#[derive(Debug, Clone)]
pub struct KeepAliveMessage {
    /// Unique cookie for matching responses
    pub cookie: u16,
    /// Message timestamp
    pub timestamp: SystemTime,
}

impl KeepAliveMessage {
    /// Create new keepalive message
    pub fn new(cookie: u16) -> Self {
        Self {
            cookie,
            timestamp: SystemTime::now(),
        }
    }
}

/// Keepalive response message
#[derive(Debug, Clone)]
pub struct KeepAliveResponse {
    /// Cookie from original message
    pub cookie: u16,
    /// Response timestamp
    pub timestamp: SystemTime,
}

impl KeepAliveResponse {
    /// Create response to keepalive
    pub fn new(cookie: u16) -> Self {
        Self {
            cookie,
            timestamp: SystemTime::now(),
        }
    }
}

/// Connection monitoring configuration
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Keepalive interval
    pub keepalive_interval: Duration,
    /// Keepalive response timeout
    pub keepalive_timeout: Duration,
    /// Maximum number of failed keepalives before marking unhealthy
    pub max_failed_keepalives: u32,
    /// Threshold for marking connection as degraded
    pub degraded_threshold: Duration,
    /// Threshold for marking connection as unhealthy
    pub unhealthy_threshold: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Enable automatic reconnection
    pub auto_reconnect: bool,
    /// Reconnection delay
    pub reconnect_delay: Duration,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            keepalive_interval: Duration::from_secs(60),
            keepalive_timeout: Duration::from_secs(30),
            max_failed_keepalives: 3,
            degraded_threshold: Duration::from_secs(120),
            unhealthy_threshold: Duration::from_secs(300),
            health_check_interval: Duration::from_secs(30),
            auto_reconnect: true,
            reconnect_delay: Duration::from_secs(60),
        }
    }
}

/// Connection monitoring events
#[derive(Debug, Clone)]
pub enum MonitorEvent {
    /// Keepalive sent
    KeepAliveSent {
        connection_id: ConnectionId,
        cookie: u16,
    },
    /// Keepalive response received
    KeepAliveReceived {
        connection_id: ConnectionId,
        cookie: u16,
        rtt: Duration,
    },
    /// Keepalive timeout
    KeepAliveTimeout {
        connection_id: ConnectionId,
        cookie: u16,
    },
    /// Health status changed
    HealthChanged {
        connection_id: ConnectionId,
        old_status: HealthStatus,
        new_status: HealthStatus,
    },
    /// Connection marked for closure
    MarkedForClosure {
        connection_id: ConnectionId,
        reason: String,
    },
}

/// Connection health monitor
pub struct ConnectionMonitor {
    /// Monitor configuration
    config: MonitorConfig,
    /// Connection metrics
    metrics: Arc<RwLock<HashMap<ConnectionId, ConnectionMetrics>>>,
    /// Pending keepalives (connection_id -> (cookie, sent_at))
    pending_keepalives: Arc<RwLock<HashMap<ConnectionId, HashMap<u16, Instant>>>>,
    /// Cookie counter
    cookie_counter: Arc<Mutex<u16>>,
    /// Event channel
    event_tx: mpsc::UnboundedSender<MonitorEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<MonitorEvent>>,
    /// Background tasks
    tasks: Vec<JoinHandle<()>>,
}

impl ConnectionMonitor {
    /// Create new connection monitor
    pub async fn new(config: MonitorConfig) -> Result<Self> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Ok(Self {
            config,
            metrics: Arc::new(RwLock::new(HashMap::new())),
            pending_keepalives: Arc::new(RwLock::new(HashMap::new())),
            cookie_counter: Arc::new(Mutex::new(1)),
            event_tx,
            event_rx: Some(event_rx),
            tasks: Vec::new(),
        })
    }

    /// Start monitoring for a connection
    pub async fn start_monitoring(&self, connection_id: ConnectionId, peer_id: PeerId) {
        let metrics = ConnectionMetrics::new(connection_id, peer_id);

        let mut metrics_map = self.metrics.write().await;
        metrics_map.insert(connection_id, metrics);

        let mut pending_map = self.pending_keepalives.write().await;
        pending_map.insert(connection_id, HashMap::new());
    }

    /// Stop monitoring for a connection
    pub async fn stop_monitoring(&self, connection_id: ConnectionId) {
        let mut metrics_map = self.metrics.write().await;
        metrics_map.remove(&connection_id);

        let mut pending_map = self.pending_keepalives.write().await;
        pending_map.remove(&connection_id);
    }

    /// Send keepalive to connection
    pub async fn send_keepalive(&self, connection_id: ConnectionId) -> Option<KeepAliveMessage> {
        let cookie = {
            let mut counter = self.cookie_counter.lock().await;
            let current = *counter;
            *counter = counter.wrapping_add(1);
            current
        };

        let message = KeepAliveMessage::new(cookie);
        let sent_at = Instant::now();

        // Record pending keepalive
        {
            let mut pending_map = self.pending_keepalives.write().await;
            if let Some(connection_pending) = pending_map.get_mut(&connection_id) {
                connection_pending.insert(cookie, sent_at);
            }
        }

        // Update metrics
        {
            let mut metrics_map = self.metrics.write().await;
            if let Some(metrics) = metrics_map.get_mut(&connection_id) {
                metrics.record_keepalive_sent();
            }
        }

        // Emit event
        let _ = self.event_tx.send(MonitorEvent::KeepAliveSent {
            connection_id,
            cookie,
        });

        Some(message)
    }

    /// Handle keepalive response
    pub async fn handle_keepalive_response(
        &self,
        connection_id: ConnectionId,
        response: KeepAliveResponse,
    ) {
        let sent_at = {
            let mut pending_map = self.pending_keepalives.write().await;
            if let Some(connection_pending) = pending_map.get_mut(&connection_id) {
                connection_pending.remove(&response.cookie)
            } else {
                None
            }
        };

        if let Some(sent_at) = sent_at {
            let rtt = Instant::now().duration_since(sent_at);

            // Update metrics
            {
                let mut metrics_map = self.metrics.write().await;
                if let Some(metrics) = metrics_map.get_mut(&connection_id) {
                    metrics.record_keepalive_response(sent_at);
                }
            }

            // Emit event
            let _ = self.event_tx.send(MonitorEvent::KeepAliveReceived {
                connection_id,
                cookie: response.cookie,
                rtt,
            });
        }
    }

    /// Update connection activity
    pub async fn update_activity(&self, connection_id: ConnectionId) {
        let mut metrics_map = self.metrics.write().await;
        if let Some(metrics) = metrics_map.get_mut(&connection_id) {
            metrics.update_activity();
        }
    }

    /// Record protocol error
    pub async fn record_error(&self, connection_id: ConnectionId) {
        let mut metrics_map = self.metrics.write().await;
        if let Some(metrics) = metrics_map.get_mut(&connection_id) {
            metrics.record_protocol_error();
        }
    }

    /// Get connection health status
    pub async fn get_health(&self, connection_id: ConnectionId) -> Option<HealthStatus> {
        let metrics_map = self.metrics.read().await;
        metrics_map.get(&connection_id).map(|m| m.health)
    }

    /// Get all connection metrics
    pub async fn get_all_metrics(&self) -> HashMap<ConnectionId, ConnectionMetrics> {
        self.metrics.read().await.clone()
    }

    /// Perform health check on all connections
    pub async fn check_connection_health(&self) {
        let mut metrics_map = self.metrics.write().await;
        let config = &self.config;

        for (connection_id, metrics) in metrics_map.iter_mut() {
            let old_health = metrics.health;
            metrics.update_health(config);
            let new_health = metrics.health;

            // Emit health change event if status changed
            if old_health != new_health {
                let _ = self.event_tx.send(MonitorEvent::HealthChanged {
                    connection_id: *connection_id,
                    old_status: old_health,
                    new_status: new_health,
                });

                // Mark unhealthy connections for closure
                if new_health == HealthStatus::Unhealthy {
                    let _ = self.event_tx.send(MonitorEvent::MarkedForClosure {
                        connection_id: *connection_id,
                        reason: "Connection unhealthy".to_string(),
                    });
                }
            }
        }

        // Check for keepalive timeouts
        self.check_keepalive_timeouts().await;
    }

    /// Check for keepalive timeouts
    async fn check_keepalive_timeouts(&self) {
        let now = Instant::now();
        let timeout_threshold = self.config.keepalive_timeout;

        let mut pending_map = self.pending_keepalives.write().await;
        let mut metrics_map = self.metrics.write().await;

        for (connection_id, connection_pending) in pending_map.iter_mut() {
            let mut timed_out_cookies = Vec::new();

            // Find timed out keepalives
            for (cookie, sent_at) in connection_pending.iter() {
                if now.duration_since(*sent_at) > timeout_threshold {
                    timed_out_cookies.push(*cookie);
                }
            }

            // Remove timed out keepalives and update metrics
            for cookie in timed_out_cookies {
                connection_pending.remove(&cookie);

                if let Some(metrics) = metrics_map.get_mut(connection_id) {
                    metrics.record_keepalive_failure();
                }

                let _ = self.event_tx.send(MonitorEvent::KeepAliveTimeout {
                    connection_id: *connection_id,
                    cookie,
                });
            }
        }
    }

    /// Get event receiver
    pub fn event_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<MonitorEvent>> {
        self.event_rx.take()
    }

    /// Start background monitoring tasks
    pub async fn start_background_monitoring(&mut self) {
        // TODO: Implement background tasks for periodic health checks and keepalive sending
        // This would require integration with the connection manager
    }

    /// Shutdown monitor
    pub async fn shutdown(&mut self) {
        for task in self.tasks.drain(..) {
            task.abort();
        }

        self.metrics.write().await.clear();
        self.pending_keepalives.write().await.clear();
    }
}

/// Keepalive protocol handler
pub struct KeepAliveProtocol {
    /// Monitor reference
    monitor: Arc<ConnectionMonitor>,
}

impl KeepAliveProtocol {
    /// Create new keepalive protocol handler
    pub fn new(monitor: Arc<ConnectionMonitor>) -> Self {
        Self { monitor }
    }

    /// Handle incoming keepalive message
    pub async fn handle_keepalive(
        &self,
        connection_id: ConnectionId,
        message: KeepAliveMessage,
    ) -> KeepAliveResponse {
        // Update activity
        self.monitor.update_activity(connection_id).await;

        // Create response
        KeepAliveResponse::new(message.cookie)
    }

    /// Handle keepalive response
    pub async fn handle_response(
        &self,
        connection_id: ConnectionId,
        response: KeepAliveResponse,
    ) {
        self.monitor.handle_keepalive_response(connection_id, response).await;
        self.monitor.update_activity(connection_id).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diffusion::PeerId;

    #[test]
    fn test_health_status_checks() {
        assert!(HealthStatus::Healthy.is_acceptable());
        assert!(HealthStatus::Degraded.is_acceptable());
        assert!(!HealthStatus::Unhealthy.is_acceptable());
        assert!(!HealthStatus::Unknown.is_acceptable());

        assert!(!HealthStatus::Healthy.needs_attention());
        assert!(HealthStatus::Degraded.needs_attention());
        assert!(HealthStatus::Unhealthy.needs_attention());
    }

    #[test]
    fn test_connection_metrics() {
        let connection_id = ConnectionId::new();
        let peer_id = PeerId::new([1u8; 32]);
        let mut metrics = ConnectionMetrics::new(connection_id, peer_id);

        // Test initial state
        assert_eq!(metrics.failed_keepalives, 0);
        assert_eq!(metrics.protocol_errors, 0);
        assert_eq!(metrics.health, HealthStatus::Healthy);

        // Test error recording
        metrics.record_protocol_error();
        assert_eq!(metrics.protocol_errors, 1);

        // Test keepalive failure
        metrics.record_keepalive_failure();
        assert_eq!(metrics.failed_keepalives, 1);
    }

    #[tokio::test]
    async fn test_monitor_creation() {
        let config = MonitorConfig::default();
        let monitor = ConnectionMonitor::new(config).await.unwrap();

        let all_metrics = monitor.get_all_metrics().await;
        assert!(all_metrics.is_empty());
    }

    #[tokio::test]
    async fn test_connection_monitoring() {
        let config = MonitorConfig::default();
        let monitor = ConnectionMonitor::new(config).await.unwrap();

        let connection_id = ConnectionId::new();
        let peer_id = PeerId::new([1u8; 32]);

        // Start monitoring
        monitor.start_monitoring(connection_id, peer_id).await;

        // Check metrics exist
        let health = monitor.get_health(connection_id).await;
        assert_eq!(health, Some(HealthStatus::Healthy));

        // Stop monitoring
        monitor.stop_monitoring(connection_id).await;

        // Check metrics removed
        let health = monitor.get_health(connection_id).await;
        assert_eq!(health, None);
    }

    #[test]
    fn test_keepalive_messages() {
        let message = KeepAliveMessage::new(42);
        assert_eq!(message.cookie, 42);

        let response = KeepAliveResponse::new(42);
        assert_eq!(response.cookie, 42);
    }
}
