//! Cardano Node Main Executable
//!
//! Entry point for the Cardano Node Rust implementation.

use anyhow::Result;
use cardano_node::{parse_cli, run_node, Commands};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = parse_cli()?;

    match cli.command {
        Commands::Run(args) => {
            info!("Starting Cardano Node");
            info!("Configuration: {:?}", args.config);
            info!("Topology: {:?}", args.topology);
            info!("Database path: {:?}", args.database_path);
            info!("Socket path: {:?}", args.socket_path);

            run_node(*args).await?;
        }
        Commands::Version(version_args) => {
            if version_args.detailed {
                println!("cardano-node {}", env!("CARGO_PKG_VERSION"));
                println!(
                    "Git commit: {}",
                    option_env!("GIT_HASH").unwrap_or("unknown")
                );
                println!(
                    "Build date: {}",
                    option_env!("BUILD_DATE").unwrap_or("unknown")
                );
                println!(
                    "Rust version: {}",
                    option_env!("RUSTC_VERSION").unwrap_or("unknown")
                );
                println!("Target: {}", option_env!("TARGET").unwrap_or("unknown"));
            } else {
                println!("cardano-node {}", env!("CARGO_PKG_VERSION"));
            }
        }
        Commands::Validate(validate_args) => {
            info!("Validating configuration files");
            // Basic configuration file validation
            if let Some(config) = &validate_args.config {
                match std::fs::read_to_string(config) {
                    Ok(content) => {
                        // Attempt to parse as JSON
                        match serde_json::from_str::<serde_json::Value>(&content) {
                            Ok(_) => {
                                println!("✓ Configuration file is valid JSON");
                                println!("Configuration validation completed successfully");
                            }
                            Err(e) => {
                                eprintln!("✗ Configuration file is invalid: {}", e);
                                std::process::exit(1);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to read configuration file: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                println!("✓ No configuration file specified, skipping validation");
            }
        }
        Commands::Info(info_args) => {
            info!("Showing node information");

            let format_json = info_args.format == "json";

            if format_json {
                let mut info_obj = serde_json::json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "build_type": if cfg!(debug_assertions) { "debug" } else { "release" },
                    "platform": std::env::consts::OS,
                    "architecture": std::env::consts::ARCH,
                });

                if info_args.protocol {
                    info_obj["protocol"] = serde_json::json!({
                        "versions_supported": ["byron", "shelley", "allegra", "mary", "alonzo", "babbage", "conway"],
                        "current": "conway",
                        "major": 8,
                        "minor": 0
                    });
                }

                if info_args.network {
                    info_obj["network"] = serde_json::json!({
                        "name": "mainnet",
                        "magic": 764824073
                    });
                }

                if info_args.build {
                    info_obj["build"] = serde_json::json!({
                        "compiler": "rustc",
                        "version": env!("CARGO_PKG_VERSION"),
                        "features": ["async", "tokio"]
                    });
                }

                println!("{}", serde_json::to_string_pretty(&info_obj).unwrap());
            } else {
                // Plain text output
                println!("Cardano Node - Rust Implementation");
                println!("==================================");
                println!();
                println!("Version:        {}", env!("CARGO_PKG_VERSION"));
                println!("Build:          {} ({})",
                    env!("CARGO_PKG_VERSION"),
                    if cfg!(debug_assertions) { "debug" } else { "release" }
                );
                println!("Platform:       {}", std::env::consts::OS);
                println!("Architecture:   {}", std::env::consts::ARCH);

                if info_args.protocol {
                    println!();
                    println!("Protocol Information:");
                    println!("  Supported Eras:  Byron, Shelley, Allegra, Mary, Alonzo, Babbage, Conway");
                    println!("  Current Era:     Conway");
                    println!("  Protocol Version: 8.0");
                }

                if info_args.network {
                    println!();
                    println!("Network Information:");
                    println!("  Network:         Mainnet");
                    println!("  Network Magic:   764824073");
                }

                if info_args.build {
                    println!();
                    println!("Build Information:");
                    println!("  Compiler:        rustc");
                    println!("  Features:        async, tokio");
                }

                println!();
                println!("For runtime status, use: cardano-node dashboard");
            }
        }
        Commands::Query(query_args) => {
            info!("Executing query command");
            cardano_node::handle_query_command(query_args).await?;
        }
        Commands::Transaction(tx_args) => {
            info!("Executing transaction command");
            cardano_node::handle_transaction_command(tx_args).await?;
        }
        Commands::StakePool(pool_args) => {
            info!("Executing stake pool command");
            cardano_node::handle_stake_pool_command(pool_args).await?;
        }
        Commands::StakeAddress(addr_args) => {
            info!("Executing stake address command");
            cardano_node::handle_stake_address_command(addr_args).await?;
        }
        Commands::Address(addr_args) => {
            info!("Executing address command");
            cardano_node::handle_address_command(addr_args).await?;
        }
        Commands::Governance(gov_args) => {
            info!("Executing governance command");
            cardano_node::handle_governance_command(gov_args).await?;
        }
        Commands::Dashboard(dash_args) => {
            info!("Starting interactive dashboard");
            cardano_node::run_dashboard(dash_args).await?;
        }
        Commands::Admin(admin_args) => {
            info!("Executing admin command");
            cardano_node::handle_admin_command(admin_args).await?;
        }
    }

    Ok(())
}
