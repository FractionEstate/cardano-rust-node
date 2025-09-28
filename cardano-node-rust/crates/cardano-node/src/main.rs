//! Cardano Node Main Executable
//!
//! Entry point for the Cardano Node Rust implementation.

use anyhow::Result;
use tracing::info;
use cardano_node::{parse_cli, Commands, run_node};

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

            run_node(args).await?;
        }
        Commands::Version(version_args) => {
            if version_args.detailed {
                println!("cardano-node {}", env!("CARGO_PKG_VERSION"));
                println!("Git commit: {}", option_env!("GIT_HASH").unwrap_or("unknown"));
                println!("Build date: {}", option_env!("BUILD_DATE").unwrap_or("unknown"));
                println!("Rust version: {}", option_env!("RUSTC_VERSION").unwrap_or("unknown"));
                println!("Target: {}", option_env!("TARGET").unwrap_or("unknown"));
            } else {
                println!("cardano-node {}", env!("CARGO_PKG_VERSION"));
            }
        }
        Commands::Validate(validate_args) => {
            info!("Validating configuration files");
            // TODO: Implement configuration validation
            println!("Configuration validation completed");
        }
        Commands::Info(info_args) => {
            info!("Showing node information");
            // TODO: Implement info display
            println!("Node information displayed");
        }
    }

    Ok(())
}
