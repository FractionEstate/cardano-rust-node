//! Preview Network Discovery Integration Test
//!
//! Tests actual DNS resolution of preview network peers from topology configuration.

use cardano_network::discovery::PeerDiscovery;
use cardano_network::topology::TopologyConfig;

#[tokio::test]
#[ignore] // Requires network access
async fn test_preview_topology_discovery() {
    // Load the actual preview topology config
    let topology = TopologyConfig::from_file("config/preview-topology.json")
        .await
        .expect("Failed to load preview topology");

    println!("Loaded topology:");
    println!("  Local roots: {}", topology.local_roots.len());
    println!("  Public roots: {}", topology.public_roots.len());
    println!("  Valency: {}", topology.total_valency());

    // Create discovery service
    let discovery = PeerDiscovery::new(topology);

    // Discover all peers
    let peers = discovery
        .discover_all()
        .await
        .expect("Failed to discover peers");

    println!("\nDiscovered {} peers:", peers.len());
    for peer in &peers {
        println!(
            "  {} -> {} (trustable: {})",
            peer.access_point.to_string(),
            peer.socket_addr,
            peer.trustable
        );
    }

    // Verify we got some peers
    assert!(!peers.is_empty(), "Should discover at least one peer");

    // Verify we have both IPv4 and potentially IPv6
    let (ipv4_peers, ipv6_peers) = PeerDiscovery::separate_by_ip_version(&peers);
    println!("\nIPv4 peers: {}", ipv4_peers.len());
    println!("IPv6 peers: {}", ipv6_peers.len());

    // Should have at least IPv4 peers
    assert!(!ipv4_peers.is_empty(), "Should have IPv4 peers");

    // Get unique addresses
    let unique = PeerDiscovery::unique_addresses(&peers);
    println!("\nUnique addresses: {}", unique.len());
    assert!(!unique.is_empty());
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_local_peers_discovery() {
    let topology = TopologyConfig::from_file("config/preview-topology.json")
        .await
        .expect("Failed to load preview topology");

    let discovery = PeerDiscovery::new(topology);

    // Discover only local trusted peers
    let local_peers = discovery
        .discover_local()
        .await
        .expect("Failed to discover local peers");

    println!("\nLocal trusted peers: {}", local_peers.len());
    for peer in &local_peers {
        println!(
            "  {} -> {} (trustable: {})",
            peer.access_point.to_string(),
            peer.socket_addr,
            peer.trustable
        );
    }

    // All local peers should be trustable
    assert!(
        local_peers.iter().all(|p| p.trustable),
        "All local peers should be trustable"
    );
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_public_relays_discovery() {
    let topology = TopologyConfig::from_file("config/preview-topology.json")
        .await
        .expect("Failed to load preview topology");

    let discovery = PeerDiscovery::new(topology);

    // Discover only public relays
    let public_peers = discovery
        .discover_public()
        .await
        .expect("Failed to discover public peers");

    println!("\nPublic relay peers: {}", public_peers.len());
    for peer in &public_peers {
        println!(
            "  {} -> {}",
            peer.access_point.to_string(),
            peer.socket_addr
        );
    }

    // Should have at least one public relay
    assert!(!public_peers.is_empty(), "Should have public relay peers");
}

#[tokio::test]
async fn test_topology_parsing() {
    // Test parsing the actual file
    let topology = TopologyConfig::from_file("config/preview-topology.json")
        .await
        .expect("Failed to load preview topology");

    // Verify structure matches expected preview config
    assert_eq!(
        topology.local_roots.len(),
        1,
        "Preview should have 1 local root group"
    );
    assert_eq!(
        topology.public_roots.len(),
        1,
        "Preview should have 1 public root group"
    );

    // Check local roots
    let local_root = &topology.local_roots[0];
    assert_eq!(
        local_root.access_points.len(),
        2,
        "Local root should have 2 access points"
    );
    assert!(local_root.trustable, "Local root should be trustable");
    assert_eq!(local_root.valency, 2, "Local root valency should be 2");

    // Check public roots
    let public_root = &topology.public_roots[0];
    assert_eq!(
        public_root.access_points.len(),
        1,
        "Public root should have 1 access point"
    );

    // Verify specific addresses
    let local_addrs: Vec<String> = local_root
        .access_points
        .iter()
        .map(|ap| ap.address.clone())
        .collect();

    assert!(
        local_addrs.contains(&"preview-node.world.dev.cardano.org".to_string()),
        "Should have preview-node.world.dev.cardano.org"
    );
    assert!(
        local_addrs.contains(&"preview-node.play.dev.cardano.org".to_string()),
        "Should have preview-node.play.dev.cardano.org"
    );
}
