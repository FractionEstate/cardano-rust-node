//! Comprehensive CLI argument parsing tests for T080
//!
//! Tests all CLI argument combinations and error scenarios for cardano-node.

use cardano_node::cli::CardanoNodeCli;
use cardano_node::{parse_cli_from, parse_cli_from_without_validation, Commands};
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

/// Helper function to parse CLI arguments
fn parse_cli(args: &[&str]) -> anyhow::Result<cardano_node::CardanoNodeCli> {
    parse_cli_from(args)
}

/// Helper function to parse CLI arguments without file validation
fn parse_cli_no_validation(args: &[&str]) -> anyhow::Result<cardano_node::CardanoNodeCli> {
    parse_cli_from_without_validation(args)
}

/// Helper function to create test paths
fn test_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/tmp/test/{}", name))
}

/// Helper function to setup test files and directories
fn setup_test_files() {
    use std::fs;

    let test_dir = "/tmp/test";
    fs::create_dir_all(test_dir).unwrap_or(());

    // Create dummy config files for testing
    let config_path = format!("{}/config.yaml", test_dir);
    fs::write(&config_path, "# Test configuration").unwrap_or(());

    let topology_path = format!("{}/topology.json", test_dir);
    fs::write(&topology_path, "{}").unwrap_or(());
}

#[cfg(test)]
mod cli_parsing_tests {
    use super::*;

    #[test]
    fn test_version_command() {
        let args = ["cardano-node", "version"];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        assert!(matches!(cli.command, Commands::Version(_)));
    }

    #[test]
    fn test_run_command_no_arguments() {
        let args = ["cardano-node", "run"];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert!(run_args.config.is_none());
                assert!(run_args.topology.is_none());
                assert!(run_args.database_path.is_none());
                assert!(run_args.socket_path.is_none());
                assert!(run_args.host_addr.is_none());
                assert!(run_args.port.is_none());
                assert!(run_args.protocol_magic.is_none());
                assert!(!run_args.validate_db);
                assert!(run_args.shutdown_ipc.is_none());
                assert!(!run_args.metrics);
                assert!(run_args.metrics_host.is_none());
                assert!(run_args.metrics_port.is_none());
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_command_with_config() {
        setup_test_files();
        let config_path = test_path("config.yaml");
        let args = [
            "cardano-node",
            "run",
            "--config",
            config_path.to_str().unwrap(),
        ];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(run_args.config, Some(config_path));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_command_with_topology() {
        setup_test_files();
        let topology_path = test_path("topology.json");
        let args = [
            "cardano-node",
            "run",
            "--topology",
            topology_path.to_str().unwrap(),
        ];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(run_args.topology, Some(topology_path));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_command_with_database_path() {
        let db_path = test_path("cardano-db");
        let args = [
            "cardano-node",
            "run",
            "--database-path",
            db_path.to_str().unwrap(),
        ];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(run_args.database_path, Some(db_path));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_command_with_socket_path() {
        let socket_path = test_path("cardano-node.socket");
        let args = [
            "cardano-node",
            "run",
            "--socket-path",
            socket_path.to_str().unwrap(),
        ];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(run_args.socket_path, Some(socket_path));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_command_with_host_addr() {
        let host = "127.0.0.1:3001";
        let args = ["cardano-node", "run", "--host-addr", host];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(
                    run_args.host_addr,
                    Some(SocketAddr::from_str(host).unwrap())
                );
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_command_with_port() {
        let args = ["cardano-node", "run", "--port", "3001"];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(run_args.port, Some(3001));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_command_with_protocol_magic() {
        let args = ["cardano-node", "run", "--protocol-magic", "764824073"];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(run_args.protocol_magic, Some(764824073));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_command_with_validate_db_flag() {
        let args = ["cardano-node", "run", "--validate-db"];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert!(run_args.validate_db);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_command_with_shutdown_ipc() {
        let ipc_path = test_path("shutdown.ipc");
        let args = [
            "cardano-node",
            "run",
            "--shutdown-ipc",
            ipc_path.to_str().unwrap(),
        ];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(run_args.shutdown_ipc, Some(ipc_path));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_command_with_metrics_flag() {
        let args = ["cardano-node", "run", "--metrics"];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert!(run_args.metrics);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_command_with_metrics_host() {
        let args = ["cardano-node", "run", "--metrics-host", "0.0.0.0"];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(run_args.metrics_host, Some("0.0.0.0".to_string()));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_command_with_metrics_port() {
        let args = ["cardano-node", "run", "--metrics-port", "12798"];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(run_args.metrics_port, Some(12798));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_command_with_all_arguments() {
        setup_test_files();
        let config_path = test_path("config.yaml");
        let topology_path = test_path("topology.json");
        let db_path = test_path("cardano-db");
        let socket_path = test_path("cardano-node.socket");
        let ipc_path = test_path("shutdown.ipc");

        let args = [
            "cardano-node",
            "run",
            "--config",
            config_path.to_str().unwrap(),
            "--topology",
            topology_path.to_str().unwrap(),
            "--database-path",
            db_path.to_str().unwrap(),
            "--socket-path",
            socket_path.to_str().unwrap(),
            "--host-addr",
            "127.0.0.1:3001",
            "--port",
            "3001",
            "--protocol-magic",
            "764824073",
            "--validate-db",
            "--shutdown-ipc",
            ipc_path.to_str().unwrap(),
            "--metrics",
            "--metrics-host",
            "0.0.0.0",
            "--metrics-port",
            "12798",
        ];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(run_args.config, Some(config_path));
                assert_eq!(run_args.topology, Some(topology_path));
                assert_eq!(run_args.database_path, Some(db_path));
                assert_eq!(run_args.socket_path, Some(socket_path));
                assert_eq!(
                    run_args.host_addr,
                    Some(SocketAddr::from_str("127.0.0.1:3001").unwrap())
                );
                assert_eq!(run_args.port, Some(3001));
                assert_eq!(run_args.protocol_magic, Some(764824073));
                assert!(run_args.validate_db);
                assert_eq!(run_args.shutdown_ipc, Some(ipc_path));
                assert!(run_args.metrics);
                assert_eq!(run_args.metrics_host, Some("0.0.0.0".to_string()));
                assert_eq!(run_args.metrics_port, Some(12798));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_invalid_command() {
        let result = parse_cli(&["cardano-node", "invalid-command"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_port_argument() {
        let result = parse_cli(&["cardano-node", "run", "--port", "invalid"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_protocol_magic_argument() {
        let result = parse_cli(&["cardano-node", "run", "--protocol-magic", "invalid"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_host_addr_argument() {
        let result = parse_cli(&["cardano-node", "run", "--host-addr", "invalid-address"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_metrics_port_argument() {
        let result = parse_cli(&["cardano-node", "run", "--metrics-port", "invalid"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_port_out_of_range() {
        let result = parse_cli(&["cardano-node", "run", "--port", "65536"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_negative_port() {
        let result = parse_cli(&["cardano-node", "run", "--port", "-1"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_config_argument_value() {
        let result = parse_cli(&["cardano-node", "run", "--config"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_topology_argument_value() {
        let result = parse_cli(&["cardano-node", "run", "--topology"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_help_flag() {
        let result = <CardanoNodeCli as Parser>::try_parse_from(["cardano-node", "--help"]);
        assert!(result.is_err()); // Help flag causes early exit with error

        // The error should be help-related
        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn test_version_flag() {
        let result = <CardanoNodeCli as Parser>::try_parse_from(["cardano-node", "--version"]);
        assert!(result.is_err()); // Version flag causes early exit with error

        // The error should be version-related
        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn test_run_help_flag() {
        let result = <CardanoNodeCli as Parser>::try_parse_from(["cardano-node", "run", "--help"]);
        assert!(result.is_err()); // Help flag causes early exit with error

        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn test_unknown_flag() {
        let result =
            <CardanoNodeCli as Parser>::try_parse_from(["cardano-node", "run", "--unknown-flag"]);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
    }

    #[test]
    fn test_no_subcommand() {
        let result = <CardanoNodeCli as Parser>::try_parse_from(["cardano-node"]);
        assert!(result.is_err());

        let error = result.unwrap_err();
        // This could be either MissingSubcommand or DisplayHelpOnMissingArgumentOrSubcommand
        let is_valid_error = matches!(
            error.kind(),
            clap::error::ErrorKind::MissingSubcommand
                | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
        assert!(
            is_valid_error,
            "Expected missing subcommand error, got: {:?}",
            error.kind()
        );
    }

    #[test]
    fn test_duplicate_arguments() {
        // Test with duplicate port arguments - clap may reject this or use the last one
        let args = ["cardano-node", "run", "--port", "3001", "--port", "3002"];

        let result = parse_cli(&args);
        // Some versions of clap may reject duplicate arguments
        if let Ok(cli) = result {
            match cli.command {
                Commands::Run(run_args) => {
                    assert_eq!(run_args.port, Some(3002)); // Last value should win
                }
                _ => panic!("Expected Run command"),
            }
        } else if let Err(error) = result {
            // It's also acceptable for clap to reject duplicate arguments
            let error_msg = format!("{}", error);
            assert!(
                error_msg.contains("argument")
                    || error_msg.contains("conflict")
                    || error_msg.contains("values")
                    || error_msg.contains("multiple"),
                "Expected argument conflict error, got: {}",
                error_msg
            );
        }
    }

    #[test]
    fn test_boolean_flag_multiple_times() {
        // Test --validate-db flag multiple times - clap may reject this
        let args = ["cardano-node", "run", "--validate-db", "--validate-db"];

        let result = parse_cli(&args);
        // Some versions of clap may reject duplicate boolean flags
        if let Ok(cli) = result {
            match cli.command {
                Commands::Run(run_args) => {
                    assert!(run_args.validate_db);
                }
                _ => panic!("Expected Run command"),
            }
        } else if let Err(error) = result {
            // It's also acceptable for clap to reject duplicate flags
            let error_msg = format!("{}", error);
            assert!(
                error_msg.contains("argument")
                    || error_msg.contains("conflict")
                    || error_msg.contains("values")
                    || error_msg.contains("multiple"),
                "Expected argument conflict error, got: {}",
                error_msg
            );
        }
    }

    #[test]
    fn test_ipv6_host_addr() {
        let ipv6_addr = "[::1]:3001";
        let args = ["cardano-node", "run", "--host-addr", ipv6_addr];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(
                    run_args.host_addr,
                    Some(SocketAddr::from_str(ipv6_addr).unwrap())
                );
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_zero_port() {
        let result = parse_cli(&["cardano-node", "run", "--port", "0"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(run_args.port, Some(0));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_max_port() {
        let result = parse_cli(&["cardano-node", "run", "--port", "65535"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(run_args.port, Some(65535));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_protocol_magic_max_value() {
        let max_u32 = u32::MAX.to_string();
        let result = parse_cli(&["cardano-node", "run", "--protocol-magic", &max_u32]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(run_args.protocol_magic, Some(u32::MAX));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_protocol_magic_zero() {
        let result = parse_cli(&["cardano-node", "run", "--protocol-magic", "0"]);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(run_args.protocol_magic, Some(0));
            }
            _ => panic!("Expected Run command"),
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_realistic_mainnet_configuration() {
        // Test a realistic mainnet configuration
        let args = [
            "cardano-node",
            "run",
            "--config",
            "/opt/cardano/config/mainnet-config.json",
            "--topology",
            "/opt/cardano/config/mainnet-topology.json",
            "--database-path",
            "/opt/cardano/data",
            "--socket-path",
            "/opt/cardano/ipc/node.socket",
            "--host-addr",
            "0.0.0.0:3001",
            "--protocol-magic",
            "764824073",
            "--validate-db",
            "--metrics",
            "--metrics-host",
            "127.0.0.1",
            "--metrics-port",
            "12798",
        ];

        let result = parse_cli_no_validation(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(
                    run_args.config,
                    Some(PathBuf::from("/opt/cardano/config/mainnet-config.json"))
                );
                assert_eq!(
                    run_args.topology,
                    Some(PathBuf::from("/opt/cardano/config/mainnet-topology.json"))
                );
                assert_eq!(
                    run_args.database_path,
                    Some(PathBuf::from("/opt/cardano/data"))
                );
                assert_eq!(
                    run_args.socket_path,
                    Some(PathBuf::from("/opt/cardano/ipc/node.socket"))
                );
                assert_eq!(
                    run_args.host_addr,
                    Some(SocketAddr::from_str("0.0.0.0:3001").unwrap())
                );
                assert_eq!(run_args.protocol_magic, Some(764824073));
                assert!(run_args.validate_db);
                assert!(run_args.metrics);
                assert_eq!(run_args.metrics_host, Some("127.0.0.1".to_string()));
                assert_eq!(run_args.metrics_port, Some(12798));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_testnet_configuration() {
        // Test a testnet configuration
        let args = [
            "cardano-node",
            "run",
            "--config",
            "/opt/cardano/config/testnet-config.json",
            "--topology",
            "/opt/cardano/config/testnet-topology.json",
            "--database-path",
            "/tmp/cardano-testnet-db",
            "--socket-path",
            "/tmp/cardano-testnet.socket",
            "--port",
            "3002",
            "--protocol-magic",
            "1097911063",
        ];

        let result = parse_cli_no_validation(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert_eq!(
                    run_args.config,
                    Some(PathBuf::from("/opt/cardano/config/testnet-config.json"))
                );
                assert_eq!(
                    run_args.topology,
                    Some(PathBuf::from("/opt/cardano/config/testnet-topology.json"))
                );
                assert_eq!(
                    run_args.database_path,
                    Some(PathBuf::from("/tmp/cardano-testnet-db"))
                );
                assert_eq!(
                    run_args.socket_path,
                    Some(PathBuf::from("/tmp/cardano-testnet.socket"))
                );
                assert_eq!(run_args.port, Some(3002));
                assert_eq!(run_args.protocol_magic, Some(1097911063));
                assert!(!run_args.validate_db);
                assert!(!run_args.metrics);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_development_configuration() {
        // Test a minimal development configuration
        let args = [
            "cardano-node",
            "run",
            "--database-path",
            "./dev-db",
            "--socket-path",
            "./dev-node.socket",
            "--port",
            "3003",
            "--validate-db",
        ];

        let result = parse_cli(&args);
        assert!(result.is_ok());

        let cli = result.unwrap();
        match cli.command {
            Commands::Run(run_args) => {
                assert!(run_args.config.is_none());
                assert!(run_args.topology.is_none());
                assert_eq!(run_args.database_path, Some(PathBuf::from("./dev-db")));
                assert_eq!(
                    run_args.socket_path,
                    Some(PathBuf::from("./dev-node.socket"))
                );
                assert_eq!(run_args.port, Some(3003));
                assert!(run_args.validate_db);
            }
            _ => panic!("Expected Run command"),
        }
    }
}
