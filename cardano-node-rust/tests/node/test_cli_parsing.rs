//! CLI Argument Parsing Tests for Cardano Node
//!
//! Tests for command line argument parsing, validation, and error handling.
//! Based on the Haskell cardano-node CLI structure.

use std::path::PathBuf;
use clap::{Parser, Subcommand, ArgAction};
use tempfile::{tempdir, NamedTempFile};

/// Test configuration for CLI parsing
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub config_file: PathBuf,
    pub topology_file: PathBuf,
    pub database_path: PathBuf,
    pub socket_path: PathBuf,
}

impl TestConfig {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;

        let config_file = temp_dir.path().join("config.json");
        let topology_file = temp_dir.path().join("topology.json");
        let database_path = temp_dir.path().join("db");
        let socket_path = temp_dir.path().join("node.socket");

        // Create minimal test files
        std::fs::write(&config_file, r#"{"defaultScribes": []}"#)?;
        std::fs::write(&topology_file, r#"{"Producers": []}"#)?;
        std::fs::create_dir_all(&database_path)?;

        Ok(Self {
            config_file,
            topology_file,
            database_path,
            socket_path,
        })
    }
}

/// Main CLI structure matching Haskell cardano-node
#[derive(Debug, Parser)]
#[command(name = "cardano-node")]
#[command(about = "Start node of the Cardano blockchain")]
#[command(long_about = None)]
pub struct CardanoNodeCli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Available commands for cardano-node
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Run the node
    Run(RunArgs),
    /// Show version information
    Version,
}

/// Arguments for the 'run' command
#[derive(Debug, Parser)]
pub struct RunArgs {
    /// Configuration file for the cardano-node
    #[arg(long = "config", value_name = "NODE-CONFIGURATION")]
    pub config: PathBuf,

    /// The path to a file describing the topology
    #[arg(long = "topology", value_name = "FILEPATH")]
    pub topology: PathBuf,

    /// Path to the blockchain database
    #[arg(long = "database-path", value_name = "FILEPATH")]
    pub database_path: PathBuf,

    /// Path to a cardano-node socket
    #[arg(long = "socket-path", value_name = "FILEPATH")]
    pub socket_path: PathBuf,

    /// An optional IPv4 address
    #[arg(long = "host-addr", value_name = "IPV4")]
    pub host_addr: Option<std::net::Ipv4Addr>,

    /// An optional IPv6 address
    #[arg(long = "host-ipv6-addr", value_name = "IPV6")]
    pub host_ipv6_addr: Option<std::net::Ipv6Addr>,

    /// The port number
    #[arg(long = "port", value_name = "PORT", default_value = "3001")]
    pub port: u16,

    /// Validate all on-disk database files
    #[arg(long = "validate-db", action = ArgAction::SetTrue)]
    pub validate_db: bool,

    /// Path to the Byron delegation certificate
    #[arg(long = "byron-delegation-certificate", value_name = "FILEPATH")]
    pub byron_delegation_certificate: Option<PathBuf>,

    /// Path to the Byron signing key
    #[arg(long = "byron-signing-key", value_name = "FILEPATH")]
    pub byron_signing_key: Option<PathBuf>,

    /// Path to the KES signing key
    #[arg(long = "shelley-kes-key", value_name = "FILEPATH")]
    pub shelley_kes_key: Option<PathBuf>,

    /// Path to the VRF signing key
    #[arg(long = "shelley-vrf-key", value_name = "FILEPATH")]
    pub shelley_vrf_key: Option<PathBuf>,

    /// Path to the delegation certificate
    #[arg(long = "shelley-operational-certificate", value_name = "FILEPATH")]
    pub shelley_operational_certificate: Option<PathBuf>,

    /// Path to the bulk pool credentials file
    #[arg(long = "bulk-credentials-file", value_name = "FILEPATH")]
    pub bulk_credentials_file: Option<PathBuf>,

    /// Start the node as a non block producing node
    #[arg(long = "start-as-non-producing-node", action = ArgAction::SetTrue)]
    pub start_as_non_producing_node: bool,

    /// Shut down the process when this inherited FD reaches EOF
    #[arg(long = "shutdown-ipc", value_name = "FD")]
    pub shutdown_ipc: Option<i32>,

    /// Connect to cardano-tracer listening on HOST:PORT
    #[arg(long = "tracer-socket-network-connect", value_name = "HOST:PORT")]
    pub tracer_socket_network_connect: Option<String>,

    /// Connect to cardano-tracer listening on a local socket
    #[arg(long = "tracer-socket-path-connect", value_name = "FILEPATH")]
    pub tracer_socket_path_connect: Option<PathBuf>,

    /// Accept incoming cardano-tracer connection on HOST:PORT
    #[arg(long = "tracer-socket-network-accept", value_name = "HOST:PORT")]
    pub tracer_socket_network_accept: Option<String>,

    /// Accept incoming cardano-tracer connection at local socket
    #[arg(long = "tracer-socket-path-accept", value_name = "FILEPATH")]
    pub tracer-socket_path_accept: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    #[test]
    fn test_cli_basic_structure() {
        let cli = CardanoNodeCli::try_parse_from(&["cardano-node", "--help"]);
        assert!(cli.is_err());

        let error = cli.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
    }

    #[test]
    fn test_version_command() {
        let cli = CardanoNodeCli::try_parse_from(&["cardano-node", "version"]);
        assert!(cli.is_ok());

        let parsed = cli.unwrap();
        matches!(parsed.command, Some(Commands::Version));
    }

    #[test]
    fn test_run_command_required_args() {
        let test_config = TestConfig::new().expect("Failed to create test config");

        let args = vec![
            "cardano-node",
            "run",
            "--config", test_config.config_file.to_str().unwrap(),
            "--topology", test_config.topology_file.to_str().unwrap(),
            "--database-path", test_config.database_path.to_str().unwrap(),
            "--socket-path", test_config.socket_path.to_str().unwrap(),
        ];

        let cli = CardanoNodeCli::try_parse_from(&args);
        assert!(cli.is_ok(), "Failed to parse valid run command: {:?}", cli.err());

        let parsed = cli.unwrap();
        if let Some(Commands::Run(run_args)) = parsed.command {
            assert_eq!(run_args.config, test_config.config_file);
            assert_eq!(run_args.topology, test_config.topology_file);
            assert_eq!(run_args.database_path, test_config.database_path);
            assert_eq!(run_args.socket_path, test_config.socket_path);
            assert_eq!(run_args.port, 3001); // Default port
        } else {
            panic!("Expected Run command");
        }
    }

    #[test]
    fn test_run_command_missing_required_args() {
        let cli = CardanoNodeCli::try_parse_from(&["cardano-node", "run"]);
        assert!(cli.is_err());

        let error = cli.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn test_run_command_with_optional_network_args() {
        let test_config = TestConfig::new().expect("Failed to create test config");

        let args = vec![
            "cardano-node",
            "run",
            "--config", test_config.config_file.to_str().unwrap(),
            "--topology", test_config.topology_file.to_str().unwrap(),
            "--database-path", test_config.database_path.to_str().unwrap(),
            "--socket-path", test_config.socket_path.to_str().unwrap(),
            "--host-addr", "127.0.0.1",
            "--port", "8080",
        ];

        let cli = CardanoNodeCli::try_parse_from(&args);
        assert!(cli.is_ok());

        let parsed = cli.unwrap();
        if let Some(Commands::Run(run_args)) = parsed.command {
            assert_eq!(run_args.host_addr, Some(std::net::Ipv4Addr::new(127, 0, 0, 1)));
            assert_eq!(run_args.port, 8080);
        } else {
            panic!("Expected Run command");
        }
    }

    #[test]
    fn test_run_command_with_ipv6_address() {
        let test_config = TestConfig::new().expect("Failed to create test config");

        let args = vec![
            "cardano-node",
            "run",
            "--config", test_config.config_file.to_str().unwrap(),
            "--topology", test_config.topology_file.to_str().unwrap(),
            "--database-path", test_config.database_path.to_str().unwrap(),
            "--socket-path", test_config.socket_path.to_str().unwrap(),
            "--host-ipv6-addr", "::1",
        ];

        let cli = CardanoNodeCli::try_parse_from(&args);
        assert!(cli.is_ok());

        let parsed = cli.unwrap();
        if let Some(Commands::Run(run_args)) = parsed.command {
            assert_eq!(run_args.host_ipv6_addr, Some(std::net::Ipv6Addr::LOCALHOST));
        } else {
            panic!("Expected Run command");
        }
    }

    #[test]
    fn test_run_command_with_producer_keys() {
        let test_config = TestConfig::new().expect("Failed to create test config");

        // Create temporary key files
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let kes_key = temp_dir.path().join("kes.skey");
        let vrf_key = temp_dir.path().join("vrf.skey");
        let cert_file = temp_dir.path().join("cert.cert");

        std::fs::write(&kes_key, "test_kes_key").expect("Failed to write KES key");
        std::fs::write(&vrf_key, "test_vrf_key").expect("Failed to write VRF key");
        std::fs::write(&cert_file, "test_cert").expect("Failed to write cert");

        let args = vec![
            "cardano-node",
            "run",
            "--config", test_config.config_file.to_str().unwrap(),
            "--topology", test_config.topology_file.to_str().unwrap(),
            "--database-path", test_config.database_path.to_str().unwrap(),
            "--socket-path", test_config.socket_path.to_str().unwrap(),
            "--shelley-kes-key", kes_key.to_str().unwrap(),
            "--shelley-vrf-key", vrf_key.to_str().unwrap(),
            "--shelley-operational-certificate", cert_file.to_str().unwrap(),
        ];

        let cli = CardanoNodeCli::try_parse_from(&args);
        assert!(cli.is_ok());

        let parsed = cli.unwrap();
        if let Some(Commands::Run(run_args)) = parsed.command {
            assert_eq!(run_args.shelley_kes_key, Some(kes_key));
            assert_eq!(run_args.shelley_vrf_key, Some(vrf_key));
            assert_eq!(run_args.shelley_operational_certificate, Some(cert_file));
        } else {
            panic!("Expected Run command");
        }
    }

    #[test]
    fn test_run_command_with_byron_keys() {
        let test_config = TestConfig::new().expect("Failed to create test config");

        // Create temporary Byron key files
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let signing_key = temp_dir.path().join("byron.skey");
        let delegation_cert = temp_dir.path().join("byron.cert");

        std::fs::write(&signing_key, "test_byron_signing_key").expect("Failed to write Byron signing key");
        std::fs::write(&delegation_cert, "test_byron_cert").expect("Failed to write Byron cert");

        let args = vec![
            "cardano-node",
            "run",
            "--config", test_config.config_file.to_str().unwrap(),
            "--topology", test_config.topology_file.to_str().unwrap(),
            "--database-path", test_config.database_path.to_str().unwrap(),
            "--socket-path", test_config.socket_path.to_str().unwrap(),
            "--byron-signing-key", signing_key.to_str().unwrap(),
            "--byron-delegation-certificate", delegation_cert.to_str().unwrap(),
        ];

        let cli = CardanoNodeCli::try_parse_from(&args);
        assert!(cli.is_ok());

        let parsed = cli.unwrap();
        if let Some(Commands::Run(run_args)) = parsed.command {
            assert_eq!(run_args.byron_signing_key, Some(signing_key));
            assert_eq!(run_args.byron_delegation_certificate, Some(delegation_cert));
        } else {
            panic!("Expected Run command");
        }
    }

    #[test]
    fn test_run_command_with_flags() {
        let test_config = TestConfig::new().expect("Failed to create test config");

        let args = vec![
            "cardano-node",
            "run",
            "--config", test_config.config_file.to_str().unwrap(),
            "--topology", test_config.topology_file.to_str().unwrap(),
            "--database-path", test_config.database_path.to_str().unwrap(),
            "--socket-path", test_config.socket_path.to_str().unwrap(),
            "--validate-db",
            "--start-as-non-producing-node",
        ];

        let cli = CardanoNodeCli::try_parse_from(&args);
        assert!(cli.is_ok());

        let parsed = cli.unwrap();
        if let Some(Commands::Run(run_args)) = parsed.command {
            assert!(run_args.validate_db);
            assert!(run_args.start_as_non_producing_node);
        } else {
            panic!("Expected Run command");
        }
    }

    #[test]
    fn test_run_command_with_tracer_options() {
        let test_config = TestConfig::new().expect("Failed to create test config");

        let temp_dir = tempdir().expect("Failed to create temp dir");
        let tracer_socket = temp_dir.path().join("tracer.socket");

        let args = vec![
            "cardano-node",
            "run",
            "--config", test_config.config_file.to_str().unwrap(),
            "--topology", test_config.topology_file.to_str().unwrap(),
            "--database-path", test_config.database_path.to_str().unwrap(),
            "--socket-path", test_config.socket_path.to_str().unwrap(),
            "--tracer-socket-path-connect", tracer_socket.to_str().unwrap(),
            "--tracer-socket-network-connect", "localhost:8080",
        ];

        let cli = CardanoNodeCli::try_parse_from(&args);
        assert!(cli.is_ok());

        let parsed = cli.unwrap();
        if let Some(Commands::Run(run_args)) = parsed.command {
            assert_eq!(run_args.tracer_socket_path_connect, Some(tracer_socket));
            assert_eq!(run_args.tracer_socket_network_connect, Some("localhost:8080".to_string()));
        } else {
            panic!("Expected Run command");
        }
    }

    #[test]
    fn test_invalid_ipv4_address() {
        let test_config = TestConfig::new().expect("Failed to create test config");

        let args = vec![
            "cardano-node",
            "run",
            "--config", test_config.config_file.to_str().unwrap(),
            "--topology", test_config.topology_file.to_str().unwrap(),
            "--database-path", test_config.database_path.to_str().unwrap(),
            "--socket-path", test_config.socket_path.to_str().unwrap(),
            "--host-addr", "999.999.999.999",
        ];

        let cli = CardanoNodeCli::try_parse_from(&args);
        assert!(cli.is_err());

        let error = cli.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ValueValidation);
    }

    #[test]
    fn test_invalid_port_number() {
        let test_config = TestConfig::new().expect("Failed to create test config");

        let args = vec![
            "cardano-node",
            "run",
            "--config", test_config.config_file.to_str().unwrap(),
            "--topology", test_config.topology_file.to_str().unwrap(),
            "--database-path", test_config.database_path.to_str().unwrap(),
            "--socket-path", test_config.socket_path.to_str().unwrap(),
            "--port", "99999",
        ];

        let cli = CardanoNodeCli::try_parse_from(&args);
        assert!(cli.is_err());

        let error = cli.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ValueValidation);
    }

    #[test]
    fn test_unknown_command() {
        let cli = CardanoNodeCli::try_parse_from(&["cardano-node", "unknown"]);
        assert!(cli.is_err());

        let error = cli.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn test_no_subcommand_shows_help() {
        let cli = CardanoNodeCli::try_parse_from(&["cardano-node"]);
        assert!(cli.is_ok()); // Should parse successfully but have None command

        let parsed = cli.unwrap();
        assert!(parsed.command.is_none());
    }

    #[test]
    fn test_help_messages() {
        // Test main help
        let cli = CardanoNodeCli::try_parse_from(&["cardano-node", "--help"]);
        assert!(cli.is_err());
        let error = cli.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
        let help_text = error.to_string();
        assert!(help_text.contains("Start node of the Cardano blockchain"));

        // Test run command help
        let cli = CardanoNodeCli::try_parse_from(&["cardano-node", "run", "--help"]);
        assert!(cli.is_err());
        let error = cli.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
        let help_text = error.to_string();
        assert!(help_text.contains("Run the node"));
        assert!(help_text.contains("--config"));
        assert!(help_text.contains("--topology"));
        assert!(help_text.contains("--database-path"));
        assert!(help_text.contains("--socket-path"));
    }

    #[test]
    fn test_argument_value_validation() {
        let test_config = TestConfig::new().expect("Failed to create test config");

        // Test with non-existent config file (should still parse, validation happens later)
        let args = vec![
            "cardano-node",
            "run",
            "--config", "/nonexistent/config.json",
            "--topology", test_config.topology_file.to_str().unwrap(),
            "--database-path", test_config.database_path.to_str().unwrap(),
            "--socket-path", test_config.socket_path.to_str().unwrap(),
        ];

        let cli = CardanoNodeCli::try_parse_from(&args);
        assert!(cli.is_ok(), "CLI parsing should succeed even with non-existent files");
    }
}
