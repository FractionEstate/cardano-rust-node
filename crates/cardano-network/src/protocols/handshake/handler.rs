//! Handshake Protocol Handler
//!
//! Implements the ProtocolHandler trait for the handshake protocol,
//! integrating it with the connection multiplexer.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::Mutex;
use tracing::{debug, error};

use crate::connection::multiplexer::{ProtocolHandler, ProtocolId};
use crate::connection::ConnectionId;
use crate::Result;

use super::state::HandshakeClient;
use super::types::NetworkMagic;
use super::HandshakeResult;

/// Handler for the handshake protocol
/// Manages per-connection handshake state
pub struct HandshakeProtocolHandler {
    /// Per-connection handshake clients
    clients: Arc<Mutex<HashMap<ConnectionId, HandshakeClient>>>,
    /// Network magic for all connections
    network_magic: NetworkMagic,
    /// Protocol identifier
    protocol_id: ProtocolId,
}

impl HandshakeProtocolHandler {
    /// Create a new handshake protocol handler
    pub fn new(network_magic: NetworkMagic) -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            network_magic,
            protocol_id: ProtocolId::HANDSHAKE,
        }
    }

    /// Start handshake for a connection (returns initial message to send)
    pub async fn start(&self, connection_id: ConnectionId) -> Result<Bytes> {
        let mut clients = self.clients.lock().await;

        // Create new client for this connection
        let mut client = HandshakeClient::new(self.network_magic);
        let initial_msg = client.start()?;

        clients.insert(connection_id, client);

        Ok(initial_msg)
    }

    /// Get the current handshake result (if completed)
    pub async fn result(&self, connection_id: ConnectionId) -> Option<HandshakeResult> {
        let clients = self.clients.lock().await;
        clients
            .get(&connection_id)
            .and_then(|c| c.result().cloned())
    }

    /// Check if handshake is complete
    pub async fn is_done(&self, connection_id: ConnectionId) -> bool {
        let clients = self.clients.lock().await;
        clients
            .get(&connection_id)
            .map(|c| c.state().is_done())
            .unwrap_or(false)
    }

    /// Check if handshake failed
    pub async fn is_failed(&self, connection_id: ConnectionId) -> bool {
        let clients = self.clients.lock().await;
        clients
            .get(&connection_id)
            .map(|c| c.state().is_failed())
            .unwrap_or(false)
    }

    /// Check for handshake timeout
    pub async fn check_timeout(&self, connection_id: ConnectionId) -> Result<()> {
        let mut clients = self.clients.lock().await;
        if let Some(client) = clients.get_mut(&connection_id) {
            client.check_timeout()?;
        }
        Ok(())
    }

    /// Remove completed handshake state (cleanup)
    pub async fn remove_connection(&self, connection_id: ConnectionId) {
        let mut clients = self.clients.lock().await;
        clients.remove(&connection_id);
    }
}

impl ProtocolHandler for HandshakeProtocolHandler {
    fn handle_message(
        &self,
        connection_id: ConnectionId,
        message: Bytes,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Option<Bytes>>> + Send>> {
        let clients = Arc::clone(&self.clients);

        Box::pin(async move {
            debug!(
                connection_id = ?connection_id,
                protocol = "Handshake",
                "Handling handshake message"
            );

            let mut clients_guard = clients.lock().await;

            if let Some(client) = clients_guard.get_mut(&connection_id) {
                match client.handle_message(&message) {
                    Ok(response) => {
                        debug!(
                            connection_id = ?connection_id,
                            state = %client.state(),
                            has_response = response.is_some(),
                            "Handshake message processed"
                        );
                        Ok(response)
                    }
                    Err(e) => {
                        error!(
                            connection_id = ?connection_id,
                            error = %e,
                            "Handshake error"
                        );
                        Err(e.into())
                    }
                }
            } else {
                error!(connection_id = ?connection_id, "No handshake client for connection");
                Err(crate::NetworkError::ProtocolError(
                    "Handshake not started for connection".to_string(),
                ))
            }
        })
    }

    fn protocol_id(&self) -> ProtocolId {
        self.protocol_id
    }

    fn name(&self) -> &str {
        "Handshake"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::handshake::codec::encode_message;
    use crate::protocols::handshake::messages::HandshakeMessage;
    use crate::protocols::handshake::types::NodeToNodeVersion;
    use crate::protocols::handshake::types::NodeToNodeVersionData;

    #[tokio::test]
    async fn test_handler_creation() {
        let handler = HandshakeProtocolHandler::new(NetworkMagic::PREVIEW_TESTNET);
        assert_eq!(handler.protocol_id(), ProtocolId::HANDSHAKE);
        assert_eq!(handler.name(), "Handshake");

        let conn_id = ConnectionId::new();
        assert!(!handler.is_done(conn_id).await);
        assert!(!handler.is_failed(conn_id).await);
    }

    #[tokio::test]
    async fn test_handler_start_and_accept() {
        let handler = HandshakeProtocolHandler::new(NetworkMagic::PREVIEW_TESTNET);
        let connection_id = ConnectionId::new();

        // Start handshake
        let proposal = handler.start(connection_id).await.expect("Failed to start");
        assert!(!proposal.is_empty());

        // Simulate server accepting V15
        let accept_msg = HandshakeMessage::accept_version(
            NodeToNodeVersion::V15,
            NodeToNodeVersionData::preview_testnet(),
        );
        let encoded = encode_message(&accept_msg).unwrap();

        // Handle acceptance
        let response = handler
            .handle_message(connection_id, encoded)
            .await
            .expect("Failed to handle message");

        assert!(response.is_none()); // No response needed
        assert!(handler.is_done(connection_id).await);

        let result = handler.result(connection_id).await.expect("No result");
        assert_eq!(result.version, NodeToNodeVersion::V15);
        assert!(result.is_preview_testnet());
    }
}
