//! Local Socket API Implementation
//!
//! Provides Unix domain socket IPC communication compatible with cardano-cli
//! and other tools. This enables queries for blockchain data, transaction
//! submission, and node status information.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info, warn};

use crate::{ApiError, Result};

pub mod protocol;
pub mod handlers;
pub mod client;

pub use protocol::*;
pub use handlers::*;
pub use client::*;

/// Configuration for the local socket server
#[derive(Debug, Clone)]
pub struct LocalSocketConfig {
    /// Path to the Unix domain socket
    pub socket_path: PathBuf,
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Connection timeout in seconds
    pub connection_timeout: u64,
}

impl Default for LocalSocketConfig {
    fn default() -> Self {
        Self {
            socket_path: PathBuf::from("/tmp/cardano-node.sock"),
            max_connections: 100,
            connection_timeout: 300,
        }
    }
}

/// Local socket server for IPC communication
pub struct LocalSocketServer {
    config: LocalSocketConfig,
    listener: Option<UnixListener>,
    handlers: Arc<LocalSocketHandlers>,
}

impl LocalSocketServer {
    /// Create a new local socket server
    pub fn new(config: LocalSocketConfig, handlers: LocalSocketHandlers) -> Self {
        Self {
            config,
            listener: None,
            handlers: Arc::new(handlers),
        }
    }

    /// Start the socket server
    pub async fn start(&mut self) -> Result<()> {
        // Remove existing socket file
        if self.config.socket_path.exists() {
            std::fs::remove_file(&self.config.socket_path)
                .map_err(|e| ApiError::InternalError(format!("Failed to remove existing socket: {}", e)))?;
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = self.config.socket_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ApiError::InternalError(format!("Failed to create socket directory: {}", e)))?;
        }

        // Bind to Unix socket
        let listener = UnixListener::bind(&self.config.socket_path)
            .map_err(|e| ApiError::InternalError(format!("Failed to bind socket: {}", e)))?;

        info!("Local socket server listening on {:?}", self.config.socket_path);

        self.listener = Some(listener);
        Ok(())
    }

    /// Run the server event loop
    pub async fn run(&self) -> Result<()> {
        let listener = self.listener
            .as_ref()
            .ok_or_else(|| ApiError::InternalError("Server not started".to_string()))?;

        let mut connection_count = 0;

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    if connection_count >= self.config.max_connections {
                        warn!("Maximum connections reached, rejecting new connection");
                        continue;
                    }

                    connection_count += 1;
                    let handlers = self.handlers.clone();
                    let timeout = self.config.connection_timeout;

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, handlers, timeout).await {
                            error!("Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    /// Handle a single client connection
    async fn handle_connection(
        stream: UnixStream,
        handlers: Arc<LocalSocketHandlers>,
        timeout_secs: u64,
    ) -> Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        debug!("New local socket connection established");

        let timeout_duration = std::time::Duration::from_secs(timeout_secs);

        loop {
            line.clear();

            // Read request with timeout
            let bytes_read = match tokio::time::timeout(timeout_duration, reader.read_line(&mut line)).await {
                Ok(Ok(bytes)) => bytes,
                Ok(Err(e)) => {
                    debug!("Connection read error: {}", e);
                    break;
                }
                Err(_) => {
                    debug!("Connection timeout");
                    break;
                }
            };

            if bytes_read == 0 {
                debug!("Connection closed by client");
                break;
            }

            // Process request
            let response = handlers.handle_request(&line).await;
            let response_json = serde_json::to_string(&response)
                .map_err(|e| ApiError::SerializationError(e.to_string()))?;

            // Send response
            if let Err(e) = writer.write_all(format!("{}\n", response_json).as_bytes()).await {
                debug!("Failed to write response: {}", e);
                break;
            }

            if let Err(e) = writer.flush().await {
                debug!("Failed to flush response: {}", e);
                break;
            }
        }

        debug!("Local socket connection closed");
        Ok(())
    }

    /// Stop the server and cleanup
    pub fn stop(&mut self) -> Result<()> {
        if let Some(_) = self.listener.take() {
            // Remove socket file
            if self.config.socket_path.exists() {
                std::fs::remove_file(&self.config.socket_path)
                    .map_err(|e| ApiError::InternalError(format!("Failed to remove socket: {}", e)))?;
            }
            info!("Local socket server stopped");
        }
        Ok(())
    }

    /// Get the socket path
    pub fn socket_path(&self) -> &Path {
        &self.config.socket_path
    }
}

impl Drop for LocalSocketServer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UnixStream;

    #[tokio::test]
    async fn test_socket_server_creation() {
        let config = LocalSocketConfig::default();
        let handlers = LocalSocketHandlers::default();
        let _server = LocalSocketServer::new(config, handlers);
    }

    #[tokio::test]
    async fn test_socket_path_cleanup() {
        let socket_path = std::env::temp_dir().join("test_cardano_node.sock");
        let config = LocalSocketConfig {
            socket_path: socket_path.clone(),
            ..Default::default()
        };

        let handlers = LocalSocketHandlers::default();
        let mut server = LocalSocketServer::new(config, handlers);

        // Create socket file
        std::fs::write(&socket_path, "").expect("Failed to create test file");
        assert!(socket_path.exists());

        // Start server should remove existing file
        server.start().await.expect("Failed to start server");

        // Stop should cleanup
        server.stop().expect("Failed to stop server");
        assert!(!socket_path.exists());
    }
}
