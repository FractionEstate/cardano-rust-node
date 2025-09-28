//! Configuration Loading Tests for T081
//!
//! Comprehensive tests for configuration file loading, parsing, validation, and error handling.

use cardano_node::{NodeConfiguration, NetworkTopology};
use serde_json::json;
use std::fs;
use tempfile::TempDir;

/// Helper for creating test configurations
struct ConfigTestHelper {
    temp_dir: TempDir,
    config_file: std::path::PathBuf,
    topology_file: std::path::PathBuf,
}

impl ConfigTestHelper {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config_file = temp_dir.path().join("config.json");
        let topology_file = temp_dir.path().join("topology.json");

        Ok(ConfigTestHelper {
            temp_dir,
            config_file,
            topology_file,
        })
    }

    /// Create a valid configuration file
    fn create_valid_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = json!({
            "network_magic": 764824073,
            "listening_port": 3001,
            "database_path": "/tmp/cardano-db",
            "socket_path": "/tmp/cardano-node.socket",
            "enable_logging": true,
            "max_connections": 100,
            "enable_metrics": true,
            "metrics_port": 12798,
            "topology_file": self.topology_file.to_string_lossy()
        });

        fs::write(&self.config_file, config.to_string())?;
        Ok(())
    }

    /// Create an invalid configuration file (missing required fields)
    fn create_invalid_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = json!({
            "network_magic": 764824073
            // Missing required fields
        });

        fs::write(&self.config_file, config.to_string())?;
        Ok(())
    }

    /// Create malformed JSON configuration
    fn create_malformed_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let malformed_json = r#"{ "network_magic": invalid_json }"#;
        fs::write(&self.config_file, malformed_json)?;
        Ok(())
    }

    /// Create a valid topology file
    fn create_valid_topology(&self) -> Result<(), Box<dyn std::error::Error>> {
        let topology = json!({
            "producers": [
                {
                    "addr": "relays-new.cardano-mainnet.iohk.io",
                    "port": 3001,
                    "valency": 2
                },
                {
                    "addr": "relays.cardano-mainnet.iohk.io",
                    "port": 3001,
                    "valency": 1
                }
            ]
        });

        fs::write(&self.topology_file, topology.to_string())?;
        Ok(())
    }

    /// Create invalid topology file (empty producers)
    fn create_invalid_topology(&self) -> Result<(), Box<dyn std::error::Error>> {
        let topology = json!({
            "producers": []
        });

        fs::write(&self.topology_file, topology.to_string())?;
        Ok(())
    }

    /// Create configuration with invalid port
    fn create_invalid_port_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = json!({
            "network_magic": 764824073,
            "listening_port": 0, // Invalid port
            "database_path": "/tmp/cardano-db",
            "socket_path": "/tmp/cardano-node.socket",
            "enable_logging": true,
            "max_connections": 100,
            "enable_metrics": true,
            "metrics_port": 12798,
            "topology_file": "/tmp/topology.json"
        });

        fs::write(&self.config_file, config.to_string())?;
        Ok(())
    }

    /// Create configuration with zero max_connections
    fn create_zero_connections_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = json!({
            "network_magic": 764824073,
            "listening_port": 3001,
            "database_path": "/tmp/cardano-db",
            "socket_path": "/tmp/cardano-node.socket",
            "enable_logging": true,
            "max_connections": 0, // Invalid - must be > 0
            "enable_metrics": true,
            "metrics_port": 12798,
            "topology_file": "/tmp/topology.json"
        });

        fs::write(&self.config_file, config.to_string())?;
        Ok(())
    }
}

// Basic Configuration Loading Tests

#[test]
fn test_load_valid_config_success() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");
    helper.create_valid_config().expect("Failed to create valid config");

    let config = NodeConfiguration::from_file(&helper.config_file);
    assert!(config.is_ok(), "Loading valid config should succeed");

    let config = config.unwrap();
    assert_eq!(config.network_magic, 764824073);
    assert_eq!(config.listening_port, 3001);
    assert_eq!(config.max_connections, 100);
    assert!(config.enable_logging);
    assert!(config.enable_metrics);
    assert_eq!(config.metrics_port, 12798);
}

#[test]
fn test_load_missing_config_file_fails() {
    let non_existent_path = "/path/that/does/not/exist/config.json";
    let result = NodeConfiguration::from_file(non_existent_path);
    assert!(result.is_err(), "Loading non-existent config file should fail");
}

#[test]
fn test_load_malformed_json_fails() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");
    helper.create_malformed_config().expect("Failed to create malformed config");

    let result = NodeConfiguration::from_file(&helper.config_file);
    assert!(result.is_err(), "Loading malformed JSON should fail");
}

#[test]
fn test_load_incomplete_config_fails() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");
    helper.create_invalid_config().expect("Failed to create invalid config");

    let result = NodeConfiguration::from_file(&helper.config_file);
    assert!(result.is_err(), "Loading incomplete config should fail");
}

// Configuration Validation Tests

#[test]
fn test_validate_valid_config_success() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");
    helper.create_valid_config().expect("Failed to create valid config");
    helper.create_valid_topology().expect("Failed to create valid topology");

    let config = NodeConfiguration::from_file(&helper.config_file).expect("Failed to load config");
    let validation_result = config.validate();

    assert!(validation_result.is_ok(), "Valid config validation should succeed");
}

#[test]
fn test_validate_invalid_port_fails() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");
    helper.create_invalid_port_config().expect("Failed to create invalid port config");

    let config = NodeConfiguration::from_file(&helper.config_file).expect("Failed to load config");
    let validation_result = config.validate();

    assert!(validation_result.is_err(), "Config with invalid port should fail validation");
    let error_message = validation_result.unwrap_err().to_string();
    assert!(error_message.contains("Invalid listening port"), "Error should mention invalid port");
}

#[test]
fn test_validate_zero_connections_fails() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");
    helper.create_zero_connections_config().expect("Failed to create zero connections config");

    let config = NodeConfiguration::from_file(&helper.config_file).expect("Failed to load config");
    let validation_result = config.validate();

    assert!(validation_result.is_err(), "Config with zero max_connections should fail validation");
    let error_message = validation_result.unwrap_err().to_string();
    assert!(error_message.contains("max_connections must be greater than 0"), "Error should mention max_connections");
}

// Network Topology Tests

#[test]
fn test_load_valid_topology_success() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");
    helper.create_valid_topology().expect("Failed to create valid topology");

    let topology = NetworkTopology::from_file(&helper.topology_file);
    assert!(topology.is_ok(), "Loading valid topology should succeed");

    let topology = topology.unwrap();
    assert_eq!(topology.producers.len(), 2);
    assert_eq!(topology.producers[0].addr, "relays-new.cardano-mainnet.iohk.io");
    assert_eq!(topology.producers[0].port, 3001);
    assert_eq!(topology.producers[0].valency, 2);
}

#[test]
fn test_load_missing_topology_fails() {
    let non_existent_path = "/path/that/does/not/exist/topology.json";
    let result = NetworkTopology::from_file(non_existent_path);
    assert!(result.is_err(), "Loading non-existent topology file should fail");
}

#[test]
fn test_validate_valid_topology_success() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");
    helper.create_valid_topology().expect("Failed to create valid topology");

    let topology = NetworkTopology::from_file(&helper.topology_file).expect("Failed to load topology");
    let validation_result = topology.validate();

    assert!(validation_result.is_ok(), "Valid topology validation should succeed");
}

#[test]
fn test_validate_empty_topology_fails() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");
    helper.create_invalid_topology().expect("Failed to create invalid topology");

    let topology = NetworkTopology::from_file(&helper.topology_file).expect("Failed to load topology");
    let validation_result = topology.validate();

    assert!(validation_result.is_err(), "Empty topology should fail validation");
    let error_message = validation_result.unwrap_err().to_string();
    assert!(error_message.contains("at least one producer"), "Error should mention producers requirement");
}

// File Format Support Tests

#[test]
fn test_json_config_file_extension() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");
    let json_config_path = helper.temp_dir.path().join("config.json");

    let config = json!({
        "network_magic": 764824073,
        "listening_port": 3001,
        "database_path": "/tmp/cardano-db",
        "socket_path": "/tmp/cardano-node.socket",
        "enable_logging": true,
        "max_connections": 100,
        "enable_metrics": true,
        "metrics_port": 12798,
        "topology_file": "/tmp/topology.json"
    });

    fs::write(&json_config_path, config.to_string()).expect("Failed to write JSON config");

    let loaded_config = NodeConfiguration::from_file(&json_config_path);
    assert!(loaded_config.is_ok(), "JSON config loading should succeed");

    let loaded_config = loaded_config.unwrap();
    assert_eq!(loaded_config.network_magic, 764824073);
    assert_eq!(loaded_config.listening_port, 3001);
}

#[test]
fn test_yaml_config_file_extension() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");
    let yaml_config_path = helper.temp_dir.path().join("config.yaml");

    // For now, use JSON format since YAML isn't fully implemented
    let config = json!({
        "network_magic": 764824073,
        "listening_port": 3001,
        "database_path": "/tmp/cardano-db",
        "socket_path": "/tmp/cardano-node.socket",
        "enable_logging": true,
        "max_connections": 100,
        "enable_metrics": true,
        "metrics_port": 12798,
        "topology_file": "/tmp/topology.json"
    });

    fs::write(&yaml_config_path, config.to_string()).expect("Failed to write YAML config");

    let loaded_config = NodeConfiguration::from_file(&yaml_config_path);
    assert!(loaded_config.is_ok(), "YAML config loading should succeed");
}

// Edge Case and Error Handling Tests

#[test]
fn test_empty_config_file() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");
    fs::write(&helper.config_file, "").expect("Failed to write empty config");

    let result = NodeConfiguration::from_file(&helper.config_file);
    assert!(result.is_err(), "Loading empty config file should fail");
}

#[test]
fn test_config_with_extra_fields() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");

    let config = json!({
        "network_magic": 764824073,
        "listening_port": 3001,
        "database_path": "/tmp/cardano-db",
        "socket_path": "/tmp/cardano-node.socket",
        "enable_logging": true,
        "max_connections": 100,
        "enable_metrics": true,
        "metrics_port": 12798,
        "topology_file": "/tmp/topology.json",
        "extra_field": "should be ignored",
        "another_extra": 42
    });

    fs::write(&helper.config_file, config.to_string()).expect("Failed to write config with extra fields");

    let result = NodeConfiguration::from_file(&helper.config_file);
    assert!(result.is_ok(), "Config with extra fields should still load successfully");
}

#[test]
fn test_config_with_different_types() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");

    let config = json!({
        "network_magic": "764824073", // String instead of number
        "listening_port": 3001,
        "database_path": "/tmp/cardano-db",
        "socket_path": "/tmp/cardano-node.socket",
        "enable_logging": true,
        "max_connections": 100,
        "enable_metrics": true,
        "metrics_port": 12798,
        "topology_file": "/tmp/topology.json"
    });

    fs::write(&helper.config_file, config.to_string()).expect("Failed to write config with wrong types");

    let result = NodeConfiguration::from_file(&helper.config_file);
    assert!(result.is_err(), "Config with wrong field types should fail");
}

// Environment-specific Configuration Tests

#[test]
fn test_mainnet_configuration() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");

    let config = json!({
        "network_magic": 764824073, // Mainnet magic
        "listening_port": 3001,
        "database_path": "/opt/cardano/data",
        "socket_path": "/opt/cardano/cardano-node.socket",
        "enable_logging": true,
        "max_connections": 200,
        "enable_metrics": true,
        "metrics_port": 12798,
        "topology_file": "/opt/cardano/topology.json"
    });

    fs::write(&helper.config_file, config.to_string()).expect("Failed to write mainnet config");

    let loaded_config = NodeConfiguration::from_file(&helper.config_file).expect("Failed to load mainnet config");
    assert_eq!(loaded_config.network_magic, 764824073);
    assert_eq!(loaded_config.max_connections, 200);
}

#[test]
fn test_testnet_configuration() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");

    let config = json!({
        "network_magic": 1097911063, // Testnet magic
        "listening_port": 3001,
        "database_path": "/tmp/cardano-testnet-db",
        "socket_path": "/tmp/cardano-testnet.socket",
        "enable_logging": true,
        "max_connections": 50,
        "enable_metrics": false,
        "metrics_port": 12798,
        "topology_file": "/tmp/testnet-topology.json"
    });

    fs::write(&helper.config_file, config.to_string()).expect("Failed to write testnet config");

    let loaded_config = NodeConfiguration::from_file(&helper.config_file).expect("Failed to load testnet config");
    assert_eq!(loaded_config.network_magic, 1097911063);
    assert_eq!(loaded_config.max_connections, 50);
    assert!(!loaded_config.enable_metrics);
}

// Performance and Large Configuration Tests

#[test]
fn test_large_topology_file() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");

    // Create topology with many producers
    let mut producers = vec![];
    for i in 0..100 {
        producers.push(json!({
            "addr": format!("relay-{}.cardano.example.com", i),
            "port": 3001,
            "valency": 1
        }));
    }

    let topology = json!({
        "producers": producers
    });

    fs::write(&helper.topology_file, topology.to_string()).expect("Failed to write large topology");

    let loaded_topology = NetworkTopology::from_file(&helper.topology_file).expect("Failed to load large topology");
    assert_eq!(loaded_topology.producers.len(), 100);

    let validation_result = loaded_topology.validate();
    assert!(validation_result.is_ok(), "Large topology validation should succeed");
}

#[test]
fn test_config_loading_performance() {
    let helper = ConfigTestHelper::new().expect("Failed to create test helper");
    helper.create_valid_config().expect("Failed to create valid config");

    let start = std::time::Instant::now();

    // Load config multiple times to test performance
    for _ in 0..100 {
        let _config = NodeConfiguration::from_file(&helper.config_file).expect("Failed to load config");
    }

    let duration = start.elapsed();

    // Should be able to load 100 configs in reasonable time (< 1 second)
    assert!(duration.as_millis() < 1000, "Config loading should be fast, took {}ms", duration.as_millis());
}

#[test]
fn test_concurrent_config_loading() {
    use std::thread;
    use std::sync::Arc;

    let helper = ConfigTestHelper::new().expect("Failed to create test helper");
    helper.create_valid_config().expect("Failed to create valid config");

    let config_path = Arc::new(helper.config_file.clone());
    let mut handles = vec![];

    // Spawn multiple threads to load config concurrently
    for _ in 0..10 {
        let path = Arc::clone(&config_path);
        let handle = thread::spawn(move || {
            NodeConfiguration::from_file(path.as_ref()).expect("Failed to load config")
        });
        handles.push(handle);
    }

    // Wait for all threads and verify results
    for handle in handles {
        let config = handle.join().expect("Thread failed");
        assert_eq!(config.network_magic, 764824073);
    }
}
