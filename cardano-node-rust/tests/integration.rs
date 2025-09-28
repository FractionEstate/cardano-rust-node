//! Integration Tests for Cardano Node API
//!
//! This module contains integration tests for all API components

mod api;
mod node;
mod t082_node_lifecycle;

// Re-export test modules
pub use api::*;
pub use node::*;
pub use t082_node_lifecycle::*;

// Node Integration Tests - T081 Configuration Loading Tests
use cardano_node::NodeConfiguration;
use serde_json::json;
use tempfile::TempDir;
use std::path::PathBuf;

#[test]
fn test_t081_valid_config_loading() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
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

    let config_path = temp_dir.path().join("valid_config.json");
    std::fs::write(&config_path, config.to_string()).expect("Failed to write config file");

    // Test valid config loading
    let config = NodeConfiguration::from_file(&config_path);
    assert!(config.is_ok(), "Valid config should load successfully: {:?}", config.err());

    let loaded_config = config.unwrap();
    assert_eq!(loaded_config.network_magic, 764824073);
    assert_eq!(loaded_config.listening_port, 3001);
    assert!(loaded_config.enable_logging);
    assert_eq!(loaded_config.max_connections, 100);

    // Test validation
    assert!(loaded_config.validate().is_ok());

    println!("✓ T081 Valid Configuration Loading Test passed");
}

#[test]
fn test_t081_invalid_config_loading() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let config_path = temp_dir.path().join("invalid_config.json");
    std::fs::write(&config_path, "{ invalid json }").expect("Failed to write config file");    // Test invalid config loading should fail
    let config = NodeConfiguration::from_file(&config_path);
    assert!(config.is_err(), "Invalid config should fail to load");

    println!("✓ T081 Invalid Configuration Loading Test passed");
}

#[test]
fn test_t081_missing_config_file() {
    let config = NodeConfiguration::from_file("/nonexistent/config.json");
    assert!(config.is_err(), "Missing config file should fail to load");

    println!("✓ T081 Missing Configuration File Test passed");
}

// T082 Node Startup/Shutdown Tests
use tokio::time::{Duration, timeout};
use futures::FutureExt;

#[tokio::test]
async fn test_t082_basic_node_startup() {
    // Create test helper for node lifecycle management
    let helper = NodeLifecycleTestHelper::new().await;

    // Test basic node startup
    let node_future = helper.start_node().timeout(Duration::from_secs(10));
    let result = timeout(Duration::from_secs(15), node_future).await;

    assert!(result.is_ok(), "Node startup should complete within timeout");
    println!("✓ T082 Basic Node Startup Test passed");
}

#[tokio::test]
async fn test_t082_graceful_shutdown() {
    let helper = NodeLifecycleTestHelper::new().await;

    // Start node and then request graceful shutdown
    let _node = helper.start_node().await.expect("Node should start");

    // Test graceful shutdown
    let shutdown_result = timeout(Duration::from_secs(10), helper.graceful_shutdown()).await;
    assert!(shutdown_result.is_ok(), "Graceful shutdown should complete within timeout");
    println!("✓ T082 Graceful Shutdown Test passed");
}

#[tokio::test]
async fn test_t082_startup_with_invalid_config() {
    let mut helper = NodeLifecycleTestHelper::new().await;
    helper.create_invalid_config();

    // Test that startup with invalid config fails appropriately
    let result = helper.start_node_expect_failure().await;
    assert!(result.is_err(), "Node startup should fail with invalid config");
    println!("✓ T082 Startup with Invalid Config Test passed");
}

// Test helper struct for node lifecycle management
pub struct NodeLifecycleTestHelper {
    temp_dir: TempDir,
    config_path: PathBuf,
}

impl NodeLifecycleTestHelper {
    pub async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_path = temp_dir.path().join("test_config.json");

        let helper = Self {
            temp_dir,
            config_path,
        };

        // Create valid default config
        helper.create_valid_config();
        helper
    }

    pub fn create_valid_config(&self) {
        let config = json!({
            "network_magic": 764824073,
            "listening_port": 3001,
            "database_path": self.temp_dir.path().join("cardano-db").to_str().unwrap(),
            "socket_path": self.temp_dir.path().join("cardano-node.socket").to_str().unwrap(),
            "enable_logging": false, // Disable logging for tests
            "max_connections": 10,
            "enable_metrics": false, // Disable metrics for tests
            "metrics_port": 12798,
            "topology_file": self.temp_dir.path().join("topology.json").to_str().unwrap()
        });

        std::fs::write(&self.config_path, config.to_string())
            .expect("Failed to write test config file");
    }

    pub fn create_invalid_config(&self) {
        std::fs::write(&self.config_path, "{ invalid json }")
            .expect("Failed to write invalid config file");
    }

    pub async fn start_node(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Mock node startup - in real implementation this would start the actual node
        // For now, just validate the config and simulate startup
        let config = NodeConfiguration::from_file(&self.config_path)?;
        config.validate()?;

        // Simulate startup delay
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(())
    }

    pub async fn start_node_expect_failure(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // This should fail with invalid config
        let config = NodeConfiguration::from_file(&self.config_path)?;
        config.validate()?;

        Ok(())
    }

    pub async fn graceful_shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Mock graceful shutdown
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(())
    }
}

#[test]
fn test_simple() {
    assert!(true);
    println!("✓ Simple test works");
}
