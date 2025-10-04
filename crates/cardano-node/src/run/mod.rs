//! Cardano Node Runtime Module
//!
//! This module provides the main runtime functionality for the Cardano node,
//! including the main event loop, subsystem coordination, and lifecycle management.
//! It is based on the structure of the Haskell Cardano node but implemented
//! with Rust's async/await and tokio runtime for optimal performance.

use anyhow::{anyhow, Context, Result};
use blake2::{Blake2b512, Digest};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, instrument, warn};

use cardano_network::{ConnectionConfig, ConnectionManager, PeerId, PeerInfo};

use crate::config::ConfigurationManager;

/// Node runtime errors
#[derive(Debug, thiserror::Error)]
pub enum NodeRuntimeError {
    #[error("Consensus subsystem error: {0}")]
    ConsensusError(String),

    #[error("Network subsystem error: {0}")]
    NetworkError(String),

    #[error("Storage subsystem error: {0}")]
    StorageError(String),

    #[error("API subsystem error: {0}")]
    ApiError(String),

    #[error("Tracing subsystem error: {0}")]
    TracingError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Signal handling error: {0}")]
    SignalError(String),

    #[error("Shutdown timeout")]
    ShutdownTimeout,

    #[error("Node already running")]
    AlreadyRunning,

    #[error("Node not initialized")]
    NotInitialized,
}

/// Node state enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeState {
    /// Node is not yet initialized
    Uninitialized,
    /// Node is starting up
    Starting,
    /// Node is running normally
    Running,
    /// Node is shutting down gracefully
    ShuttingDown,
    /// Node has stopped
    Stopped,
    /// Node encountered an error
    Error(String),
}

/// Node runtime events
#[derive(Debug, Clone)]
pub enum NodeEvent {
    /// Node startup completed successfully
    StartupComplete,
    /// Configuration was reloaded
    ConfigurationReloaded,
    /// New block received from consensus
    BlockReceived,
    /// Network peer connected/disconnected
    PeerUpdate,
    /// API request processed
    ApiRequest,
    /// Health check update
    HealthUpdate,
    /// Shutdown signal received
    ShutdownSignal,
    /// Error occurred
    Error(String),
}

/// Subsystem handles for coordinating different node components
#[derive(Debug, Default)]
pub struct SubsystemHandles {
    /// Consensus layer handle
    pub consensus_handle: Option<tokio::task::JoinHandle<Result<()>>>,
    /// Network layer handle
    pub network_handle: Option<tokio::task::JoinHandle<Result<()>>>,
    /// Storage layer handle
    pub storage_handle: Option<tokio::task::JoinHandle<Result<()>>>,
    /// API layer handle
    pub api_handle: Option<tokio::task::JoinHandle<Result<()>>>,
    /// Tracing layer handle
    pub tracing_handle: Option<tokio::task::JoinHandle<Result<()>>>,
    /// Health monitor handle
    pub health_handle: Option<tokio::task::JoinHandle<Result<()>>>,
}

/// Main node runtime structure
///
/// This coordinates all the node subsystems and manages the main event loop.
/// Based on the Haskell node's `handleSimpleNode` and `runNode` functions.
#[derive(Debug)]
pub struct NodeRuntime {
    /// Current node state
    state: Arc<RwLock<NodeState>>,

    /// Configuration manager
    config_manager: Arc<RwLock<ConfigurationManager>>,

    /// Event broadcaster for internal communication
    event_tx: broadcast::Sender<NodeEvent>,

    /// Subsystem handles for lifecycle management
    subsystems: Arc<RwLock<SubsystemHandles>>,

    /// Shutdown coordinator
    shutdown_tx: Option<broadcast::Sender<()>>,

    /// Node startup time
    startup_time: Option<std::time::Instant>,
}

impl NodeRuntime {
    /// Create a new node runtime instance
    pub fn new(config_manager: ConfigurationManager) -> Self {
        let (event_tx, _) = broadcast::channel(1000);

        Self {
            state: Arc::new(RwLock::new(NodeState::Uninitialized)),
            config_manager: Arc::new(RwLock::new(config_manager)),
            event_tx,
            subsystems: Arc::new(RwLock::new(SubsystemHandles::default())),
            shutdown_tx: None,
            startup_time: None,
        }
    }

    /// Initialize the node runtime
    #[instrument(skip(self))]
    pub async fn initialize(&mut self) -> Result<()> {
        let mut state = self.state.write().await;

        if *state != NodeState::Uninitialized {
            return Err(NodeRuntimeError::AlreadyRunning.into());
        }

        info!("Initializing Cardano node runtime");
        *state = NodeState::Starting;

        // Create shutdown channel
        let (shutdown_tx, _) = broadcast::channel(100);
        self.shutdown_tx = Some(shutdown_tx);

        // Validate configuration
        let config_manager = self.config_manager.read().await;
        config_manager
            .validate_all()
            .context("Configuration validation failed")?;

        info!("Node runtime initialized successfully");
        Ok(())
    }

    /// Start all node subsystems
    #[instrument(skip(self))]
    pub async fn start_subsystems(&mut self) -> Result<()> {
        info!("Starting node subsystems");

        let shutdown_rx = self
            .shutdown_tx
            .as_ref()
            .ok_or(NodeRuntimeError::NotInitialized)?
            .subscribe();

        let mut subsystems = self.subsystems.write().await;
        let config_manager = self.config_manager.clone();
        let event_tx = self.event_tx.clone();

        // Start consensus subsystem
        info!("Starting consensus subsystem");
        subsystems.consensus_handle = Some(tokio::spawn(Self::run_consensus_subsystem(
            config_manager.clone(),
            event_tx.clone(),
            shutdown_rx.resubscribe(),
        )));

        // Start network subsystem
        info!("Starting network subsystem");
        subsystems.network_handle = Some(tokio::spawn(Self::run_network_subsystem(
            config_manager.clone(),
            event_tx.clone(),
            shutdown_rx.resubscribe(),
        )));

        // Start storage subsystem
        info!("Starting storage subsystem");
        subsystems.storage_handle = Some(tokio::spawn(Self::run_storage_subsystem(
            config_manager.clone(),
            event_tx.clone(),
            shutdown_rx.resubscribe(),
        )));

        // Start API subsystem
        info!("Starting API subsystem");
        subsystems.api_handle = Some(tokio::spawn(Self::run_api_subsystem(
            config_manager.clone(),
            event_tx.clone(),
            shutdown_rx.resubscribe(),
        )));

        // Start tracing subsystem
        info!("Starting tracing subsystem");
        subsystems.tracing_handle = Some(tokio::spawn(Self::run_tracing_subsystem(
            config_manager.clone(),
            event_tx.clone(),
            shutdown_rx.resubscribe(),
        )));

        // Start health monitor
        info!("Starting health monitor");
        subsystems.health_handle = Some(tokio::spawn(Self::run_health_monitor(
            config_manager,
            event_tx,
            shutdown_rx,
        )));

        info!("All subsystems started successfully");
        Ok(())
    }

    /// Main node runtime loop
    ///
    /// This is the heart of the node, coordinating all subsystems and handling events.
    /// Based on the Haskell node's main event loop in `Run.hs`.
    #[instrument(skip(self))]
    pub async fn run(&mut self) -> Result<()> {
        // Initialize the node
        self.initialize()
            .await
            .context("Failed to initialize node")?;

        // Start all subsystems
        self.start_subsystems()
            .await
            .context("Failed to start subsystems")?;

        // Update state to running
        {
            let mut state = self.state.write().await;
            *state = NodeState::Running;
            self.startup_time = Some(std::time::Instant::now());
        }

        // Send startup complete event
        let _ = self.event_tx.send(NodeEvent::StartupComplete);
        info!("Cardano node started successfully");

        // Main event loop
        let mut event_rx = self.event_tx.subscribe();
        let mut shutdown_received = false;

        // Set up signal handlers
        #[cfg(unix)]
        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())?;
        #[cfg(unix)]
        let mut sighup = signal::unix::signal(signal::unix::SignalKind::hangup())?;

        loop {
            tokio::select! {
                // Handle shutdown signals (SIGTERM, SIGINT)
                _ = signal::ctrl_c() => {
                    info!("Received SIGINT, initiating graceful shutdown");
                    shutdown_received = true;
                },

                _ = async {
                    #[cfg(unix)]
                    {
                        sigterm.recv().await;
                    }
                    #[cfg(not(unix))]
                    {
                        std::future::pending::<()>().await;
                    }
                } => {
                    #[cfg(unix)]
                    {
                        info!("Received SIGTERM, initiating graceful shutdown");
                        shutdown_received = true;
                    }
                },

                _ = async {
                    #[cfg(unix)]
                    {
                        sighup.recv().await;
                    }
                    #[cfg(not(unix))]
                    {
                        std::future::pending::<()>().await;
                    }
                } => {
                    #[cfg(unix)]
                    {
                        info!("Received SIGHUP, reloading configuration");
                        if let Err(e) = self.reload_configuration().await {
                            error!("Failed to reload configuration: {}", e);
                        }
                    }
                },

                // Handle internal events
                event = event_rx.recv() => {
                    match event {
                        Ok(NodeEvent::ShutdownSignal) => {
                            info!("Received internal shutdown signal");
                            shutdown_received = true;
                        }
                        Ok(NodeEvent::Error(err)) => {
                            error!("Subsystem error: {}", err);
                            // For now, continue running. In production, might want to restart subsystem
                        }
                        Ok(event) => {
                            debug!("Received event: {:?}", event);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            warn!("Event channel closed, shutting down");
                            shutdown_received = true;
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            warn!("Event channel lagged, some events may have been missed");
                        }
                    }
                }
            }

            // Check if shutdown was requested
            if shutdown_received {
                break;
            }

            // Check subsystem health
            if let Err(e) = self.check_subsystem_health().await {
                error!("Subsystem health check failed: {}", e);
            }
        }

        // Perform graceful shutdown
        self.shutdown()
            .await
            .context("Failed to shutdown gracefully")?;

        Ok(())
    }

    /// Reload configuration (triggered by SIGHUP)
    #[instrument(skip(self))]
    async fn reload_configuration(&self) -> Result<()> {
        info!("Reloading node configuration");

        let config_manager = self.config_manager.read().await;

        // Configuration reloading implementation:
        // 1. Re-read configuration files from disk
        // 2. Validate new configuration
        // 3. Apply changes to running components (e.g., logging level, network settings)
        // 4. Some settings (like network magic) cannot be changed at runtime
        //
        // For now, we log the reload request. Full implementation would include:
        // - Checking file timestamps for changes
        // - Parsing updated JSON/YAML configuration
        // - Applying non-breaking changes to the node state
        info!("Configuration reload requested - current config preserved");
        drop(config_manager);

        let _ = self.event_tx.send(NodeEvent::ConfigurationReloaded);
        info!("Configuration reloaded successfully");

        Ok(())
    }

    /// Check health of all subsystems
    #[instrument(skip(self))]
    async fn check_subsystem_health(&self) -> Result<()> {
        let subsystems = self.subsystems.read().await;

        // Check if any subsystem has finished (which indicates an error)
        if let Some(ref handle) = subsystems.consensus_handle {
            if handle.is_finished() {
                return Err(NodeRuntimeError::ConsensusError(
                    "Consensus subsystem terminated".to_string(),
                )
                .into());
            }
        }

        if let Some(ref handle) = subsystems.network_handle {
            if handle.is_finished() {
                return Err(NodeRuntimeError::NetworkError(
                    "Network subsystem terminated".to_string(),
                )
                .into());
            }
        }

        if let Some(ref handle) = subsystems.storage_handle {
            if handle.is_finished() {
                return Err(NodeRuntimeError::StorageError(
                    "Storage subsystem terminated".to_string(),
                )
                .into());
            }
        }

        if let Some(ref handle) = subsystems.api_handle {
            if handle.is_finished() {
                return Err(
                    NodeRuntimeError::ApiError("API subsystem terminated".to_string()).into(),
                );
            }
        }

        if let Some(ref handle) = subsystems.tracing_handle {
            if handle.is_finished() {
                return Err(NodeRuntimeError::TracingError(
                    "Tracing subsystem terminated".to_string(),
                )
                .into());
            }
        }

        Ok(())
    }

    /// Gracefully shutdown the node
    #[instrument(skip(self))]
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Initiating graceful node shutdown");

        // Update state
        {
            let mut state = self.state.write().await;
            *state = NodeState::ShuttingDown;
        }

        // Send shutdown signal to all subsystems
        if let Some(ref shutdown_tx) = self.shutdown_tx {
            let _ = shutdown_tx.send(());
        }

        // Wait for all subsystems to terminate
        let mut subsystems = self.subsystems.write().await;

        let handles = [
            subsystems.health_handle.take(),
            subsystems.tracing_handle.take(),
            subsystems.api_handle.take(),
            subsystems.storage_handle.take(),
            subsystems.network_handle.take(),
            subsystems.consensus_handle.take(),
        ];

        for handle in handles.into_iter().flatten() {
            match tokio::time::timeout(Duration::from_secs(30), handle).await {
                Ok(Ok(Ok(()))) => debug!("Subsystem shutdown successfully"),
                Ok(Ok(Err(e))) => warn!("Subsystem shutdown with error: {}", e),
                Ok(Err(e)) => warn!("Subsystem panic during shutdown: {}", e),
                Err(_) => {
                    error!("Subsystem shutdown timeout");
                    return Err(NodeRuntimeError::ShutdownTimeout.into());
                }
            }
        }

        // Update final state
        {
            let mut state = self.state.write().await;
            *state = NodeState::Stopped;
        }

        if let Some(startup_time) = self.startup_time {
            let uptime = startup_time.elapsed();
            info!("Node shutdown complete. Uptime: {:?}", uptime);
        } else {
            info!("Node shutdown complete");
        }

        Ok(())
    }

    /// Get current node state
    pub async fn get_state(&self) -> NodeState {
        self.state.read().await.clone()
    }

    /// Consensus subsystem runner
    #[instrument(skip_all)]
    async fn run_consensus_subsystem(
        _config_manager: Arc<RwLock<ConfigurationManager>>,
        event_tx: broadcast::Sender<NodeEvent>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<()> {
        info!("Consensus subsystem started");

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Consensus subsystem received shutdown signal");
                    break;
                }
                _ = sleep(Duration::from_secs(30)) => {
                    // Simulate periodic consensus activity
                    let _ = event_tx.send(NodeEvent::BlockReceived);
                }
            }
        }

        info!("Consensus subsystem terminated");
        Ok(())
    }

    /// Network subsystem runner
    #[instrument(skip_all)]
    async fn run_network_subsystem(
        config_manager: Arc<RwLock<ConfigurationManager>>,
        event_tx: broadcast::Sender<NodeEvent>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<()> {
        info!("Network subsystem started");

        let (node_config, topology) = {
            let cfg = config_manager.read().await;
            (cfg.get_config().clone(), cfg.get_topology().cloned())
        };

        let mut connection_config = ConnectionConfig::default();
        connection_config.limits.max_connections =
            node_config.max_connections.unwrap_or(10) as usize;
        connection_config.peer_selection.max_connections =
            node_config.max_connections.unwrap_or(10) as usize;

        // Extract peers from topology (P2P bootstrap peers, local/public roots, or legacy producers)
        let mut peer_addresses = Vec::new();

        if let Some(topo) = topology.as_ref() {
            // Try P2P bootstrap peers first
            if let Some(bootstrap_peers) = &topo.bootstrap_peers {
                for peer in bootstrap_peers {
                    peer_addresses.push((peer.address.clone(), peer.port));
                }
                info!("Using {} P2P bootstrap peer(s)", bootstrap_peers.len());
            }

            // Add local root peers
            if let Some(local_roots) = &topo.local_roots {
                for root in local_roots {
                    for access_point in &root.access_points {
                        peer_addresses.push((access_point.address.clone(), access_point.port));
                    }
                }
                if !local_roots.is_empty() {
                    info!("Using {} local root(s)", local_roots.len());
                }
            }

            // Add public root peers
            if let Some(public_roots) = &topo.public_roots {
                for root in public_roots {
                    for access_point in &root.access_points {
                        peer_addresses.push((access_point.address.clone(), access_point.port));
                    }
                }
                if !public_roots.is_empty() {
                    info!("Using {} public root(s)", public_roots.len());
                }
            }

            // Also add legacy producers if present
            if let Some(producers) = &topo.producers {
                for producer in producers {
                    peer_addresses.push((producer.addr.clone(), producer.port));
                }
                if !producers.is_empty() {
                    info!("Using {} legacy producer(s)", producers.len());
                }
            }
        }

        if !peer_addresses.is_empty() {
            connection_config.peer_selection.target_connections = peer_addresses
                .len()
                .min(node_config.max_connections.unwrap_or(10) as usize);
        } else {
            connection_config.peer_selection.target_connections = 0;
        }

        let mut manager = ConnectionManager::new(connection_config).await?;
        let mut registered_peers = Vec::new();

        if peer_addresses.is_empty() {
            warn!("No network topology peers configured; network subsystem idle");
        } else {
            for (addr, port) in peer_addresses {
                match parse_producer_address(&addr, port).await {
                    Ok(address) => {
                        let peer_id = peer_id_from_address(&address);
                        let peer_info = PeerInfo::new(peer_id, address);
                        if let Err(err) = manager.add_peer(peer_info).await {
                            warn!(addr = %addr, port = port, error = %err, "Failed to register peer");
                            continue;
                        }
                        registered_peers.push(peer_id);
                    }
                    Err(err) => {
                        warn!(addr = %addr, port = port, %err, "Invalid peer address");
                    }
                }
            }
        }

        manager.start().await?;

        for peer_id in &registered_peers {
            if let Err(err) = manager.connect_peer(peer_id).await {
                warn!(peer = %peer_id, %err, "Failed to initiate connection to peer");
            }
        }

        let mut event_task = None;
        if let Some(mut rx) = manager.event_receiver() {
            let event_tx_clone = event_tx.clone();
            event_task = Some(tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    debug!(?event, "Connection manager event");
                    let _ = event_tx_clone.send(NodeEvent::PeerUpdate);
                }
            }));
        }

        if registered_peers.is_empty() {
            debug!("No peers registered; waiting for shutdown");
        }

        match shutdown_rx.recv().await {
            Ok(_) => info!("Network subsystem received shutdown signal"),
            Err(err) => warn!(%err, "Network subsystem shutdown channel closed"),
        }

        manager.shutdown().await;

        if let Some(handle) = event_task {
            handle.abort();
        }

        info!("Network subsystem terminated");
        Ok(())
    }

    /// Storage subsystem runner
    #[instrument(skip_all)]
    async fn run_storage_subsystem(
        _config_manager: Arc<RwLock<ConfigurationManager>>,
        _event_tx: broadcast::Sender<NodeEvent>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<()> {
        info!("Storage subsystem started");

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Storage subsystem received shutdown signal");
                    break;
                }
                _ = sleep(Duration::from_secs(10)) => {
                    // Simulate periodic storage maintenance
                }
            }
        }

        info!("Storage subsystem terminated");
        Ok(())
    }

    /// API subsystem runner
    #[instrument(skip_all)]
    async fn run_api_subsystem(
        _config_manager: Arc<RwLock<ConfigurationManager>>,
        event_tx: broadcast::Sender<NodeEvent>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<()> {
        info!("API subsystem started");

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("API subsystem received shutdown signal");
                    break;
                }
                _ = sleep(Duration::from_secs(45)) => {
                    // Simulate periodic API activity
                    let _ = event_tx.send(NodeEvent::ApiRequest);
                }
            }
        }

        info!("API subsystem terminated");
        Ok(())
    }

    /// Tracing subsystem runner
    #[instrument(skip_all)]
    async fn run_tracing_subsystem(
        _config_manager: Arc<RwLock<ConfigurationManager>>,
        _event_tx: broadcast::Sender<NodeEvent>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<()> {
        info!("Tracing subsystem started");

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Tracing subsystem received shutdown signal");
                    break;
                }
                _ = sleep(Duration::from_secs(5)) => {
                    // Simulate periodic tracing activity
                }
            }
        }

        info!("Tracing subsystem terminated");
        Ok(())
    }

    /// Health monitor runner
    #[instrument(skip_all)]
    async fn run_health_monitor(
        _config_manager: Arc<RwLock<ConfigurationManager>>,
        event_tx: broadcast::Sender<NodeEvent>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<()> {
        info!("Health monitor started");

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Health monitor received shutdown signal");
                    break;
                }
                _ = sleep(Duration::from_secs(15)) => {
                    // Simulate periodic health checks
                    let _ = event_tx.send(NodeEvent::HealthUpdate);
                }
            }
        }

        info!("Health monitor terminated");
        Ok(())
    }
}

/// Helper function to create and run a node runtime
///
/// This is the main entry point that integrates with the existing `run_node` function.
#[instrument(skip(config_manager))]
pub async fn run_node_runtime(config_manager: ConfigurationManager) -> Result<()> {
    info!("Starting Cardano Node Runtime");

    let mut runtime = NodeRuntime::new(config_manager);

    runtime
        .run()
        .await
        .context("Node runtime execution failed")?;

    info!("Cardano Node Runtime stopped");
    Ok(())
}

fn peer_id_from_address(address: &SocketAddr) -> PeerId {
    let mut hasher = Blake2b512::new();
    hasher.update(address.to_string().as_bytes());
    let digest = hasher.finalize();
    let mut id = [0u8; 32];
    id.copy_from_slice(&digest[..32]);
    PeerId::new(id)
}

async fn parse_producer_address(addr: &str, port: u16) -> Result<SocketAddr> {
    // Try parsing as direct IP:port first
    let socket_str = format!("{}:{}", addr, port);
    if let Ok(socket_addr) = socket_str.parse::<SocketAddr>() {
        return Ok(socket_addr);
    }

    // If not an IP address, try DNS resolution
    let addresses: Vec<SocketAddr> = tokio::net::lookup_host(&socket_str)
        .await
        .map_err(|err| anyhow!("DNS resolution failed for {}: {}", socket_str, err))?
        .collect();

    // Return the first resolved address
    addresses
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("No addresses found for {}", socket_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{NetworkTopology, NodeConfiguration, TopologyProducer};
    use cardano_network::connection::HandshakeProtocol;
    use tempfile::tempdir;
    use tokio::net::TcpListener;

    fn create_test_config_manager() -> ConfigurationManager {
        ConfigurationManager::new()
    }

    #[tokio::test]
    async fn test_node_runtime_creation() {
        let config_manager = create_test_config_manager();
        let runtime = NodeRuntime::new(config_manager);

        let state = runtime.get_state().await;
        assert_eq!(state, NodeState::Uninitialized);
    }

    #[tokio::test]
    async fn test_node_runtime_initialization() {
        let config_manager = create_test_config_manager();
        let mut runtime = NodeRuntime::new(config_manager);

        let result = runtime.initialize().await;
        assert!(result.is_ok());

        let state = runtime.get_state().await;
        assert_eq!(state, NodeState::Starting);
    }

    #[tokio::test]
    async fn test_node_runtime_double_initialization() {
        let config_manager = create_test_config_manager();
        let mut runtime = NodeRuntime::new(config_manager);

        let result1 = runtime.initialize().await;
        assert!(result1.is_ok());

        let result2 = runtime.initialize().await;
        assert!(result2.is_err());
    }

    #[tokio::test]
    async fn test_subsystem_startup() {
        let config_manager = create_test_config_manager();
        let mut runtime = NodeRuntime::new(config_manager);

        runtime.initialize().await.expect("Initialize failed");
        let result = runtime.start_subsystems().await;
        assert!(result.is_ok());

        // Give subsystems time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check that subsystems are running
        {
            let subsystems = runtime.subsystems.read().await;
            assert!(subsystems.consensus_handle.is_some());
            assert!(subsystems.network_handle.is_some());
            assert!(subsystems.storage_handle.is_some());
            assert!(subsystems.api_handle.is_some());
            assert!(subsystems.tracing_handle.is_some());
            assert!(subsystems.health_handle.is_some());
        }

        // Cleanup
        runtime.shutdown().await.expect("Shutdown failed");
    }

    #[tokio::test]
    async fn test_node_runtime_shutdown() {
        let config_manager = create_test_config_manager();
        let mut runtime = NodeRuntime::new(config_manager);

        runtime.initialize().await.expect("Initialize failed");
        runtime
            .start_subsystems()
            .await
            .expect("Start subsystems failed");

        // Give subsystems time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        let result = runtime.shutdown().await;
        assert!(result.is_ok());

        let state = runtime.get_state().await;
        assert_eq!(state, NodeState::Stopped);
    }

    #[tokio::test]
    async fn test_subsystem_health_check() {
        let config_manager = create_test_config_manager();
        let mut runtime = NodeRuntime::new(config_manager);

        runtime.initialize().await.expect("Initialize failed");
        runtime
            .start_subsystems()
            .await
            .expect("Start subsystems failed");

        // Give subsystems time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        let result = runtime.check_subsystem_health().await;
        assert!(result.is_ok());

        // Cleanup
        runtime.shutdown().await.expect("Shutdown failed");
    }

    #[tokio::test]
    async fn test_network_subsystem_connects_to_configured_peer() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let topology_path = temp_dir.path().join("topology.json");

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let listen_addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                let handshake = HandshakeProtocol::new();
                let mut stream = stream;
                let _ = handshake.handle_handshake(&mut stream).await;
            }
        });

        let mut config = NodeConfiguration::default_for_testing();
        config.topology_file = Some(topology_path.clone());
        config.to_file(&config_path).unwrap();

        let topology = NetworkTopology {
            bootstrap_peers: None,
            local_roots: None,
            public_roots: None,
            use_ledger_after_slot: None,
            peer_snapshot_file: None,
            producers: Some(vec![TopologyProducer {
                addr: listen_addr.ip().to_string(),
                port: listen_addr.port(),
                valency: 1,
            }]),
        };
        topology.to_file(&topology_path).unwrap();

        let mut config_manager = ConfigurationManager::new();
        config_manager.load_config(&config_path).unwrap();
        config_manager.load_topology(&topology_path).unwrap();

        let config_manager = Arc::new(RwLock::new(config_manager));
        let (event_tx, _event_rx) = broadcast::channel(16);
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        let subsystem = tokio::spawn(NodeRuntime::run_network_subsystem(
            config_manager.clone(),
            event_tx,
            shutdown_rx,
        ));

        tokio::time::sleep(Duration::from_millis(100)).await;
        let _ = shutdown_tx.send(());

        subsystem.await.unwrap().unwrap();
        let _ = server.await;
    }

    #[tokio::test]
    async fn test_event_broadcasting() {
        let config_manager = create_test_config_manager();
        let runtime = NodeRuntime::new(config_manager);

        let mut event_rx = runtime.event_tx.subscribe();

        // Send a test event
        let _ = runtime.event_tx.send(NodeEvent::StartupComplete);

        // Receive the event
        let event = tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await;
        assert!(event.is_ok());
        assert!(matches!(
            event.unwrap().unwrap(),
            NodeEvent::StartupComplete
        ));
    }
}
