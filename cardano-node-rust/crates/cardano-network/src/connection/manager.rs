//! Connection Manager
//!
//! The connection manager orchestrates peer connections, integrates with the peer selection
//! system, and manages the lifecycle of network connections for the Cardano node.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::task::JoinHandle;
use tokio::time::{interval, timeout, sleep};

use super::{
    Connection, ConnectionEvent, ConnectionId, ConnectionLimits, ConnectionError, ProtocolId,
};
use super::multiplexer::{ConnectionMultiplexer, MultiplexerConfig, ProtocolHandler};
use super::handshake::HandshakeProtocol;
use super::state::ConnectionStateMachine;
use super::state::{ConnectionState, TransitionReason};
use super::monitor::{ConnectionMonitor, MonitorConfig};
use crate::diffusion::{PeerId, PeerInfo, PeerSelector, SelectionConfig};
use crate::{NetworkError, Result};

/// Connection management configuration
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Connection limits and timeouts
    pub limits: ConnectionLimits,
    /// Multiplexer configuration
    pub multiplexer: MultiplexerConfig,
    /// Monitoring configuration
    pub monitor: MonitorConfig,
    /// Peer selection configuration
    pub peer_selection: SelectionConfig,
    /// Automatic reconnection settings
    pub reconnect: ReconnectConfig,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            limits: ConnectionLimits::default(),
            multiplexer: MultiplexerConfig::default(),
            monitor: MonitorConfig::default(),
            peer_selection: SelectionConfig::default(),
            reconnect: ReconnectConfig::default(),
        }
    }
}

/// Automatic reconnection configuration
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Enable automatic reconnection
    pub enabled: bool,
    /// Initial retry delay
    pub initial_delay: Duration,
    /// Maximum retry delay
    pub max_delay: Duration,
    /// Backoff multiplier
    pub backoff_factor: f64,
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Reset attempts after successful connection duration
    pub reset_after: Duration,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(300), // 5 minutes
            backoff_factor: 2.0,
            max_attempts: 10,
            reset_after: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Connection management events for external monitoring
#[derive(Debug, Clone)]
pub enum ManagementEvent {
    /// Connection pool size changed
    PoolSizeChanged {
        active_connections: usize,
        target_connections: usize,
    },
    /// Peer selection completed
    PeerSelectionCompleted {
        candidates: usize,
        selected: usize,
    },
    /// Connection limit reached
    ConnectionLimitReached {
        current: usize,
        limit: usize,
    },
    /// Reconnection attempt started
    ReconnectionStarted {
        peer_id: PeerId,
        attempt: u32,
        delay: Duration,
    },
    /// Peer blacklisted
    PeerBlacklisted {
        peer_id: PeerId,
        reason: String,
        duration: Duration,
    },
}

/// Main connection manager
pub struct ConnectionManager {
    /// Manager configuration
    config: ConnectionConfig,
    /// Active connections registry
    connections: Arc<RwLock<HashMap<ConnectionId, Connection>>>,
    /// Connection state machines
    state_machines: Arc<RwLock<HashMap<ConnectionId, ConnectionStateMachine>>>,
    /// Peer to connection mapping
    peer_connections: Arc<RwLock<HashMap<PeerId, ConnectionId>>>,
    /// Registered protocol handlers
    protocol_handlers: Arc<RwLock<HashMap<ProtocolId, Arc<dyn ProtocolHandler>>>>,
    /// Peer selector for connection decisions
    peer_selector: Arc<Mutex<PeerSelector>>,
    /// Connection monitor
    monitor: Arc<ConnectionMonitor>,
    /// Event channel for external monitoring
    event_tx: mpsc::UnboundedSender<ManagementEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<ManagementEvent>>,
    /// Background tasks
    tasks: Vec<JoinHandle<()>>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl ConnectionManager {
    /// Create new connection manager
    pub async fn new(config: ConnectionConfig) -> Result<Self> {
        let peer_selector = PeerSelector::new(config.peer_selection.clone());
        let monitor = ConnectionMonitor::new(config.monitor.clone()).await?;
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Ok(Self {
            config,
            connections: Arc::new(RwLock::new(HashMap::new())),
            state_machines: Arc::new(RwLock::new(HashMap::new())),
            peer_connections: Arc::new(RwLock::new(HashMap::new())),
            protocol_handlers: Arc::new(RwLock::new(HashMap::new())),
            peer_selector: Arc::new(Mutex::new(peer_selector)),
            monitor: Arc::new(monitor),
            event_tx,
            event_rx: Some(event_rx),
            tasks: Vec::new(),
            shutdown_tx: None,
        })
    }

    /// Start the connection manager
    pub async fn start(&mut self) -> Result<()> {
        let (shutdown_tx, shutdown_rx1) = mpsc::channel(1);
        let (_, shutdown_rx2) = mpsc::channel(1);
        let (_, shutdown_rx3) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        // Start connection management task
        let management_task = self.spawn_management_task(shutdown_rx1).await?;
        self.tasks.push(management_task);

        // Start peer selection task
        let selection_task = self.spawn_peer_selection_task(shutdown_rx2).await?;
        self.tasks.push(selection_task);

        // Start monitoring task
        let monitor_task = self.spawn_monitoring_task(shutdown_rx3).await?;
        self.tasks.push(monitor_task);

        Ok(())
    }

    /// Register a protocol handler
    pub async fn register_protocol(&mut self, handler: Arc<dyn ProtocolHandler>) -> Result<()> {
        let protocol_id = handler.protocol_id();
        let mut handlers = self.protocol_handlers.write().await;

        if handlers.contains_key(&protocol_id) {
            return Err(NetworkError::ProtocolError(
                format!("Protocol {} already registered", protocol_id)
            ));
        }

        handlers.insert(protocol_id, handler);
        Ok(())
    }

    /// Connect to a specific peer
    pub async fn connect_to_peer(&self, address: SocketAddr) -> Result<ConnectionId> {
        // Check connection limits
        let current_connections = self.connections.read().await.len();
        if current_connections >= self.config.limits.max_connections {
            self.emit_event(ManagementEvent::ConnectionLimitReached {
                current: current_connections,
                limit: self.config.limits.max_connections,
            }).await;
            return Err(NetworkError::ConnectionError(
                ConnectionError::ProtocolViolation("Maximum connections reached".to_string())
            ));
        }

        // Create peer info (in real implementation, this would come from peer discovery)
        let peer_id = PeerId::new([0u8; 32]); // Placeholder
        let peer_info = PeerInfo::new(peer_id, address);

        // Check if already connected to this peer
        if let Some(_existing_id) = self.peer_connections.read().await.get(&peer_id) {
            return Err(NetworkError::ConnectionError(
                ConnectionError::ProtocolViolation("Already connected to this peer".to_string())
            ));
        }

        self.initiate_connection(peer_info).await
    }

    /// Disconnect from a peer
    pub async fn disconnect_peer(&self, peer_id: &PeerId) -> Result<()> {
        let connection_id = {
            let peer_connections = self.peer_connections.read().await;
            peer_connections.get(peer_id).copied()
        };

        if let Some(conn_id) = connection_id {
            self.close_connection(conn_id, TransitionReason::UserDisconnect).await?;
        }

        Ok(())
    }

    /// Get connection statistics
    pub async fn connection_stats(&self) -> HashMap<ConnectionId, super::ConnectionStats> {
        let connections = self.connections.read().await;
        let mut stats = HashMap::new();

        for (id, connection) in connections.iter() {
            if let Some(multiplexer) = &connection.multiplexer {
                // In a full implementation, we would extract stats from the multiplexer
                stats.insert(*id, super::ConnectionStats::default());
            }
        }

        stats
    }

    /// Get current connection count
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Get event receiver for external monitoring
    pub fn event_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<ManagementEvent>> {
        self.event_rx.take()
    }

    /// Shutdown the connection manager
    pub async fn shutdown(&mut self) {
        // Signal shutdown to all tasks
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()).await;
        }

        // Wait for tasks to complete
        for task in self.tasks.drain(..) {
            task.abort();
        }

        // Close all connections
        let connection_ids: Vec<ConnectionId> = {
            let connections = self.connections.read().await;
            connections.keys().copied().collect()
        };

        for conn_id in connection_ids {
            let _ = self.close_connection(conn_id, TransitionReason::UserDisconnect).await;
        }
    }

    /// Initiate connection to peer
    async fn initiate_connection(&self, peer_info: PeerInfo) -> Result<ConnectionId> {
        let connection_id = ConnectionId::new();
        let peer_id = peer_info.peer_id;

        // Create connection and state machine
        let connection = Connection::new(peer_info.clone(), peer_info.address);
        let mut state_machine = ConnectionStateMachine::new(connection_id, peer_id);

        // Transition to connecting state
        state_machine.transition(
            ConnectionState::Connecting,
            TransitionReason::UserInitiated,
        ).map_err(ConnectionError::from)?;

        // Store connection and state machine
        {
            let mut connections = self.connections.write().await;
            let mut state_machines = self.state_machines.write().await;
            let mut peer_connections = self.peer_connections.write().await;

            connections.insert(connection_id, connection);
            state_machines.insert(connection_id, state_machine);
            peer_connections.insert(peer_id, connection_id);
        }

        // Emit connection event
        self.emit_connection_event(ConnectionEvent::Connecting {
            connection_id,
            peer_id,
            address: peer_info.address,
        }).await;

        // Start connection attempt in background
        let manager = self.clone_for_task();
        tokio::spawn(async move {
            if let Err(e) = manager.establish_connection(connection_id, peer_info).await {
                eprintln!("Connection establishment failed: {}", e);
                let connection_error = match e {
                    NetworkError::ConnectionError(ce) => ce,
                    _ => ConnectionError::ProtocolViolation(e.to_string()),
                };
                let _ = manager.handle_connection_error(connection_id, connection_error).await;
            }
        });

        Ok(connection_id)
    }

    /// Establish TCP connection and handshake
    async fn establish_connection(&self, connection_id: ConnectionId, peer_info: PeerInfo) -> Result<()> {
        let connect_timeout = self.config.limits.connect_timeout;
        let handshake_timeout = self.config.limits.handshake_timeout;

        // Establish TCP connection
        let stream = timeout(connect_timeout, TcpStream::connect(peer_info.address))
            .await
            .map_err(|_| ConnectionError::Timeout)?
            .map_err(ConnectionError::from)?;

        // Update state to connected
        {
            let mut state_machines = self.state_machines.write().await;
            if let Some(state_machine) = state_machines.get_mut(&connection_id) {
                state_machine.transition(
                    ConnectionState::Connected,
                    TransitionReason::TcpEstablished,
                )?;
            }
        }

        // Emit connected event
        self.emit_connection_event(ConnectionEvent::Connected {
            connection_id,
            peer_id: peer_info.peer_id,
        }).await;

        // Perform handshake
        let handshake = HandshakeProtocol::new();
        let protocol_version = timeout(handshake_timeout, handshake.perform_handshake(stream))
            .await
            .map_err(|_| ConnectionError::Timeout)?
            .map_err(ConnectionError::HandshakeError)?;

        // For now, skip creating multiplexer due to stream ownership issue
        // In a full implementation, handshake would return the stream
        let multiplexer = None;

        // Register protocol handlers with multiplexer
        // (In a full implementation, this would be done properly)

        // Update connection with multiplexer
        {
            let mut connections = self.connections.write().await;
            if let Some(connection) = connections.get_mut(&connection_id) {
                connection.multiplexer = multiplexer;
                connection.state = ConnectionState::Authenticated;
            }
        }

        // Update state to authenticated
        {
            let mut state_machines = self.state_machines.write().await;
            if let Some(state_machine) = state_machines.get_mut(&connection_id) {
                state_machine.transition(
                    ConnectionState::Authenticated,
                    TransitionReason::HandshakeComplete,
                )?;
            }
        }

        // Emit authenticated event
        self.emit_connection_event(ConnectionEvent::Authenticated {
            connection_id,
            peer_id: peer_info.peer_id,
            protocol_version,
        }).await;

        Ok(())
    }

    /// Close a connection
    async fn close_connection(
        &self,
        connection_id: ConnectionId,
        reason: TransitionReason,
    ) -> Result<()> {
        let peer_id = {
            let mut connections = self.connections.write().await;
            let mut state_machines = self.state_machines.write().await;
            let mut peer_connections = self.peer_connections.write().await;

            // Get peer ID before removal
            let peer_id = connections.get(&connection_id)
                .map(|conn| conn.peer.peer_id);

            // Update state machine
            if let Some(state_machine) = state_machines.get_mut(&connection_id) {
                let _ = state_machine.transition(ConnectionState::Closing, reason.clone());
            }

            // Remove from collections
            connections.remove(&connection_id);
            state_machines.remove(&connection_id);

            if let Some(peer_id) = peer_id {
                peer_connections.remove(&peer_id);
            }

            peer_id
        };

        // Emit disconnection event
        if let Some(peer_id) = peer_id {
            self.emit_connection_event(ConnectionEvent::Disconnected {
                connection_id,
                peer_id,
                reason: reason.to_string(),
            }).await;
        }

        Ok(())
    }

    /// Handle connection error
    async fn handle_connection_error(
        &self,
        connection_id: ConnectionId,
        error: ConnectionError,
    ) -> Result<()> {
        let peer_id = {
            let connections = self.connections.read().await;
            connections.get(&connection_id)
                .map(|conn| conn.peer.peer_id)
        };

        if let Some(peer_id) = peer_id {
            // Emit error event
            self.emit_connection_event(ConnectionEvent::Error {
                connection_id,
                peer_id,
                error: error.clone(),
            }).await;

            // Update state machine to failed
            {
                let mut state_machines = self.state_machines.write().await;
                if let Some(state_machine) = state_machines.get_mut(&connection_id) {
                    state_machine.fail(
                        TransitionReason::NetworkError,
                        error.to_string(),
                    );
                }
            }

            // Schedule reconnection if enabled
            if self.config.reconnect.enabled {
                self.schedule_reconnection(peer_id).await;
            }
        }

        // Clean up connection
        self.close_connection(connection_id, TransitionReason::NetworkError).await
    }

    /// Schedule automatic reconnection
    async fn schedule_reconnection(&self, peer_id: PeerId) {
        // Implementation would track reconnection attempts and delays
        // For now, just emit event
        self.emit_event(ManagementEvent::ReconnectionStarted {
            peer_id,
            attempt: 1,
            delay: self.config.reconnect.initial_delay,
        }).await;
    }

    /// Spawn connection management task
    async fn spawn_management_task(
        &self,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) -> Result<JoinHandle<()>> {
        let manager = self.clone_for_task();

        let task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        manager.perform_maintenance().await;
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });

        Ok(task)
    }

    /// Spawn peer selection task
    async fn spawn_peer_selection_task(
        &self,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) -> Result<JoinHandle<()>> {
        let manager = self.clone_for_task();

        let task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        manager.perform_peer_selection().await;
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });

        Ok(task)
    }

    /// Spawn monitoring task
    async fn spawn_monitoring_task(
        &self,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) -> Result<JoinHandle<()>> {
        let monitor = self.monitor.clone();

        let task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        monitor.check_connection_health().await;
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });

        Ok(task)
    }

    /// Perform periodic maintenance
    async fn perform_maintenance(&self) {
        // Clean up failed connections, update statistics, etc.
        let current_count = self.connection_count().await;
        let target_count = self.config.peer_selection.target_connections;

        self.emit_event(ManagementEvent::PoolSizeChanged {
            active_connections: current_count,
            target_connections: target_count,
        }).await;
    }

    /// Perform peer selection
    async fn perform_peer_selection(&self) {
        let current_count = self.connection_count().await;
        let target_count = self.config.peer_selection.target_connections;

        if current_count < target_count {
            let needed = target_count - current_count;

            // Get candidates from peer selector
            let candidates = {
                let mut selector = self.peer_selector.lock().await;
                selector.select_connection_candidates(needed)
            };

            self.emit_event(ManagementEvent::PeerSelectionCompleted {
                candidates: candidates.len(),
                selected: needed.min(candidates.len()),
            }).await;

            // Initiate connections to selected candidates
            for peer_id in candidates.into_iter().take(needed) {
                // Convert PeerId to PeerInfo (placeholder address)
                let peer_info = PeerInfo::new(peer_id, "127.0.0.1:3001".parse().unwrap());
                if let Err(e) = self.initiate_connection(peer_info).await {
                    eprintln!("Failed to initiate connection: {}", e);
                }
            }
        }
    }

    /// Emit connection event
    async fn emit_connection_event(&self, event: ConnectionEvent) {
        // In a full implementation, this would forward to interested subscribers
        println!("Connection event: {:?}", event);
    }

    /// Emit management event
    async fn emit_event(&self, event: ManagementEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Create a clone for use in tasks (with reduced state)
    fn clone_for_task(&self) -> ConnectionManagerHandle {
        ConnectionManagerHandle {
            connections: self.connections.clone(),
            state_machines: self.state_machines.clone(),
            peer_connections: self.peer_connections.clone(),
            protocol_handlers: self.protocol_handlers.clone(),
            peer_selector: self.peer_selector.clone(),
            monitor: self.monitor.clone(),
            event_tx: self.event_tx.clone(),
            config: self.config.clone(),
        }
    }
}

/// Lightweight handle for use in tasks
#[derive(Clone)]
struct ConnectionManagerHandle {
    connections: Arc<RwLock<HashMap<ConnectionId, Connection>>>,
    state_machines: Arc<RwLock<HashMap<ConnectionId, ConnectionStateMachine>>>,
    peer_connections: Arc<RwLock<HashMap<PeerId, ConnectionId>>>,
    protocol_handlers: Arc<RwLock<HashMap<ProtocolId, Arc<dyn ProtocolHandler>>>>,
    peer_selector: Arc<Mutex<PeerSelector>>,
    monitor: Arc<ConnectionMonitor>,
    event_tx: mpsc::UnboundedSender<ManagementEvent>,
    config: ConnectionConfig,
}

impl ConnectionManagerHandle {
    async fn establish_connection(&self, connection_id: ConnectionId, peer_info: PeerInfo) -> Result<()> {
        // Implementation similar to ConnectionManager::establish_connection
        // but using the handle's state
        Err(NetworkError::ConnectionError(
            ConnectionError::ProtocolViolation("Not implemented".to_string())
        ))
    }

    async fn handle_connection_error(&self, connection_id: ConnectionId, error: ConnectionError) -> Result<()> {
        // Implementation similar to ConnectionManager::handle_connection_error
        Err(NetworkError::ConnectionError(
            ConnectionError::ProtocolViolation("Not implemented".to_string())
        ))
    }

    async fn perform_maintenance(&self) {
        // Maintenance implementation
    }

    async fn perform_peer_selection(&self) {
        // Peer selection implementation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_manager_creation() {
        let config = ConnectionConfig::default();
        let manager = ConnectionManager::new(config).await.unwrap();

        assert_eq!(manager.connection_count().await, 0);
    }

    #[test]
    fn test_reconnect_config_defaults() {
        let config = ReconnectConfig::default();

        assert!(config.enabled);
        assert_eq!(config.initial_delay, Duration::from_secs(1));
        assert_eq!(config.max_delay, Duration::from_secs(300));
        assert_eq!(config.backoff_factor, 2.0);
    }
}
