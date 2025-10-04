//! Command Line Interface Module
//!
//! Handles CLI argument parsing, validation, and command execution for cardano-node.

pub mod commands;
pub use commands::*;

use anyhow::{Context, Result};
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Cardano Node command-line interface
#[derive(Parser, Debug, Clone)]
#[command(name = "cardano-node")]
#[command(about = "Cardano Node - Rust Implementation")]
#[command(version = "8.7.3")]
#[command(long_about = "
A Rust implementation of the Cardano blockchain node.

This node implementation provides full compatibility with the Haskell cardano-node
while offering improved performance and memory efficiency through Rust's
zero-cost abstractions and memory safety guarantees.

Features:
- Full Ouroboros consensus protocol support
- Compatible with existing Cardano networks (mainnet, testnet, etc.)
- High-performance block validation and transaction processing
- Comprehensive metrics and monitoring capabilities
- Memory-efficient storage and caching
")]
pub struct CardanoNodeCli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose output
    #[arg(long, short, global = true, help = "Enable verbose logging")]
    pub verbose: bool,

    /// Log level (error, warn, info, debug, trace)
    #[arg(
        long,
        global = true,
        default_value = "info",
        help = "Set logging level"
    )]
    pub log_level: String,

    /// Log format (json, plain)
    #[arg(
        long,
        global = true,
        default_value = "plain",
        help = "Set log output format"
    )]
    pub log_format: String,
}

/// Available commands for the Cardano Node
#[derive(Parser, Debug, Clone)]
pub enum Commands {
    /// Run the cardano-node
    Run(Box<RunArgs>),

    /// Show version information
    Version(VersionArgs),

    /// Validate configuration files
    Validate(ValidateArgs),

    /// Show node information and statistics
    Info(InfoArgs),

    /// Query blockchain and node state
    Query(QueryArgs),

    /// Transaction operations
    Transaction(TransactionArgs),

    /// Stake pool operations
    StakePool(StakePoolArgs),

    /// Stake address operations
    StakeAddress(StakeAddressArgs),

    /// Address operations
    Address(AddressArgs),

    /// Governance operations (Conway era)
    Governance(GovernanceArgs),

    /// Interactive terminal dashboard
    Dashboard(DashboardArgs),

    /// Node administration commands
    Admin(AdminArgs),
}

/// Arguments for the run command
#[derive(Parser, Debug, Clone)]
pub struct RunArgs {
    /// Configuration file path
    #[arg(long, help = "Path to the node configuration file")]
    pub config: Option<PathBuf>,

    /// Network topology file path
    #[arg(long, help = "Path to the network topology file")]
    pub topology: Option<PathBuf>,

    /// Database path for blockchain storage
    #[arg(long, help = "Path to the blockchain database directory")]
    pub database_path: Option<PathBuf>,

    /// Socket path for local client connections
    #[arg(long, help = "Path to the node socket for local connections")]
    pub socket_path: Option<PathBuf>,

    /// Host address to bind to
    #[arg(long, help = "Host address to bind the node to")]
    pub host_addr: Option<SocketAddr>,

    /// Port to listen on for peer connections
    #[arg(long, help = "Port number for peer connections")]
    pub port: Option<u16>,

    /// Protocol magic number for network identification
    #[arg(long, help = "Protocol magic number for the network")]
    pub protocol_magic: Option<u32>,

    /// Validate database on startup
    #[arg(long, help = "Perform database validation during startup")]
    pub validate_db: bool,

    /// Shutdown IPC file path
    #[arg(long, help = "Path to the shutdown signal file")]
    pub shutdown_ipc: Option<PathBuf>,

    /// Enable metrics collection
    #[arg(long, help = "Enable Prometheus metrics collection")]
    pub metrics: bool,

    /// Metrics host address
    #[arg(long, help = "Host address for metrics endpoint")]
    pub metrics_host: Option<String>,

    /// Metrics port
    #[arg(long, help = "Port for Prometheus metrics endpoint")]
    pub metrics_port: Option<u16>,

    /// Byron genesis file path
    #[arg(long, help = "Path to Byron era genesis configuration")]
    pub byron_genesis: Option<PathBuf>,

    /// Shelley genesis file path
    #[arg(long, help = "Path to Shelley era genesis configuration")]
    pub shelley_genesis: Option<PathBuf>,

    /// Alonzo genesis file path
    #[arg(long, help = "Path to Alonzo era genesis configuration")]
    pub alonzo_genesis: Option<PathBuf>,

    /// Conway genesis file path
    #[arg(long, help = "Path to Conway era genesis configuration")]
    pub conway_genesis: Option<PathBuf>,

    /// Enable development mode with relaxed validation
    #[arg(long, help = "Enable development mode (warning: less secure)")]
    pub dev_mode: bool,
}

/// Arguments for version command
#[derive(Parser, Debug, Clone)]
pub struct VersionArgs {
    /// Show detailed version information
    #[arg(long, help = "Show detailed version and build information")]
    pub detailed: bool,

    /// Output format (plain, json)
    #[arg(long, default_value = "plain", help = "Output format for version info")]
    pub format: String,
}

/// Arguments for validate command
#[derive(Parser, Debug, Clone)]
pub struct ValidateArgs {
    /// Configuration file to validate
    #[arg(long, help = "Path to configuration file to validate")]
    pub config: Option<PathBuf>,

    /// Topology file to validate
    #[arg(long, help = "Path to topology file to validate")]
    pub topology: Option<PathBuf>,

    /// Genesis files to validate
    #[arg(long, help = "Path to genesis file to validate")]
    pub genesis: Option<PathBuf>,

    /// Validate all configuration files
    #[arg(long, help = "Validate all configuration files")]
    pub all: bool,
}

/// Arguments for info command
#[derive(Parser, Debug, Clone)]
pub struct InfoArgs {
    /// Show protocol information
    #[arg(long, help = "Show supported protocol versions")]
    pub protocol: bool,

    /// Show network information
    #[arg(long, help = "Show network configuration")]
    pub network: bool,

    /// Show build information
    #[arg(long, help = "Show build and compilation information")]
    pub build: bool,

    /// Output format (plain, json)
    #[arg(long, default_value = "plain", help = "Output format for info")]
    pub format: String,
}

impl CardanoNodeCli {
    /// Parse command line arguments
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }

    /// Parse from iterator (useful for testing)
    pub fn try_parse_from<I, T>(iter: I) -> Result<Self>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        <Self as Parser>::try_parse_from(iter).context("Failed to parse command line arguments")
    }

    /// Validate the parsed CLI arguments
    pub fn validate(&self) -> Result<()> {
        // Validate log level
        match self.log_level.to_lowercase().as_str() {
            "error" | "warn" | "info" | "debug" | "trace" => {}
            _ => return Err(anyhow::anyhow!("Invalid log level: {}", self.log_level)),
        }

        // Validate log format
        match self.log_format.to_lowercase().as_str() {
            "json" | "plain" => {}
            _ => return Err(anyhow::anyhow!("Invalid log format: {}", self.log_format)),
        }

        // Validate command-specific arguments
        match &self.command {
            Commands::Run(args) => self.validate_run_args(args)?,
            Commands::Version(args) => self.validate_version_args(args)?,
            Commands::Validate(args) => self.validate_validate_args(args)?,
            Commands::Info(args) => self.validate_info_args(args)?,
            // Other commands have their own validation logic
            _ => {}
        }

        Ok(())
    }

    fn validate_run_args(&self, args: &RunArgs) -> Result<()> {
        // Check config file exists if provided
        if let Some(config_path) = &args.config {
            if !config_path.exists() {
                return Err(anyhow::anyhow!(
                    "Configuration file does not exist: {:?}",
                    config_path
                ));
            }
        }

        // Check topology file exists if provided
        if let Some(topology_path) = &args.topology {
            if !topology_path.exists() {
                return Err(anyhow::anyhow!(
                    "Topology file does not exist: {:?}",
                    topology_path
                ));
            }
        }

        // Check genesis files exist if provided
        for (name, path) in [
            ("Byron genesis", &args.byron_genesis),
            ("Shelley genesis", &args.shelley_genesis),
            ("Alonzo genesis", &args.alonzo_genesis),
            ("Conway genesis", &args.conway_genesis),
        ] {
            if let Some(genesis_path) = path {
                if !genesis_path.exists() {
                    return Err(anyhow::anyhow!(
                        "{} file does not exist: {:?}",
                        name,
                        genesis_path
                    ));
                }
            }
        }

        // Validate port numbers (port 0 is valid - means system chooses available port)
        // No additional validation needed as clap already validates port range 0-65535

        // Validate metrics configuration
        if args.metrics && args.metrics_host.is_none() && args.metrics_port.is_none() {
            tracing::warn!("Metrics enabled but no host or port specified, using defaults");
        }

        Ok(())
    }

    fn validate_version_args(&self, args: &VersionArgs) -> Result<()> {
        match args.format.to_lowercase().as_str() {
            "plain" | "json" => Ok(()),
            _ => Err(anyhow::anyhow!("Invalid version format: {}", args.format)),
        }
    }

    fn validate_validate_args(&self, args: &ValidateArgs) -> Result<()> {
        // Must specify at least one file to validate
        if args.config.is_none() && args.topology.is_none() && args.genesis.is_none() && !args.all {
            return Err(anyhow::anyhow!(
                "Must specify at least one file to validate or use --all"
            ));
        }

        Ok(())
    }

    fn validate_info_args(&self, args: &InfoArgs) -> Result<()> {
        match args.format.to_lowercase().as_str() {
            "plain" | "json" => Ok(()),
            _ => Err(anyhow::anyhow!("Invalid info format: {}", args.format)),
        }
    }

    /// Get help text for the CLI
    pub fn get_help() -> String {
        let mut app = <Self as clap::CommandFactory>::command();
        let mut help_text = Vec::new();
        app.write_help(&mut help_text).unwrap();
        String::from_utf8(help_text).unwrap()
    }
}

/// Parse and validate command line arguments
pub fn parse_cli() -> Result<CardanoNodeCli> {
    let cli = CardanoNodeCli::parse();
    cli.validate()?;
    Ok(cli)
}

/// Parse CLI from arguments (used for testing)
pub fn parse_cli_from<I, T>(args: I) -> Result<CardanoNodeCli>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = CardanoNodeCli::try_parse_from(args)?;
    cli.validate()?;
    Ok(cli)
}

/// Parse CLI from arguments without validation (used for testing parsing only)
pub fn parse_cli_from_without_validation<I, T>(args: I) -> Result<CardanoNodeCli>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = CardanoNodeCli::try_parse_from(args)?;
    Ok(cli)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_cli_parsing() {
        let cli = CardanoNodeCli::try_parse_from(["cardano-node", "run"]).unwrap();
        assert!(matches!(cli.command, Commands::Run(_)));
    }

    #[test]
    fn test_version_command() {
        let cli = CardanoNodeCli::try_parse_from(["cardano-node", "version"]).unwrap();
        assert!(matches!(cli.command, Commands::Version(_)));
    }

    #[test]
    fn test_validate_command() {
        let cli = CardanoNodeCli::try_parse_from(["cardano-node", "validate", "--all"]).unwrap();
        assert!(matches!(cli.command, Commands::Validate(_)));
    }

    #[test]
    fn test_info_command() {
        let cli = CardanoNodeCli::try_parse_from(["cardano-node", "info", "--protocol"]).unwrap();
        assert!(matches!(cli.command, Commands::Info(_)));
    }

    #[test]
    fn test_global_flags() {
        let cli = CardanoNodeCli::try_parse_from([
            "cardano-node",
            "--verbose",
            "--log-level",
            "debug",
            "--log-format",
            "json",
            "run",
        ])
        .unwrap();

        assert!(cli.verbose);
        assert_eq!(cli.log_level, "debug");
        assert_eq!(cli.log_format, "json");
    }

    #[test]
    fn test_run_with_all_args() {
        let cli = CardanoNodeCli::try_parse_from([
            "cardano-node",
            "run",
            "--config",
            "/path/to/config.json",
            "--topology",
            "/path/to/topology.json",
            "--database-path",
            "/path/to/db",
            "--socket-path",
            "/path/to/socket",
            "--port",
            "3001",
            "--protocol-magic",
            "764824073",
            "--metrics",
            "--metrics-port",
            "12798",
            "--validate-db",
            "--dev-mode",
        ])
        .unwrap();

        if let Commands::Run(args) = cli.command {
            assert!(args.config.is_some());
            assert!(args.topology.is_some());
            assert!(args.database_path.is_some());
            assert!(args.socket_path.is_some());
            assert_eq!(args.port, Some(3001));
            assert_eq!(args.protocol_magic, Some(764824073));
            assert!(args.metrics);
            assert_eq!(args.metrics_port, Some(12798));
            assert!(args.validate_db);
            assert!(args.dev_mode);
        } else {
            panic!("Expected run command");
        }
    }

    #[test]
    fn test_invalid_log_level_fails() {
        let cli = CardanoNodeCli::try_parse_from(["cardano-node", "--log-level", "invalid", "run"])
            .unwrap();

        assert!(cli.validate().is_err());
    }

    #[test]
    fn test_invalid_format_fails() {
        let cli =
            CardanoNodeCli::try_parse_from(["cardano-node", "version", "--format", "invalid"])
                .unwrap();

        assert!(cli.validate().is_err());
    }
}
