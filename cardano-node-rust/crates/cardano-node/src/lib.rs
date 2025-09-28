//! Cardano Node Library
//!
//! Core library components for Cardano Node functionality.

use anyhow::Result;

// Re-export CLI components from the cli module
pub mod cli;
pub use cli::{CardanoNodeCli, Commands, RunArgs, VersionArgs, ValidateArgs, InfoArgs, parse_cli, parse_cli_from, parse_cli_from_without_validation};

// Re-export configuration components from the config module
pub mod config;
pub use config::{
    NodeConfiguration, NetworkTopology, TopologyProducer,
    AdvancedNodeConfiguration, ProtocolVersion, LoggingConfiguration, TracingConfiguration,
    ConfigurationManager, ConfigTestHelper
};

// Re-export node runtime components
pub mod run;
pub use run::{NodeRuntime, NodeState, NodeEvent, NodeRuntimeError, run_node_runtime};



/// Main node execution function
/// 
/// This function initializes the configuration and starts the node runtime.
/// It integrates with the new NodeRuntime for complete node functionality.
pub async fn run_node(args: RunArgs) -> Result<()> {
    tracing::info!("Starting Cardano Node with configuration: {:?}", args.config);

    // Initialize configuration manager
    let mut config_manager = ConfigurationManager::new();

    // Load and validate configuration if provided
    if let Some(config_path) = &args.config {
        config_manager.load_config(config_path)?;
        tracing::info!("Node configuration loaded successfully from: {:?}", config_path);
    } else {
        tracing::info!("No configuration file provided, using defaults");
    }

    // Apply CLI arguments to configuration
    config_manager.apply_cli_args(&args);

    // Load topology file if specified
    if let Some(topology_path) = &args.topology {
        config_manager.load_topology(topology_path)?;
        tracing::info!("Network topology loaded successfully from: {:?}", topology_path);
    }

    // Validate all configurations
    config_manager.validate_all()?;
    tracing::info!("All configurations validated successfully");

    // Start the node runtime with the configured manager
    run_node_runtime(config_manager).await?;

    Ok(())
}#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_basic_parsing() {
        let args = vec!["cardano-node", "run"];
        let cli = parse_cli_from(args);
        assert!(cli.is_ok(), "Basic CLI parsing should succeed");
    }

    #[test]
    fn test_version_command() {
        let args = vec!["cardano-node", "version"];
        let cli = parse_cli_from(args).unwrap();
        assert!(matches!(cli.command, Commands::Version(_)));
    }
}
