#[cfg(test)]
mod tests {
    use cardano_node::{NetworkTopology, NodeConfiguration};

    #[test]
    fn test_parse_mainnet_config() {
        // Download files first:
        // curl -o /tmp/mainnet-config.json https://book.world.dev.cardano.org/environments/mainnet/config.json
        // curl -o /tmp/mainnet-topology.json https://book.world.dev.cardano.org/environments/mainnet/topology.json

        if std::path::Path::new("/tmp/mainnet-config.json").exists() {
            let config = NodeConfiguration::from_file("/tmp/mainnet-config.json")
                .expect("Should parse mainnet config");

            assert_eq!(config.protocol.as_deref(), Some("Cardano"));
            assert_eq!(config.consensus_mode.as_deref(), Some("PraosMode"));
            assert_eq!(config.enable_p2_p, Some(true));
            assert!(config.byron_genesis_file.is_some());
            assert!(config.shelley_genesis_file.is_some());
            assert!(config.alonzo_genesis_file.is_some());
            assert!(config.conway_genesis_file.is_some());

            println!("✅ Successfully parsed official mainnet config.json");
        }
    }

    #[test]
    fn test_parse_mainnet_topology() {
        if std::path::Path::new("/tmp/mainnet-topology.json").exists() {
            let topology = NetworkTopology::from_file("/tmp/mainnet-topology.json")
                .expect("Should parse mainnet topology");

            assert!(topology.bootstrap_peers.is_some());
            assert!(topology.local_roots.is_some());
            assert!(topology.public_roots.is_some());
            assert!(topology.use_ledger_after_slot.is_some());

            topology
                .validate()
                .expect("Should validate mainnet topology");

            println!("✅ Successfully parsed official mainnet topology.json");
        }
    }
}
