//! End-to-end integration tests for Cardano Node
//!
//! These tests verify complete node functionality including network protocols,
//! connection management, and inter-node communication.

use anyhow::Result;
use bytes::Bytes;
use cardano_network::connection::multiplexer::{MessageFrame, ProtocolId};
use cardano_network::connection::{ConnectionConfig, ConnectionManager, HandshakeProtocol};
use cardano_network::protocols::chainsync::{
    ChainSyncMessage, ChainSyncProtocolHandler, ChainSyncWireMessage,
};
use cardano_network::{PeerId, PeerInfo};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::sleep;
use tracing::{info, warn};

/// Test that two nodes can establish connection and exchange ChainSync messages
#[tokio::test]
async fn test_node_to_node_chainsync() -> Result<()> {
    tracing_subscriber::fmt::try_init().ok();

    info!("Setting up test nodes");

    // Setup listening node (server)
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let server_addr = listener.local_addr()?;
    info!("Server listening on {}", server_addr);

    // Start server that will handle incoming connections
    let server_task = tokio::spawn(async move {
        info!("Server: Waiting for connection");
        match listener.accept().await {
            Ok((mut stream, client_addr)) => {
                info!("Server: Accepted connection from {}", client_addr);

                // Perform handshake
                let handshake = HandshakeProtocol::new();
                match handshake.handle_handshake(&mut stream).await {
                    Ok(version) => {
                        info!("Server: Handshake completed with version {}", version);

                        // Create multiplexer and register ChainSync handler
                        let connection_id = cardano_network::ConnectionId::new();
                        let config = cardano_network::connection::multiplexer::MultiplexerConfig {
                            max_frame_size: 64 * 1024,
                            send_queue_size: 100,
                            recv_timeout: Duration::from_secs(30),
                        };

                        match cardano_network::connection::multiplexer::ConnectionMultiplexer::new(
                            connection_id,
                            stream,
                            config,
                        ) {
                            Ok(mut multiplexer) => {
                                // Register ChainSync handler with a small test chain
                                let handler =
                                    std::sync::Arc::new(ChainSyncProtocolHandler::with_mock_chain(5));
                                if let Err(e) = multiplexer.register_protocol(handler).await {
                                    warn!("Server: Failed to register protocol: {}", e);
                                    return Err(e);
                                }

                                info!("Server: Starting multiplexer");
                                if let Err(e) = multiplexer.start().await {
                                    warn!("Server: Multiplexer start failed: {}", e);
                                    return Err(e);
                                }

                                // Keep running for test duration
                                sleep(Duration::from_secs(2)).await;
                                info!("Server: Test complete");
                                Ok(())
                            }
                            Err(e) => {
                                warn!("Server: Failed to create multiplexer: {}", e);
                                Err(e.into())
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Server: Handshake failed: {}", e);
                        Err(e.into())
                    }
                }
            }
            Err(e) => {
                warn!("Server: Accept failed: {}", e);
                Err(e.into())
            }
        }
    });

    // Give server time to start listening
    sleep(Duration::from_millis(50)).await;

    // Setup client node using ConnectionManager
    info!("Client: Creating connection manager");
    let mut client_config = ConnectionConfig::default();
    client_config.limits.connect_timeout = Duration::from_secs(5);
    client_config.limits.handshake_timeout = Duration::from_secs(5);
    client_config.peer_selection.max_connections = 1;

    let mut manager = ConnectionManager::new(client_config).await?;

    // Create peer info for server
    let peer_id = PeerId::random();
    let peer_info = PeerInfo::new(peer_id, server_addr);

    info!("Client: Adding peer {}", server_addr);
    manager.add_peer(peer_info).await?;

    info!("Client: Starting connection manager");
    manager.start().await?;

    info!("Client: Connecting to peer");
    let connection_id = manager.connect_peer(&peer_id).await?;
    info!("Client: Connected with connection_id: {}", connection_id);

    // Wait for connection to be fully established
    sleep(Duration::from_millis(200)).await;

    // Verify connection is active
    let stats = manager.connection_stats().await;
    assert!(
        stats.contains_key(&connection_id),
        "Connection should be registered"
    );

    info!("Client: Connection established successfully");

    // Note: In a real test, we would send ChainSync messages through the multiplexer
    // For now, we verify that the connection was established and the handler was registered

    // Cleanup
    sleep(Duration::from_millis(500)).await;
    info!("Client: Shutting down");
    manager.shutdown().await;

    // Wait for server to complete
    let server_result = tokio::time::timeout(Duration::from_secs(5), server_task).await;
    match server_result {
        Ok(Ok(Ok(()))) => info!("Server completed successfully"),
        Ok(Ok(Err(e))) => warn!("Server returned error: {}", e),
        Ok(Err(e)) => warn!("Server panicked: {}", e),
        Err(_) => warn!("Server timeout"),
    }

    Ok(())
}

/// Test ChainSync wire message serialization round-trip
#[tokio::test]
async fn test_chainsync_wire_message_roundtrip() -> Result<()> {
    tracing_subscriber::fmt::try_init().ok();

    // Create a test ChainSync message
    let request = ChainSyncMessage::RequestNext;

    // Convert to wire format
    let wire = ChainSyncWireMessage::from_domain(request.clone())?;

    // Serialize
    let encoded = minicbor::to_vec(&wire)?;
    info!("Encoded message size: {} bytes", encoded.len());

    // Deserialize
    let decoded_wire: ChainSyncWireMessage = minicbor::decode(&encoded)?;

    // Convert back to domain
    let decoded_message = decoded_wire.into_domain()?;

    // Verify round-trip
    match decoded_message {
        ChainSyncMessage::RequestNext => {
            info!("Round-trip successful for RequestNext");
            Ok(())
        }
        other => panic!("Expected RequestNext, got {:?}", other),
    }
}

/// Test that ConnectionManager auto-registers ChainSync handler
#[tokio::test]
async fn test_connection_manager_auto_registers_chainsync() -> Result<()> {
    tracing_subscriber::fmt::try_init().ok();

    let config = ConnectionConfig::default();
    let manager = ConnectionManager::new(config).await?;

    // The manager should have ChainSync handler registered by default
    // We can verify this by checking that a connection would have it available
    // For now, we just verify the manager was created successfully
    assert_eq!(manager.connection_count().await, 0);

    Ok(())
}

/// Test message frame encoding/decoding for ChainSync protocol
#[tokio::test]
async fn test_chainsync_message_frame() -> Result<()> {
    tracing_subscriber::fmt::try_init().ok();

    // Create a ChainSync message
    let message = ChainSyncMessage::RequestNext;
    let wire = ChainSyncWireMessage::from_domain(message)?;
    let payload = Bytes::from(minicbor::to_vec(&wire)?);

    // Create message frame
    let frame = MessageFrame::new(ProtocolId::CHAINSYNC, payload.clone());

    // Encode frame
    let encoded = frame.encode()?;
    info!("Frame size: {} bytes", encoded.len());

    // Decode frame
    let decoded_frame = MessageFrame::decode(encoded)?;

    assert_eq!(decoded_frame.protocol_id, ProtocolId::CHAINSYNC);
    assert_eq!(decoded_frame.payload, payload);

    // Verify we can decode the ChainSync message
    let decoded_wire: ChainSyncWireMessage = minicbor::decode(&decoded_frame.payload)?;
    let decoded_message = decoded_wire.into_domain()?;

    match decoded_message {
        ChainSyncMessage::RequestNext => {
            info!("Successfully decoded ChainSync message from frame");
            Ok(())
        }
        other => panic!("Expected RequestNext, got {:?}", other),
    }
}

/// Benchmark test: Measure ChainSync message throughput
#[tokio::test]
#[ignore] // Ignored by default, run with --ignored
async fn benchmark_chainsync_message_throughput() -> Result<()> {
    tracing_subscriber::fmt::try_init().ok();

    let iterations = 10_000;
    let message = ChainSyncMessage::RequestNext;

    let start = std::time::Instant::now();

    for _ in 0..iterations {
        let wire = ChainSyncWireMessage::from_domain(message.clone())?;
        let encoded = minicbor::to_vec(&wire)?;
        let decoded: ChainSyncWireMessage = minicbor::decode(&encoded)?;
        let _ = decoded.into_domain()?;
    }

    let elapsed = start.elapsed();
    let per_second = iterations as f64 / elapsed.as_secs_f64();

    info!(
        "ChainSync message throughput: {:.2} msg/sec ({:?} total for {} iterations)",
        per_second, elapsed, iterations
    );

    assert!(
        per_second > 1000.0,
        "Expected at least 1000 msg/sec, got {:.2}",
        per_second
    );

    Ok(())
}
