//! Connection Multiplexer
//!
//! This module implements protocol multiplexing over a single TCP connection,
//! allowing multiple Cardano mini-protocols to share the same network connection
//! with proper message framing, routing, and flow control.

use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

use super::{ConnectionError, ConnectionId};
use crate::Result;

/// Protocol identifier for message routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProtocolId(u16);

impl ProtocolId {
    /// Handshake protocol
    pub const HANDSHAKE: Self = Self(0);
    /// ChainSync protocol
    pub const CHAINSYNC: Self = Self(2);
    /// BlockFetch protocol
    pub const BLOCKFETCH: Self = Self(3);
    /// TxSubmission protocol
    pub const TXSUBMISSION: Self = Self(4);
    /// KeepAlive protocol
    pub const KEEPALIVE: Self = Self(8);
    /// Gossip protocol
    pub const GOSSIP: Self = Self(9);

    /// Create custom protocol ID
    pub const fn new(id: u16) -> Self {
        Self(id)
    }

    /// Get protocol ID value
    pub fn value(&self) -> u16 {
        self.0
    }
}

impl fmt::Display for ProtocolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::HANDSHAKE => write!(f, "Handshake"),
            Self::CHAINSYNC => write!(f, "ChainSync"),
            Self::BLOCKFETCH => write!(f, "BlockFetch"),
            Self::TXSUBMISSION => write!(f, "TxSubmission"),
            Self::KEEPALIVE => write!(f, "KeepAlive"),
            Self::GOSSIP => write!(f, "Gossip"),
            _ => write!(f, "Protocol({})", self.0),
        }
    }
}

/// Message frame for protocol multiplexing
#[derive(Debug, Clone)]
pub struct MessageFrame {
    /// Target protocol
    pub protocol_id: ProtocolId,
    /// Message payload
    pub payload: Bytes,
    /// Frame timestamp
    pub timestamp: std::time::Instant,
}

impl MessageFrame {
    /// Create new message frame
    pub fn new(protocol_id: ProtocolId, payload: Bytes) -> Self {
        Self {
            protocol_id,
            payload,
            timestamp: std::time::Instant::now(),
        }
    }

    /// Get frame size including header
    pub fn frame_size(&self) -> usize {
        // 8 bytes for mux frame header + payload size
        // Header format: [timestamp: u16][protocol_id: u16][length: u16][reserved: u16]
        8 + self.payload.len()
    }

    /// Encode frame to bytes
    pub fn encode(&self) -> Result<Bytes> {
        let mut buf = BytesMut::with_capacity(self.frame_size());

        // Cardano mux frame format (from ouroboros-network):
        // [timestamp: u32][protocol_id: u16][payload_length: u16][payload: bytes]
        // timestamp: transmission time (0x00000000 for basic mode)
        buf.put_u32(0x00000000); // timestamp (unused in basic mode) - 32 bits!
        buf.put_u16(self.protocol_id.value()); // protocol ID - 16 bits
        buf.put_u16(self.payload.len() as u16); // length - 16 bits
        buf.put(self.payload.clone());

        Ok(buf.freeze())
    }

    /// Decode frame from bytes
    pub fn decode(mut data: Bytes) -> Result<Self> {
        if data.len() < 8 {
            return Err(MultiplexerError::InvalidFrameSize {
                expected: 8,
                actual: data.len(),
            }
            .into());
        }

        // Cardano mux frame format: [timestamp: u32][protocol_id: u16][length: u16][payload]
        let _timestamp = data.get_u32(); // unused - 32 bits
        let raw_protocol_id = data.get_u16(); // 16 bits - includes mode bit
        let payload_len = data.get_u16() as usize; // 16 bits

        // Extract protocol ID by masking off the mode bit (bit 15)
        // Bit 15: 0 = Initiator, 1 = Responder
        // We use the base protocol ID for routing
        let protocol_id = ProtocolId::new(raw_protocol_id & 0x7FFF);

        tracing::trace!(raw_protocol_id, actual_protocol_id = protocol_id.value(),
            is_responder = (raw_protocol_id & 0x8000) != 0, "Decoded protocol ID");

        if data.len() < payload_len {
            return Err(MultiplexerError::InvalidFrameSize {
                expected: payload_len,
                actual: data.len(),
            }
            .into());
        }

        let payload = data.slice(..payload_len);

        Ok(Self::new(protocol_id, payload))
    }
}

/// Protocol handler interface
pub trait ProtocolHandler: Send + Sync {
    /// Handle incoming message for this protocol
    fn handle_message(
        &self,
        connection_id: ConnectionId,
        message: Bytes,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Option<Bytes>>> + Send>>;

    /// Get protocol identifier
    fn protocol_id(&self) -> ProtocolId;

    /// Protocol name for logging
    fn name(&self) -> &str;

    /// Allow downcasting to concrete types
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Multiplexer error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum MultiplexerError {
    #[error("Protocol {0} not registered")]
    ProtocolNotRegistered(ProtocolId),

    #[error("Protocol {0} already registered")]
    ProtocolAlreadyRegistered(ProtocolId),

    #[error("Invalid frame size: expected {expected}, got {actual}")]
    InvalidFrameSize { expected: usize, actual: usize },

    #[error("Frame too large: {size} bytes (max: {max})")]
    FrameTooLarge { size: usize, max: usize },

    #[error("Protocol handler error: {0}")]
    HandlerError(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Send queue full")]
    SendQueueFull,

    #[error("Receive timeout")]
    ReceiveTimeout,

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Encoding error: {0}")]
    EncodingError(String),
}

impl From<std::io::Error> for MultiplexerError {
    fn from(error: std::io::Error) -> Self {
        Self::IoError(error.to_string())
    }
}

impl From<MultiplexerError> for ConnectionError {
    fn from(err: MultiplexerError) -> Self {
        ConnectionError::MultiplexerError(err)
    }
}

/// Connection multiplexer for handling multiple protocols
pub struct ConnectionMultiplexer {
    /// Connection identifier
    connection_id: ConnectionId,
    /// Registered protocol handlers
    handlers: Arc<RwLock<HashMap<ProtocolId, Arc<dyn ProtocolHandler>>>>,
    /// Outbound message sender
    outbound_tx: mpsc::UnboundedSender<MessageFrame>,
    /// Multiplexer configuration
    config: MultiplexerConfig,
    /// Task handles for cleanup
    tasks: Vec<JoinHandle<()>>,
}

impl fmt::Debug for ConnectionMultiplexer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectionMultiplexer")
            .field("connection_id", &self.connection_id)
            .field("config", &self.config)
            .field(
                "handlers_count",
                &self.handlers.try_read().map(|h| h.len()).unwrap_or(0),
            )
            .finish()
    }
}

impl ConnectionMultiplexer {
    /// Create new multiplexer for connection
    pub fn new(
        connection_id: ConnectionId,
        stream: TcpStream,
        config: MultiplexerConfig,
    ) -> Result<Self> {
        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
        let handlers = Arc::new(RwLock::new(HashMap::new()));

        let mut multiplexer = Self {
            connection_id,
            handlers: handlers.clone(),
            outbound_tx,
            config,
            tasks: Vec::new(),
        };

        // Start I/O tasks
        let (read_half, write_half) = stream.into_split();

        let read_task = multiplexer.spawn_reader(read_half, handlers.clone())?;
        let write_task = multiplexer.spawn_writer(write_half, outbound_rx)?;

        multiplexer.tasks.push(read_task);
        multiplexer.tasks.push(write_task);

        Ok(multiplexer)
    }

    /// Register protocol handler
    pub async fn register_protocol(&mut self, handler: Arc<dyn ProtocolHandler>) -> Result<()> {
        let protocol_id = handler.protocol_id();
        let mut handlers = self.handlers.write().await;

        if handlers.contains_key(&protocol_id) {
            return Err(MultiplexerError::ProtocolAlreadyRegistered(protocol_id).into());
        }

        handlers.insert(protocol_id, handler);
        Ok(())
    }

    /// Unregister protocol handler
    pub async fn unregister_protocol(&mut self, protocol_id: ProtocolId) -> Result<()> {
        let mut handlers = self.handlers.write().await;
        handlers.remove(&protocol_id);
        Ok(())
    }

    /// Send message to protocol
    pub fn send_message(&self, protocol_id: ProtocolId, payload: Bytes) -> Result<()> {
        let frame = MessageFrame::new(protocol_id, payload);

        self.outbound_tx
            .send(frame)
            .map_err(|_| MultiplexerError::SendQueueFull)?;

        Ok(())
    }

    /// Get registered protocols
    pub async fn registered_protocols(&self) -> Vec<ProtocolId> {
        let handlers = self.handlers.read().await;
        handlers.keys().copied().collect()
    }

    /// Close multiplexer and cleanup resources
    pub async fn close(&mut self) {
        // Abort all tasks
        for task in self.tasks.drain(..) {
            task.abort();
        }

        // Clear handlers
        self.handlers.write().await.clear();
    }

    /// Spawn reader task for incoming messages
    fn spawn_reader(
        &self,
        mut reader: tokio::net::tcp::OwnedReadHalf,
        handlers: Arc<RwLock<HashMap<ProtocolId, Arc<dyn ProtocolHandler>>>>,
    ) -> Result<JoinHandle<()>> {
        let connection_id = self.connection_id;
        let max_frame_size = self.config.max_frame_size;

        let task = tokio::spawn(async move {
            let mut buffer = BytesMut::with_capacity(8192);

            tracing::debug!(?connection_id, "Multiplexer reader task started");

            loop {
                // Read frame header
                match reader.read_buf(&mut buffer).await {
                    Ok(0) => {
                        // Connection closed
                        tracing::debug!(?connection_id, "Connection closed (read 0 bytes)");
                        break;
                    }
                    Ok(n) => {
                        tracing::trace!(?connection_id, bytes_read = n, buffer_len = buffer.len(), "Read data from connection");

                        // Process complete frames
                        while buffer.len() >= 8 {
                            let frame_len = {
                                let mut header = buffer.as_ref();
                                header.get_u32(); // timestamp (32-bit)
                                header.get_u16(); // protocol_id (16-bit)
                                let payload_len = header.get_u16() as usize; // payload_length (16-bit)
                                payload_len
                            } + 8; // + 8-byte mux header (4 + 2 + 2)

                            tracing::trace!(?connection_id, frame_len, buffer_len = buffer.len(), "Frame detected");

                            if frame_len > max_frame_size {
                                // Frame too large, close connection
                                tracing::error!(?connection_id, frame_len, max_frame_size, "Frame too large");
                                break;
                            }

                            if buffer.len() < frame_len {
                                // Need more data
                                tracing::trace!(?connection_id, frame_len, buffer_len = buffer.len(), "Need more data for complete frame");
                                break;
                            }

                            // Extract complete frame
                            let frame_data = buffer.split_to(frame_len);

                            match MessageFrame::decode(frame_data.freeze()) {
                                Ok(frame) => {
                                    tracing::debug!(?connection_id, protocol_id = %frame.protocol_id, payload_len = frame.payload.len(), "Received frame");
                                    // Handle frame
                                    Self::handle_incoming_frame(connection_id, frame, &handlers)
                                        .await;
                                }
                                Err(e) => {
                                    tracing::error!(?connection_id, error = %e, "Frame decode error");
                                    // Continue processing other frames
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(?connection_id, error = %e, "Read error");
                        break;
                    }
                }
            }

            tracing::debug!(?connection_id, "Multiplexer reader task terminated");
        });

        Ok(task)
    }

    /// Spawn writer task for outgoing messages
    fn spawn_writer(
        &self,
        mut writer: tokio::net::tcp::OwnedWriteHalf,
        mut outbound_rx: mpsc::UnboundedReceiver<MessageFrame>,
    ) -> Result<JoinHandle<()>> {
        let connection_id = self.connection_id;

        let task = tokio::spawn(async move {
            tracing::debug!(?connection_id, "Multiplexer writer task started");

            while let Some(frame) = outbound_rx.recv().await {
                tracing::debug!(?connection_id, protocol_id = %frame.protocol_id, payload_len = frame.payload.len(), "Sending frame");

                match frame.encode() {
                    Ok(data) => {
                        tracing::debug!(?connection_id, protocol_id = %frame.protocol_id, payload_len = frame.payload.len(),
                            frame_hex = hex::encode(&data[..data.len().min(64)]),
                            "Sending frame (first 64 bytes in hex)");

                        if let Err(e) = writer.write_all(&data).await {
                            tracing::error!(?connection_id, error = %e, "Write error");
                            break;
                        }
                        // Flush to ensure data is sent immediately
                        if let Err(e) = writer.flush().await {
                            tracing::error!(?connection_id, error = %e, "Flush error");
                            break;
                        }
                        tracing::trace!(?connection_id, bytes_written = data.len(), "Frame sent and flushed");
                    }
                    Err(e) => {
                        tracing::error!(?connection_id, error = %e, "Frame encode error");
                        // Continue with next frame
                    }
                }
            }

            tracing::debug!(?connection_id, "Multiplexer writer task terminated");
        });

        Ok(task)
    }

    /// Handle incoming frame
    async fn handle_incoming_frame(
        connection_id: ConnectionId,
        frame: MessageFrame,
        handlers: &Arc<RwLock<HashMap<ProtocolId, Arc<dyn ProtocolHandler>>>>,
    ) {
        let handlers_guard = handlers.read().await;

        if let Some(handler) = handlers_guard.get(&frame.protocol_id) {
            let handler = handler.clone();
            drop(handlers_guard);

            // Handle message in separate task to avoid blocking
            let frame_protocol = frame.protocol_id;
            let frame_payload = frame.payload.clone();

            tokio::spawn(async move {
                match handler.handle_message(connection_id, frame_payload).await {
                    Ok(Some(response)) => {
                        tracing::debug!(
                            protocol = %frame_protocol,
                            response_len = response.len(),
                            "Protocol handler produced response without back-channel"
                        );
                        // TODO: Send response back through multiplexer when back-channel is implemented
                    }
                    Ok(None) => {
                        // No response needed
                    }
                    Err(e) => {
                        eprintln!("Protocol {} handler error: {}", frame_protocol, e);
                    }
                }
            });
        } else {
            eprintln!(
                "No handler registered for protocol {} on connection {:?}",
                frame.protocol_id, connection_id
            );
        }
    }
}

impl Drop for ConnectionMultiplexer {
    fn drop(&mut self) {
        // Abort all tasks when dropped
        for task in self.tasks.drain(..) {
            task.abort();
        }
    }
}

/// Multiplexer configuration
#[derive(Debug, Clone)]
pub struct MultiplexerConfig {
    /// Maximum frame size in bytes
    pub max_frame_size: usize,
    /// Maximum number of queued outbound messages
    pub max_outbound_queue: usize,
    /// Read buffer size
    pub read_buffer_size: usize,
    /// Write buffer size
    pub write_buffer_size: usize,
    /// Enable frame compression
    pub enable_compression: bool,
}

impl Default for MultiplexerConfig {
    fn default() -> Self {
        Self {
            max_frame_size: 64 * 1024, // 64KB
            max_outbound_queue: 1000,
            read_buffer_size: 8192,
            write_buffer_size: 8192,
            enable_compression: false,
        }
    }
}

/// Simple protocol handler implementation for testing
#[derive(Debug)]
pub struct EchoProtocolHandler {
    protocol_id: ProtocolId,
    name: String,
}

impl EchoProtocolHandler {
    pub fn new(protocol_id: ProtocolId, name: String) -> Self {
        Self { protocol_id, name }
    }
}

impl ProtocolHandler for EchoProtocolHandler {
    fn handle_message(
        &self,
        _connection_id: ConnectionId,
        message: Bytes,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Option<Bytes>>> + Send>> {
        let response = message.clone(); // Echo the message back
        Box::pin(async move { Ok(Some(response)) })
    }

    fn protocol_id(&self) -> ProtocolId {
        self.protocol_id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NetworkError;

    #[test]
    fn test_protocol_id_constants() {
        assert_eq!(ProtocolId::HANDSHAKE.value(), 0);
        assert_eq!(ProtocolId::CHAINSYNC.value(), 2);
        assert_eq!(ProtocolId::BLOCKFETCH.value(), 3);
        assert_eq!(ProtocolId::TXSUBMISSION.value(), 4);
        assert_eq!(ProtocolId::KEEPALIVE.value(), 8);
    }

    #[test]
    fn test_message_frame_encoding() {
        let protocol_id = ProtocolId::CHAINSYNC;
        let payload = Bytes::from_static(b"test message");
        let frame = MessageFrame::new(protocol_id, payload.clone());

        let encoded = frame.encode().unwrap();
        let decoded = MessageFrame::decode(encoded).unwrap();

        assert_eq!(decoded.protocol_id, protocol_id);
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn test_frame_size_calculation() {
        let payload = Bytes::from_static(b"hello");
        let frame = MessageFrame::new(ProtocolId::HANDSHAKE, payload);

        // 8 bytes header (u32 timestamp + u16 protocol_id + u16 length) + 5 bytes payload = 13 bytes
        assert_eq!(frame.frame_size(), 13);
    }

    #[test]
    fn test_invalid_frame_decode() {
        let invalid_data = Bytes::from_static(b"xx"); // Too short
        let result = MessageFrame::decode(invalid_data);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            NetworkError::MultiplexerError(MultiplexerError::InvalidFrameSize { .. })
        ));
    }

    #[tokio::test]
    async fn test_echo_protocol_handler() {
        let handler = EchoProtocolHandler::new(ProtocolId::CHAINSYNC, "TestEcho".to_string());

        let message = Bytes::from_static(b"test");
        let response = handler
            .handle_message(ConnectionId::new(), message.clone())
            .await
            .unwrap();

        assert_eq!(response, Some(message));
        assert_eq!(handler.protocol_id(), ProtocolId::CHAINSYNC);
        assert_eq!(handler.name(), "TestEcho");
    }
}
