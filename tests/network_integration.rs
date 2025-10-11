//! Network integration tests module
//!
//! End-to-end tests for network functionality

pub mod integration;
pub mod network;

// Handshake integration tests
use bytes::Bytes;
use cardano_network::protocols::handshake::{
    HandshakeProtocolHandler, NetworkMagic,
    codec::{decode_message, encode_message},
    messages::HandshakeMessage,
    types::{NodeToNodeVersion, NodeToNodeVersionData},
};
use cardano_network::connection::ConnectionId;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Mock server that responds to handshake
async fn mock_handshake_server(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Read MsgProposeVersions
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;

    if n == 0 {
        return Err("Connection closed".into());
    }

    let received = Bytes::from(buf[..n].to_vec());

    // Decode the proposal
    let _proposal = decode_message(&received)?;

    // Send MsgAcceptVersion with V15
    let response = HandshakeMessage::accept_version(
        NodeToNodeVersion::V15,
        NodeToNodeVersionData::preview_testnet(),
    );

    let encoded = encode_message(&response)?;
    stream.write_all(&encoded).await?;
    stream.flush().await?;

    Ok(())
}

#[tokio::test]
#[ignore] // Ignore by default, run with --ignored
async fn test_handshake_with_mock_server() {
    // Start mock server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server task
    let server_handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        mock_handshake_server(stream).await
    });

    // Give server time to start
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Create handshake handler
    let handler = HandshakeProtocolHandler::new(NetworkMagic::PREVIEW_TESTNET);
    let conn_id = ConnectionId::new();

    // Connect to mock server
    let mut client_stream = TcpStream::connect(addr).await.unwrap();

    // Start handshake
    let proposal = handler.start(conn_id).await.unwrap();

    client_stream.write_all(&proposal).await.unwrap();
    client_stream.flush().await.unwrap();

    // Read response
    let mut buf = vec![0u8; 4096];
    let n = client_stream.read(&mut buf).await.unwrap();
    let response = Bytes::from(buf[..n].to_vec());

    // Handle response
    let result = handler.handle_message(conn_id, response).await;
    assert!(result.is_ok(), "Handshake should succeed");

    // Check handshake completed
    assert!(handler.is_done(conn_id).await, "Handshake should be done");
    assert!(!handler.is_failed(conn_id).await, "Handshake should not fail");

    // Get result
    let handshake_result = handler.result(conn_id).await;
    assert!(handshake_result.is_some(), "Should have result");

    let result = handshake_result.unwrap();
    assert_eq!(result.version, NodeToNodeVersion::V15);
    assert!(result.is_preview_testnet());

    // Wait for server
    let _ = server_handle.await;
}

#[tokio::test]
async fn test_handshake_version_negotiation() {
    let handler = HandshakeProtocolHandler::new(NetworkMagic::PREVIEW_TESTNET);
    let conn_id = ConnectionId::new();

    // Start handshake
    let proposal = handler.start(conn_id).await.unwrap();

    // Decode to verify it contains V14 and V15
    let msg = decode_message(&proposal).unwrap();

    if let HandshakeMessage::ProposeVersions { versions } = msg {
        assert!(versions.contains_key(&NodeToNodeVersion::V14));
        assert!(versions.contains_key(&NodeToNodeVersion::V15));
    } else {
        panic!("Expected ProposeVersions message");
    }
}

#[tokio::test]
async fn test_handshake_network_magic_mismatch() {
    let handler = HandshakeProtocolHandler::new(NetworkMagic::PREVIEW_TESTNET);
    let conn_id = ConnectionId::new();

    // Start handshake
    let _proposal = handler.start(conn_id).await.unwrap();

    // Create response with wrong network magic (mainnet instead of preview)
    let wrong_response = HandshakeMessage::accept_version(
        NodeToNodeVersion::V15,
        NodeToNodeVersionData::mainnet(), // Wrong network!
    );

    let encoded = encode_message(&wrong_response).unwrap();

    // Handle response - should detect mismatch
    let result = handler.handle_message(conn_id, encoded).await;

    // Should fail or be marked as failed
    if result.is_ok() {
        assert!(handler.is_failed(conn_id).await, "Should fail due to network mismatch");
    }
}

#[test]
fn test_network_magic_constants() {
    assert_eq!(NetworkMagic::MAINNET.0, 764824073);
    assert_eq!(NetworkMagic::PREVIEW_TESTNET.0, 1097911063);
    assert_eq!(NetworkMagic::PREPROD_TESTNET.0, 1);
}
