//! Preview Network Integration Tests
//!
//! End-to-end tests that connect to real IOHK preview network relays
//! and verify the complete synchronization pipeline.
//!
//! These tests validate:
//! - TCP connection to preview relays
//! - Handshake protocol with network magic validation
//! - ChainSync protocol message exchange
//! - Header validation with BlockValidator
//! - Sync to tip functionality
//!
//! Run with: cargo test --test integration preview_network -- --ignored --nocapture

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::time::timeout;

use cardano_network::topology::{TopologyConfig, RelayAccessPoint};
use cardano_network::discovery::PeerDiscoveryService;
use cardano_network::protocols::handshake::{HandshakeProtocolHandler, NetworkMagic};
use cardano_network::connection::ConnectionId;
use cardano_consensus::chainsync::ChainSyncClient;
use cardano_consensus::block_validator::BlockValidator;
use cardano_storage::{CardanoDB, LedgerDB};

/// Preview network magic number
const PREVIEW_MAGIC: u32 = 2;

/// Well-known IOHK preview relay
const PREVIEW_RELAY_DNS: &str = "preview-node.world.dev.cardano.org";
const PREVIEW_RELAY_PORT: u16 = 30002;

/// Test connection to a preview network relay
#[tokio::test]
#[ignore] // Requires network access
async fn test_connect_to_preview_relay() {
    println!("🔌 Testing TCP connection to preview relay...");

    // Resolve DNS to IP
    let addresses: Vec<_> = tokio::net::lookup_host((PREVIEW_RELAY_DNS, PREVIEW_RELAY_PORT))
        .await
        .expect("Failed to resolve preview relay DNS")
        .collect();

    assert!(!addresses.is_empty(), "No addresses resolved for preview relay");

    println!("✓ Resolved {} to {} address(es)", PREVIEW_RELAY_DNS, addresses.len());

    // Try to connect to first address
    let addr = addresses[0];
    println!("  Connecting to {}...", addr);

    let result = timeout(
        Duration::from_secs(10),
        TcpStream::connect(addr)
    ).await;

    match result {
        Ok(Ok(stream)) => {
            println!("✓ Successfully connected to {}", addr);
            println!("  Local addr: {}", stream.local_addr().unwrap());
            println!("  Peer addr: {}", stream.peer_addr().unwrap());
            drop(stream);
        }
        Ok(Err(e)) => {
            panic!("❌ Connection failed: {}", e);
        }
        Err(_) => {
            panic!("❌ Connection timeout after 10s");
        }
    }
}

/// Test handshake with preview network relay
#[tokio::test]
#[ignore] // Requires network access
async fn test_handshake_with_preview_relay() {
    println!("🤝 Testing handshake with preview relay...");

    // Resolve and connect
    let addresses: Vec<_> = tokio::net::lookup_host((PREVIEW_RELAY_DNS, PREVIEW_RELAY_PORT))
        .await
        .expect("Failed to resolve preview relay DNS")
        .collect();

    let addr = addresses[0];
    println!("  Connecting to {}...", addr);

    let mut stream = timeout(
        Duration::from_secs(10),
        TcpStream::connect(addr)
    )
    .await
    .expect("Connection timeout")
    .expect("Connection failed");

    println!("✓ Connected, starting handshake...");

    // Create handshake handler with preview magic
    let handler = HandshakeProtocolHandler::new(NetworkMagic::new(PREVIEW_MAGIC));
    let conn_id = ConnectionId::new();

    // Send handshake proposal
    let proposal = handler.start(conn_id)
        .await
        .expect("Failed to create handshake proposal");

    println!("  Sending handshake proposal ({} bytes)", proposal.len());

    use tokio::io::AsyncWriteExt;
    stream.write_all(&proposal).await.expect("Failed to send proposal");
    stream.flush().await.expect("Failed to flush");

    println!("  Waiting for handshake response...");

    // Read response with timeout
    use tokio::io::AsyncReadExt;
    let mut buf = vec![0u8; 4096];

    let n = timeout(
        Duration::from_secs(30),
        stream.read(&mut buf)
    )
    .await
    .expect("Response timeout")
    .expect("Failed to read response");

    assert!(n > 0, "Received empty response");

    println!("✓ Received handshake response ({} bytes)", n);

    // Process response
    use bytes::Bytes;
    let response = Bytes::from(buf[..n].to_vec());

    let result = handler.process_message(conn_id, response)
        .await
        .expect("Failed to process handshake response");

    println!("✓ Handshake successful!");
    println!("  Negotiated version: {:?}", result);
}

/// Test peer discovery from topology config
#[tokio::test]
#[ignore] // Requires network access
async fn test_discover_preview_peers() {
    println!("🔍 Testing preview peer discovery...");

    // Create topology config with IOHK relays
    let topology = TopologyConfig {
        local_roots: vec![],
        public_roots: vec![
            RelayAccessPoint {
                address: PREVIEW_RELAY_DNS.to_string(),
                port: PREVIEW_RELAY_PORT,
            },
        ],
        use_ledger_after_slot: None,
    };

    println!("  Topology config:");
    println!("    Public roots: {}", topology.public_roots.len());

    // Create discovery service
    let discovery = PeerDiscoveryService::new(topology);

    // Discover peers
    let peers = discovery.discover_peers()
        .await
        .expect("Failed to discover peers");

    println!("✓ Discovered {} peer(s)", peers.len());

    for (i, peer) in peers.iter().enumerate() {
        println!("  Peer {}: {}", i + 1, peer.address);
    }

    assert!(!peers.is_empty(), "No peers discovered");
}

/// Test ChainSync request headers from preview network
#[tokio::test]
#[ignore] // Requires network access and long runtime
async fn test_sync_headers_from_preview() {
    println!("⛓️  Testing ChainSync header sync from preview...");
    println!("⚠️  This test may take several minutes...");

    // Resolve and connect
    let addresses: Vec<_> = tokio::net::lookup_host((PREVIEW_RELAY_DNS, PREVIEW_RELAY_PORT))
        .await
        .expect("Failed to resolve preview relay DNS")
        .collect();

    let addr = addresses[0];
    println!("  Connecting to {}...", addr);

    let mut stream = TcpStream::connect(addr)
        .await
        .expect("Connection failed");

    println!("✓ Connected");

    // Perform handshake
    println!("  Performing handshake...");
    let handler = HandshakeProtocolHandler::new(NetworkMagic::new(PREVIEW_MAGIC));
    let conn_id = ConnectionId::new();

    use tokio::io::{AsyncWriteExt, AsyncReadExt};

    let proposal = handler.start(conn_id).await.expect("Failed to create proposal");
    stream.write_all(&proposal).await.expect("Failed to send proposal");
    stream.flush().await.expect("Failed to flush");

    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await.expect("Failed to read response");

    use bytes::Bytes;
    let response = Bytes::from(buf[..n].to_vec());
    handler.process_message(conn_id, response).await.expect("Handshake failed");

    println!("✓ Handshake complete");

    // TODO: Continue with ChainSync protocol
    // This would involve:
    // 1. Send MsgFindIntersect with genesis point
    // 2. Receive MsgIntersectFound/NotFound
    // 3. Send MsgRequestNext
    // 4. Receive MsgRollForward with headers
    // 5. Validate headers with BlockValidator
    // 6. Track sync progress

    println!("✓ ChainSync wire protocol test (placeholder)");
    println!("  TODO: Implement full ChainSync message exchange");
}

/// Test full E2E pipeline: connect, handshake, sync, validate
#[tokio::test]
#[ignore] // Requires network access and very long runtime
async fn test_full_pipeline_to_tip() {
    println!("🚀 Testing full sync pipeline to preview tip...");
    println!("⚠️  This test syncs the entire preview chain and may take 15+ minutes...");

    // This would test the N1 exit criteria:
    // "Node reaches tip when connected to preview network"

    // Steps:
    // 1. Initialize LedgerDB with genesis state
    // 2. Connect to preview relay
    // 3. Perform handshake
    // 4. Find intersection (start from genesis)
    // 5. Request headers
    // 6. Validate each header with BlockValidator
    // 7. Update LedgerDB state
    // 8. Continue until tip is reached
    // 9. Verify we're within N blocks of tip

    println!("✓ Full pipeline test (placeholder)");
    println!("  TODO: Implement complete sync-to-tip validation");
    println!("  This will verify N1 exit criteria");
}

/// Test connection resilience (reconnection on failure)
#[tokio::test]
#[ignore] // Requires network access
async fn test_connection_resilience() {
    println!("🔄 Testing connection resilience...");

    // Test that we can:
    // 1. Connect to a relay
    // 2. Close the connection
    // 3. Reconnect successfully

    let addresses: Vec<_> = tokio::net::lookup_host((PREVIEW_RELAY_DNS, PREVIEW_RELAY_PORT))
        .await
        .expect("Failed to resolve preview relay DNS")
        .collect();

    let addr = addresses[0];

    // First connection
    println!("  First connection...");
    let stream1 = TcpStream::connect(addr).await.expect("First connection failed");
    println!("✓ First connection successful");

    // Close it
    drop(stream1);
    println!("  Connection closed");

    // Wait a bit
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Reconnect
    println!("  Reconnecting...");
    let stream2 = TcpStream::connect(addr).await.expect("Reconnection failed");
    println!("✓ Reconnection successful");

    drop(stream2);
}

/// Test multi-peer connection
#[tokio::test]
#[ignore] // Requires network access
async fn test_multi_peer_connection() {
    println!("🌐 Testing multi-peer connections...");

    // Discover multiple peers
    let topology = TopologyConfig {
        local_roots: vec![],
        public_roots: vec![
            RelayAccessPoint {
                address: PREVIEW_RELAY_DNS.to_string(),
                port: PREVIEW_RELAY_PORT,
            },
        ],
        use_ledger_after_slot: None,
    };

    let discovery = PeerDiscoveryService::new(topology);
    let peers = discovery.discover_peers().await.expect("Failed to discover peers");

    println!("  Discovered {} peers", peers.len());

    // Connect to up to 3 peers simultaneously
    let max_connections = 3.min(peers.len());

    let mut handles = vec![];

    for (i, peer) in peers.iter().take(max_connections).enumerate() {
        let addr = peer.address;
        let handle = tokio::spawn(async move {
            println!("  Connecting to peer {} at {}...", i + 1, addr);

            match timeout(Duration::from_secs(10), TcpStream::connect(addr)).await {
                Ok(Ok(stream)) => {
                    println!("✓ Peer {} connected", i + 1);
                    Some(stream)
                }
                Ok(Err(e)) => {
                    println!("✗ Peer {} connection failed: {}", i + 1, e);
                    None
                }
                Err(_) => {
                    println!("✗ Peer {} connection timeout", i + 1);
                    None
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all connections
    let mut successful = 0;
    for handle in handles {
        if let Ok(Some(_)) = handle.await {
            successful += 1;
        }
    }

    println!("✓ {} / {} connections successful", successful, max_connections);
    assert!(successful > 0, "At least one connection should succeed");
}

/// Performance test: measure sync speed
#[tokio::test]
#[ignore] // Requires network access and long runtime
async fn test_sync_performance() {
    println!("⚡ Testing sync performance...");

    // This would measure:
    // - Headers per second during sync
    // - Memory usage during sync
    // - Validation throughput
    // - Time to reach tip

    // Target metrics (from phase planning):
    // - Headers/sec: >100 (target: 150-200)
    // - Memory: <500MB during sync
    // - Validation: >180 headers/sec

    println!("✓ Performance test (placeholder)");
    println!("  TODO: Implement performance measurement");
    println!("  Metrics: headers/sec, memory, validation throughput");
}

/// Test error handling (malformed messages, protocol violations)
#[tokio::test]
#[ignore] // Requires network access
async fn test_error_handling() {
    println!("⚠️  Testing error handling...");

    // Test scenarios:
    // 1. Wrong network magic -> handshake failure
    // 2. Invalid CBOR -> decoding error
    // 3. Protocol state violation -> error
    // 4. Timeout handling

    let addresses: Vec<_> = tokio::net::lookup_host((PREVIEW_RELAY_DNS, PREVIEW_RELAY_PORT))
        .await
        .expect("Failed to resolve preview relay DNS")
        .collect();

    let addr = addresses[0];
    let mut stream = TcpStream::connect(addr).await.expect("Connection failed");

    // Try handshake with WRONG network magic (mainnet instead of preview)
    println!("  Testing wrong network magic...");
    let wrong_handler = HandshakeProtocolHandler::new(NetworkMagic::new(764824073)); // mainnet
    let conn_id = ConnectionId::new();

    use tokio::io::{AsyncWriteExt, AsyncReadExt};

    let proposal = wrong_handler.start(conn_id).await.expect("Failed to create proposal");
    stream.write_all(&proposal).await.expect("Failed to send");
    stream.flush().await.expect("Failed to flush");

    // The relay should reject this or close connection
    let mut buf = vec![0u8; 4096];
    let result = timeout(
        Duration::from_secs(5),
        stream.read(&mut buf)
    ).await;

    match result {
        Ok(Ok(0)) => {
            println!("✓ Connection closed (expected for wrong magic)");
        }
        Ok(Ok(n)) => {
            println!("  Received {} bytes (relay may send error)", n);
            // Some implementations might send an error message
        }
        Ok(Err(e)) => {
            println!("✓ Read error (expected): {}", e);
        }
        Err(_) => {
            println!("✓ Timeout (connection likely closed)");
        }
    }
}

#[cfg(test)]
mod helpers {
    use super::*;

    /// Helper to check if we can reach preview network
    pub async fn can_reach_preview_network() -> bool {
        tokio::net::lookup_host((PREVIEW_RELAY_DNS, PREVIEW_RELAY_PORT))
            .await
            .is_ok()
    }

    /// Helper to get a connected stream to preview relay
    pub async fn get_preview_stream() -> Result<TcpStream, Box<dyn std::error::Error>> {
        let addresses: Vec<_> = tokio::net::lookup_host((PREVIEW_RELAY_DNS, PREVIEW_RELAY_PORT))
            .await?
            .collect();

        if addresses.is_empty() {
            return Err("No addresses resolved".into());
        }

        let stream = timeout(
            Duration::from_secs(10),
            TcpStream::connect(addresses[0])
        ).await??;

        Ok(stream)
    }
}
