//! Config tests

use cardano_node::{NetworkTopology, NodeConfiguration};

#[test]
fn test_default_config() {
    let cfg = NodeConfiguration::default_for_testing();
    assert!(cfg.validate().is_ok());
}

#[test]
fn test_default_topology() {
    let topo = NetworkTopology::default_for_testing();
    assert!(topo.validate().is_ok());
}

#[test]
#[ignore] // Run with: cargo test -- --ignored
fn test_real_mainnet_config() {
    if let Ok(cfg) = NodeConfiguration::from_file("/tmp/mainnet-config.json") {
        println!("✅ Successfully parsed official mainnet config!");
        assert_eq!(cfg.protocol.as_deref(), Some("Cardano"));
        assert_eq!(cfg.consensus_mode.as_deref(), Some("PraosMode"));
        assert!(cfg.byron_genesis_file.is_some());
    }
}

#[test]
#[ignore]
fn test_real_mainnet_topology() {
    if let Ok(topo) = NetworkTopology::from_file("/tmp/mainnet-topology.json") {
        println!("✅ Successfully parsed official mainnet topology!");
        assert!(topo.bootstrap_peers.is_some());
        assert!(topo.validate().is_ok());
    }
}
