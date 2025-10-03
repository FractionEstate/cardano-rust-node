//! Connection Manager
//!
//! The connection manager orchestrates peer connections, integrates with the peer selection
//! system, and manages the lifecycle of network connections for the Cardano node.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use blake2::{Blake2b512, Digest};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{interval, timeout};
use tracing::{error, warn};

use super::handshake::HandshakeProtocol;
use super::monitor::{ConnectionMonitor, MonitorConfig};
use super::multiplexer::{ConnectionMultiplexer, MultiplexerConfig, ProtocolHandler};
use super::state::ConnectionStateMachine;
use super::state::{ConnectionState, TransitionReason};
use super::{
    Connection, ConnectionError, ConnectionEvent, ConnectionId, ConnectionLimits,
    ConnectionRegistry, ProtocolId,
};
use crate::diffusion::{
    ConnectionState as PeerConnectionState, PeerId, PeerInfo, PeerSelector, SelectionConfig,
};
use crate::protocols::chainsync::ChainSyncProtocolHandler;
use crate::{NetworkError, Result};

/// Connection management configuration
#[derive(Debug, Clone, Default)]
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
    PeerSelectionCompleted { candidates: usize, selected: usize },
    /// Connection limit reached
    ConnectionLimitReached { current: usize, limit: usize },
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
    connections: ConnectionRegistry,
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

        let mut manager = Self {
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
        };

        // Register default ChainSync protocol handler
        let chainsync_handler = Arc::new(ChainSyncProtocolHandler::with_mock_chain(32));
        manager.register_protocol(chainsync_handler).await?;

        Ok(manager)
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
            return Err(NetworkError::ProtocolError(format!(
                "Protocol {} already registered",
                protocol_id
            )));
        }

        handlers.insert(protocol_id, handler);
        Ok(())
    }

    /// Register a known peer with the selector
    pub async fn add_peer(&self, peer: PeerInfo) -> Result<()> {
        let mut selector = self.peer_selector.lock().await;
        selector
            .add_peer(peer)
            .map_err(|err| NetworkError::PeerSelectionError(err.to_string()))
    }

    /// Attempt to connect to a known peer by identifier
    pub async fn connect_peer(&self, peer_id: &PeerId) -> Result<ConnectionId> {
        let peer_info = {
            let selector = self.peer_selector.lock().await;
            selector.get_peer(peer_id).cloned().ok_or_else(|| {
                NetworkError::PeerSelectionError(format!("Peer {} not registered", peer_id))
            })
        }?;

        self.clone_for_task().initiate_connection(peer_info).await
    }

    /// Connect to a specific peer
    pub async fn connect_to_peer(&self, address: SocketAddr) -> Result<ConnectionId> {
        // Check connection limits
        let current_connections = self.connections.read().await.len();
        if current_connections >= self.config.limits.max_connections {
            self.emit_event(ManagementEvent::ConnectionLimitReached {
                current: current_connections,
                limit: self.config.limits.max_connections,
            })
            .await;
            return Err(NetworkError::ConnectionError(
                ConnectionError::ProtocolViolation("Maximum connections reached".to_string()),
            ));
        }

        // Derive a deterministic peer ID from the address so repeated calls reuse the same entry
        let mut hasher = Blake2b512::new();
        hasher.update(address.to_string().as_bytes());
        let digest = hasher.finalize();
        let mut peer_id_bytes = [0u8; 32];
        peer_id_bytes.copy_from_slice(&digest[..32]);
        let peer_id = PeerId::new(peer_id_bytes);
        let peer_info = PeerInfo::new(peer_id, address);

        {
            let mut selector = self.peer_selector.lock().await;
            if let Err(err) = selector.add_peer(peer_info.clone()) {
                if !matches!(
                    err,
                    crate::diffusion::PeerSelectionError::PeerAlreadyExists(_)
                ) {
                    return Err(NetworkError::PeerSelectionError(err.to_string()));
                }
            }
        }

        self.clone_for_task().initiate_connection(peer_info).await
    }

    /// Disconnect from a peer
    pub async fn disconnect_peer(&self, peer_id: &PeerId) -> Result<()> {
        let connection_id = {
            let peer_connections = self.peer_connections.read().await;
            peer_connections.get(peer_id).copied()
        };

        if let Some(conn_id) = connection_id {
            self.close_connection(conn_id, TransitionReason::UserDisconnect)
                .await?;
        }

        Ok(())
    }

    /// Get connection statistics
    pub async fn connection_stats(&self) -> HashMap<ConnectionId, super::ConnectionStats> {
        let connections = self.connections.read().await;
        let mut stats = HashMap::new();

        for (id, connection) in connections.iter() {
            let mut connection_stats = super::ConnectionStats::default();

            if connection.is_active() {
                connection_stats.connect_time = Some(connection.uptime());
            }

            stats.insert(*id, connection_stats);
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
            let _ = self
                .close_connection(conn_id, TransitionReason::UserDisconnect)
                .await;
        }
    }

    /// Close a connection
    async fn close_connection(
        &self,
        connection_id: ConnectionId,
        reason: TransitionReason,
    ) -> Result<()> {
        self.clone_for_task()
            .close_connection(connection_id, reason)
            .await
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
            peer_selector: self.peer_selector.clone(),
            event_tx: self.event_tx.clone(),
            protocol_handlers: self.protocol_handlers.clone(),
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
    peer_selector: Arc<Mutex<PeerSelector>>,
    event_tx: mpsc::UnboundedSender<ManagementEvent>,
    protocol_handlers: Arc<RwLock<HashMap<ProtocolId, Arc<dyn ProtocolHandler>>>>,
    config: ConnectionConfig,
}

impl ConnectionManagerHandle {
    async fn initiate_connection(&self, peer_info: PeerInfo) -> Result<ConnectionId> {
        let connection_id = ConnectionId::new();
        let peer_id = peer_info.peer_id;

        let connection = Connection::new(peer_info.clone(), peer_info.address);
        let mut state_machine = ConnectionStateMachine::new(connection_id, peer_id);

        state_machine.transition(ConnectionState::Connecting, TransitionReason::UserInitiated)?;

        {
            let mut connections = self.connections.write().await;
            let mut state_machines = self.state_machines.write().await;
            let mut peer_connections = self.peer_connections.write().await;

            connections.insert(connection_id, connection);
            state_machines.insert(connection_id, state_machine);
            peer_connections.insert(peer_id, connection_id);
        }

        {
            let mut selector = self.peer_selector.lock().await;
            if let Some(peer) = selector.get_peer_mut(&peer_id) {
                peer.set_connection_state(PeerConnectionState::Connecting);
            }
        }

        self.emit_connection_event(ConnectionEvent::Connecting {
            connection_id,
            peer_id,
            address: peer_info.address,
        })
        .await;

        let handle = self.clone();
        tokio::spawn(async move {
            if let Err(e) = handle
                .establish_connection(connection_id, peer_info.clone())
                .await
            {
                warn!("Connection establishment failed: {}", e);
                let connection_error = match e {
                    NetworkError::ConnectionError(ce) => ce,
                    _ => ConnectionError::ProtocolViolation(e.to_string()),
                };
                if let Err(err) = handle
                    .handle_connection_error(connection_id, connection_error)
                    .await
                {
                    error!("Connection error handling failed: {}", err);
                }
            }
        });

        Ok(connection_id)
    }

    async fn establish_connection(
        &self,
        connection_id: ConnectionId,
        peer_info: PeerInfo,
    ) -> Result<()> {
        let connect_timeout = self.config.limits.connect_timeout;
        let handshake_timeout = self.config.limits.handshake_timeout;

        let mut stream = timeout(connect_timeout, TcpStream::connect(peer_info.address))
            .await
            .map_err(|_| ConnectionError::Timeout)?
            .map_err(ConnectionError::from)?;

        {
            let mut state_machines = self.state_machines.write().await;
            if let Some(state_machine) = state_machines.get_mut(&connection_id) {
                state_machine
                    .transition(ConnectionState::Connected, TransitionReason::TcpEstablished)?;
            }
        }

        self.emit_connection_event(ConnectionEvent::Connected {
            connection_id,
            peer_id: peer_info.peer_id,
        })
        .await;

        let handshake = HandshakeProtocol::new();
        let protocol_version = timeout(handshake_timeout, handshake.perform_handshake(&mut stream))
            .await
            .map_err(|_| ConnectionError::Timeout)?
            .map_err(ConnectionError::HandshakeError)?;

        let mut multiplexer =
            ConnectionMultiplexer::new(connection_id, stream, self.config.multiplexer.clone())?;

        let handlers = {
            let guard = self.protocol_handlers.read().await;
            guard.values().cloned().collect::<Vec<_>>()
        };

        for handler in handlers {
            multiplexer.register_protocol(handler).await?;
        }

        let multiplexer = Arc::new(multiplexer);

        {
            let mut connections = self.connections.write().await;
            if let Some(connection) = connections.get_mut(&connection_id) {
                connection.multiplexer = Some(multiplexer);
                connection.state = ConnectionState::Authenticated;
            }
        }

        {
            let mut state_machines = self.state_machines.write().await;
            if let Some(state_machine) = state_machines.get_mut(&connection_id) {
                state_machine.transition(
                    ConnectionState::Authenticated,
                    TransitionReason::HandshakeComplete,
                )?;
            }
        }

        self.emit_connection_event(ConnectionEvent::Authenticated {
            connection_id,
            peer_id: peer_info.peer_id,
            protocol_version,
        })
        .await;

        {
            let mut selector = self.peer_selector.lock().await;
            if let Err(err) = selector.handle_connection_success(&peer_info.peer_id) {
                warn!("Failed to update peer selection after connection: {}", err);
            }
        }

        Ok(())
    }

    async fn handle_connection_error(
        &self,
        connection_id: ConnectionId,
        error: ConnectionError,
    ) -> Result<()> {
        let peer_id = {
            let connections = self.connections.read().await;
            connections
                .get(&connection_id)
                .map(|conn| conn.peer.peer_id)
        };

        if let Some(peer_id) = peer_id {
            self.emit_connection_event(ConnectionEvent::Error {
                connection_id,
                peer_id,
                error: error.clone(),
            })
            .await;

            {
                let mut state_machines = self.state_machines.write().await;
                if let Some(state_machine) = state_machines.get_mut(&connection_id) {
                    state_machine.fail(TransitionReason::NetworkError, error.to_string());
                }
            }

            {
                let mut selector = self.peer_selector.lock().await;
                if let Err(err) = selector.handle_connection_failure(&peer_id, error.to_string()) {
                    warn!("Failed to record connection failure: {}", err);
                }
            }

            if self.config.reconnect.enabled {
                self.schedule_reconnection(peer_id).await;
            }
        }

        self.close_connection(connection_id, TransitionReason::NetworkError)
            .await
    }

    async fn perform_maintenance(&self) {
        let current_count = self.connection_count().await;
        let target_count = self.config.peer_selection.target_connections;

        self.emit_event(ManagementEvent::PoolSizeChanged {
            active_connections: current_count,
            target_connections: target_count,
        })
        .await;
    }

    async fn perform_peer_selection(&self) {
        let current_count = self.connection_count().await;
        let target_count = self.config.peer_selection.target_connections;

        if current_count >= target_count {
            return;
        }

        let needed = target_count - current_count;

        let (candidates, peer_infos) = {
            let mut selector = self.peer_selector.lock().await;
            let candidate_ids = selector.select_connection_candidates(needed);
            let infos = candidate_ids
                .iter()
                .filter_map(|peer_id| selector.get_peer(peer_id).cloned())
                .collect::<Vec<_>>();
            (candidate_ids, infos)
        };

        self.emit_event(ManagementEvent::PeerSelectionCompleted {
            candidates: candidates.len(),
            selected: needed.min(candidates.len()),
        })
        .await;

        for peer_info in peer_infos.into_iter().take(needed) {
            if let Err(e) = self.initiate_connection(peer_info).await {
                warn!("Failed to initiate connection: {}", e);
            }
        }
    }

    async fn close_connection(
        &self,
        connection_id: ConnectionId,
        reason: TransitionReason,
    ) -> Result<()> {
        let peer_id = {
            let mut connections = self.connections.write().await;
            let mut state_machines = self.state_machines.write().await;
            let mut peer_connections = self.peer_connections.write().await;

            let peer_id = connections
                .get(&connection_id)
                .map(|conn| conn.peer.peer_id);

            if let Some(state_machine) = state_machines.get_mut(&connection_id) {
                let _ = state_machine.transition(ConnectionState::Closing, reason.clone());
            }

            connections.remove(&connection_id);
            state_machines.remove(&connection_id);

            if let Some(peer_id) = peer_id {
                peer_connections.remove(&peer_id);
            }

            peer_id
        };

        if let Some(peer_id) = peer_id {
            self.emit_connection_event(ConnectionEvent::Disconnected {
                connection_id,
                peer_id,
                reason: reason.to_string(),
            })
            .await;

            let mut selector = self.peer_selector.lock().await;
            selector.handle_disconnection(&peer_id);
        }

        Ok(())
    }

    async fn schedule_reconnection(&self, peer_id: PeerId) {
        self.emit_event(ManagementEvent::ReconnectionStarted {
            peer_id,
            attempt: 1,
            delay: self.config.reconnect.initial_delay,
        })
        .await;
    }

    async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }

    async fn emit_connection_event(&self, event: ConnectionEvent) {
        println!("Connection event: {:?}", event);
    }

    async fn emit_event(&self, event: ManagementEvent) {
        let _ = self.event_tx.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blake2::{Blake2b512, Digest};
    use bytes::Bytes;
    use std::pin::Pin;
    use std::sync::Arc;
    use tokio::net::TcpListener;

    struct DummyProtocolHandler {
        id: ProtocolId,
        name: &'static str,
    }

    impl DummyProtocolHandler {
        fn new(id: ProtocolId, name: &'static str) -> Self {
            Self { id, name }
        }
    }

    impl ProtocolHandler for DummyProtocolHandler {
        fn handle_message(
            &self,
            _connection_id: ConnectionId,
            _message: Bytes,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<Option<Bytes>>> + Send>> {
            Box::pin(async { Ok(None) })
        }

        fn protocol_id(&self) -> ProtocolId {
            self.id
        }

        fn name(&self) -> &str {
            self.name
        }
    }

    #[tokio::test]
    async fn test_connection_manager_creation() {
        let config = ConnectionConfig::default();
        let manager = ConnectionManager::new(config).await.unwrap();

        assert_eq!(manager.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_connect_peer_success() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                let handshake = HandshakeProtocol::new();
                let mut stream = stream;
                let _ = handshake.handle_handshake(&mut stream).await;
            }
        });

        let mut config = ConnectionConfig::default();
        config.limits.connect_timeout = Duration::from_secs(2);
        config.limits.handshake_timeout = Duration::from_secs(2);
        config.peer_selection.target_connections = 1;
        config.peer_selection.max_connections = 1;

        let mut manager = ConnectionManager::new(config).await.unwrap();
        let mut hasher = Blake2b512::new();
        hasher.update(addr.to_string().as_bytes());
        let digest = hasher.finalize();
        let mut peer_id_bytes = [0u8; 32];
        peer_id_bytes.copy_from_slice(&digest[..32]);
        let peer_id = PeerId::new(peer_id_bytes);
        let peer = PeerInfo::new(peer_id, addr);

        manager.add_peer(peer).await.unwrap();
        manager.start().await.unwrap();

        manager.connect_peer(&peer_id).await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(manager.connection_count().await, 1);

        manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_multiplexer_registers_protocols() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                let handshake = HandshakeProtocol::new();
                let mut stream = stream;
                let _ = handshake.handle_handshake(&mut stream).await;
            }
        });

        let mut config = ConnectionConfig::default();
        config.limits.connect_timeout = Duration::from_secs(2);
        config.limits.handshake_timeout = Duration::from_secs(2);
        config.peer_selection.target_connections = 1;
        config.peer_selection.max_connections = 1;

        let mut manager = ConnectionManager::new(config).await.unwrap();
        let handler = Arc::new(DummyProtocolHandler::new(ProtocolId::GOSSIP, "Dummy"));
        manager.register_protocol(handler).await.unwrap();

        let mut hasher = Blake2b512::new();
        hasher.update(addr.to_string().as_bytes());
        let digest = hasher.finalize();
        let mut peer_id_bytes = [0u8; 32];
        peer_id_bytes.copy_from_slice(&digest[..32]);
        let peer_id = PeerId::new(peer_id_bytes);
        let peer = PeerInfo::new(peer_id, addr);

        manager.add_peer(peer).await.unwrap();
        manager.start().await.unwrap();
        manager.connect_peer(&peer_id).await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        let multiplexer = {
            let connections = manager.connections.read().await;
            let connection = connections.values().next().unwrap();
            connection.multiplexer.clone()
        };

        assert!(multiplexer.is_some());

        let registered = multiplexer.unwrap().registered_protocols().await;

        assert!(registered.contains(&ProtocolId::CHAINSYNC));
        assert!(registered.contains(&ProtocolId::GOSSIP));

        manager.shutdown().await;
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
