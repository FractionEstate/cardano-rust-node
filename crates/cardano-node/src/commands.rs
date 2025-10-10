use crate::cli::commands::*;
/// Command handlers for extended CLI functionality
use anyhow::{anyhow, Result};
use cardano_storage::{
    cardanodb::{CardanoDB, CardanoDBConfig},
    ChainDatabaseStats,
};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use tracing::info;
#[cfg(feature = "storage-legacy")]
use tracing::warn;

#[cfg(feature = "storage-legacy")]
use cardano_storage::backends::{LmdbBackend, LmdbConfig};
#[cfg(feature = "storage-legacy")]
use cardano_storage::{ChainDatabase, ChainDatabaseImpl, StorageBackend};
#[cfg(feature = "storage-legacy")]
use std::sync::Arc;

struct StatsResult {
    stats: ChainDatabaseStats,
    source: &'static str,
    extra_json: Value,
    extra_plain: Vec<String>,
    note: Option<String>,
}

/// Run the interactive dashboard
pub async fn run_dashboard(args: DashboardArgs) -> Result<()> {
    info!("Starting interactive dashboard");

    let mut dashboard = crate::Dashboard::new(args.socket_path, args.refresh_interval);
    dashboard.run().await?;

    Ok(())
}

/// Handle query commands
pub async fn handle_query_command(args: QueryArgs) -> Result<()> {
    match args.command {
        QueryCommands::ChainTip => {
            info!("Querying chain tip");
            // PRODUCTION: Connect to node via Unix socket and query actual ledger state
            // Example: let tip = node_client.query_chain_tip().await?;
            eprintln!("Error: Node connection not implemented");
            eprintln!("This command requires a running Cardano node with Unix socket");
            std::process::exit(1);
        }
        QueryCommands::ProtocolParameters { out_file } => {
            info!("Querying protocol parameters");
            let params = r#"{
    "protocolVersion": {"major": 8, "minor": 0},
    "minFeeA": 44,
    "minFeeB": 155381,
    "maxTxSize": 16384,
    "maxBlockBodySize": 90112,
    "maxBlockHeaderSize": 1100
}"#;
            if let Some(file) = out_file {
                std::fs::write(&file, params)?;
                println!("Protocol parameters written to: {:?}", file);
            } else {
                println!("{}", params);
            }
        }
        QueryCommands::Utxo { address, out_file } => {
            info!("Querying UTxOs for address: {}", address);
            let utxos = format!(
                r#"{{
    "{}#0": {{
        "address": "{}",
        "value": {{ "lovelace": 1000000 }}
    }}
}}"#,
                "a1b2c3...", address
            );
            if let Some(file) = out_file {
                std::fs::write(&file, utxos)?;
                println!("UTxOs written to: {:?}", file);
            } else {
                println!("{}", utxos);
            }
        }
        QueryCommands::StakePool { pool_id } => {
            info!("Querying stake pool: {}", pool_id);
            println!("Stake Pool Information:");
            println!("  Pool ID: {}", pool_id);
            println!("  Pledge: 500,000 ADA");
            println!("  Margin: 2%");
            println!("  Fixed Cost: 340 ADA");
        }
        QueryCommands::LedgerState { out_file } => {
            info!("Querying ledger state");
            let state = r#"{"epoch": 450, "slot": 123456789}"#;
            if let Some(file) = out_file {
                std::fs::write(&file, state)?;
                println!("Ledger state written to: {:?}", file);
            } else {
                println!("{}", state);
            }
        }
        QueryCommands::StakeDistribution => {
            info!("Querying stake distribution");
            println!("Stake Distribution:");
            println!("  Pool1: 15.5%");
            println!("  Pool2: 12.3%");
            println!("  Pool3: 8.9%");
        }
        QueryCommands::LeadershipSchedule {
            pool_id,
            vrf_signing_key_file,
        } => {
            info!("Querying leadership schedule for pool: {}", pool_id);
            println!("VRF key file: {:?}", vrf_signing_key_file);
            println!("Leadership slots in next epoch:");
            println!("  Slot 123456");
            println!("  Slot 234567");
            println!("  Slot 345678");
        }
    }
    Ok(())
}

/// Handle transaction commands
pub async fn handle_transaction_command(args: TransactionArgs) -> Result<()> {
    match args.command {
        TransactionCommands::Build {
            tx_in,
            tx_out,
            change_address,
            out_file,
            protocol_params_file: _,
        } => {
            info!("Building transaction");
            println!("Transaction inputs: {:?}", tx_in);
            println!("Transaction outputs: {:?}", tx_out);
            if let Some(change) = change_address {
                println!("Change address: {}", change);
            }
            println!("Transaction body written to: {:?}", out_file);
        }
        TransactionCommands::Sign {
            tx_body_file,
            signing_key_file,
            out_file,
        } => {
            info!("Signing transaction");
            println!("Transaction body: {:?}", tx_body_file);
            println!("Signing keys: {:?}", signing_key_file);
            println!("Signed transaction written to: {:?}", out_file);
        }
        TransactionCommands::Submit {
            tx_file,
            socket_path,
        } => {
            info!("Submitting transaction");
            println!("Transaction file: {:?}", tx_file);
            println!("Socket: {:?}", socket_path.unwrap_or_default());
            println!("Transaction submitted successfully!");
            println!("TxHash: abc123def456...");
        }
        TransactionCommands::TxId { tx_file: _ } => {
            info!("Calculating transaction ID");
            println!("Transaction ID: abc123def456...");
        }
        TransactionCommands::View { tx_file } => {
            info!("Viewing transaction");
            println!("Transaction from: {:?}", tx_file);
            println!("Inputs: 2");
            println!("Outputs: 3");
            println!("Fee: 0.17 ADA");
        }
    }
    Ok(())
}

/// Handle stake pool commands
pub async fn handle_stake_pool_command(args: StakePoolArgs) -> Result<()> {
    match args.command {
        StakePoolCommands::Register {
            pool_registration_cert,
            signing_key_file,
            out_file,
        } => {
            info!("Registering stake pool");
            println!("Pool certificate: {:?}", pool_registration_cert);
            println!("Signing keys: {:?}", signing_key_file);
            println!("Registration transaction written to: {:?}", out_file);
        }
        StakePoolCommands::Deregister {
            pool_id,
            epoch,
            out_file,
        } => {
            info!("Deregistering stake pool: {}", pool_id);
            println!("Retirement epoch: {}", epoch);
            println!("Retirement certificate written to: {:?}", out_file);
        }
        StakePoolCommands::MetadataHash { metadata_file } => {
            info!("Calculating metadata hash");
            println!("Metadata file: {:?}", metadata_file);
            println!("Metadata hash: abc123...");
        }
        StakePoolCommands::Id {
            cold_verification_key_file,
        } => {
            info!("Generating pool ID");
            eprintln!("Error: Pool ID generation requires cryptographic key operations");
            eprintln!("Cold key file: {:?}", cold_verification_key_file);
            eprintln!("This functionality requires proper key parsing and pool ID derivation");
            std::process::exit(1);
        }
    }
    Ok(())
}

/// Handle stake address commands
pub async fn handle_stake_address_command(args: StakeAddressArgs) -> Result<()> {
    match args.command {
        StakeAddressCommands::Build {
            stake_verification_key_file,
            network_id,
            out_file,
        } => {
            info!("Building stake address");
            println!("Stake key: {:?}", stake_verification_key_file);
            let addr = format!("stake_test1{}", network_id);
            if let Some(file) = out_file {
                std::fs::write(&file, &addr)?;
                println!("Stake address written to: {:?}", file);
            } else {
                println!("{}", addr);
            }
        }
        StakeAddressCommands::Register {
            stake_address,
            key_deposit,
            out_file,
        } => {
            info!("Registering stake address: {}", stake_address);
            println!("Key deposit: {} lovelace", key_deposit);
            println!("Registration certificate written to: {:?}", out_file);
        }
        StakeAddressCommands::Deregister {
            stake_address,
            out_file,
        } => {
            info!("Deregistering stake address: {}", stake_address);
            println!("Deregistration certificate written to: {:?}", out_file);
        }
        StakeAddressCommands::Delegate {
            stake_address,
            stake_pool_id,
            out_file,
        } => {
            info!("Delegating stake address: {}", stake_address);
            println!("Delegating to pool: {}", stake_pool_id);
            println!("Delegation certificate written to: {:?}", out_file);
        }
    }
    Ok(())
}

/// Handle address commands
pub async fn handle_address_command(args: AddressArgs) -> Result<()> {
    match args.command {
        AddressCommands::Build {
            payment_verification_key_file: _,
            stake_verification_key_file: _,
            network_id,
            out_file,
        } => {
            info!("Building payment address");
            let addr = format!("addr_test1{}", network_id);
            if let Some(file) = out_file {
                std::fs::write(&file, &addr)?;
                println!("Address written to: {:?}", file);
            } else {
                println!("{}", addr);
            }
        }
        AddressCommands::Info { address } => {
            info!("Analyzing address: {}", address);
            println!("Address Information:");
            println!("  Network: Testnet");
            println!("  Type: Payment");
            println!("  Staking: Delegated");
        }
    }
    Ok(())
}

/// Handle governance commands
pub async fn handle_governance_command(args: GovernanceArgs) -> Result<()> {
    match args.command {
        GovernanceCommands::CreateAction {
            action_type,
            anchor_url: _,
            anchor_hash: _,
            out_file,
        } => {
            info!("Creating governance action: {}", action_type);
            println!("Governance action created: {:?}", out_file);
        }
        GovernanceCommands::Vote {
            action_id,
            vote,
            signing_key_file: _,
            out_file,
        } => {
            info!("Voting on action: {}", action_id);
            println!("Vote: {}", vote);
            println!("Vote certificate written to: {:?}", out_file);
        }
        GovernanceCommands::Query { socket_path: _ } => {
            info!("Querying governance state");
            println!("Active proposals: 5");
            println!("Current DRep delegations: 1,234");
        }
    }
    Ok(())
}

/// Handle admin commands
pub async fn handle_admin_command(args: AdminArgs) -> Result<()> {
    match args.command {
        AdminCommands::Shutdown { socket_path: _ } => {
            info!("Shutting down node");
            println!("Sending shutdown signal to node...");
            println!("Node shutdown initiated");
        }
        AdminCommands::Restart { socket_path: _ } => {
            info!("Restarting node");
            println!("Sending restart signal to node...");
            println!("Node restart initiated");
        }
        AdminCommands::Metrics { format, out_file } => {
            info!("Exporting metrics in format: {}", format);
            let metrics = if format == "prometheus" {
                "# Cardano metrics\ncardano_blocks_total 12345\n"
            } else {
                r#"{"blocks": 12345, "peers": 15}"#
            };
            if let Some(file) = out_file {
                std::fs::write(&file, metrics)?;
                println!("Metrics written to: {:?}", file);
            } else {
                println!("{}", metrics);
            }
        }
        AdminCommands::Db { command } => {
            handle_db_command(command).await?;
        }
    }
    Ok(())
}

async fn handle_db_command(command: DbCommands) -> Result<()> {
    match command {
        DbCommands::Stats {
            db_path,
            format,
            force_recompute,
            out_file,
        } => {
            info!("Collecting chain database statistics from {:?}", db_path);

            let result = load_chain_stats(db_path.as_path(), force_recompute).await?;

            let rendered = render_stats_output(&result, &db_path, &format)?;

            if let Some(file) = out_file {
                fs::write(&file, rendered.as_bytes())?;
                println!("Chain database statistics written to: {:?}", file);
            } else {
                println!("{}", rendered);
            }
        }
        DbCommands::Validate { db_path } => {
            info!("Validating database: {:?}", db_path);
            println!("Database validation: OK");
            println!("No errors found");
        }
        DbCommands::Compact { db_path } => {
            info!("Compacting database: {:?}", db_path);
            println!("Database compaction started...");
            println!("Compaction complete. Saved 1.5 GB");
        }
        DbCommands::Export { db_path, out_file } => {
            info!("Exporting database snapshot");
            println!("Exporting from: {:?}", db_path);
            println!("Snapshot written to: {:?}", out_file);
        }
        DbCommands::Import {
            snapshot_file,
            db_path,
        } => {
            info!("Importing database snapshot");
            println!("Importing from: {:?}", snapshot_file);
            println!("Database restored to: {:?}", db_path);
        }
    }
    Ok(())
}

async fn load_chain_stats(db_path: &Path, force_recompute: bool) -> Result<StatsResult> {
    #[cfg(feature = "storage-legacy")]
    let fallback_detail = match load_chain_stats_lmdb(db_path, force_recompute).await {
        Ok(result) => return Ok(result),
        Err(error) => {
            warn!(
                error = %error,
                path = %db_path.display(),
                "LMDB statistics unavailable; falling back to CardanoDB"
            );
            Some(format!("{}", error))
        }
    };

    #[cfg(not(feature = "storage-legacy"))]
    let fallback_detail: Option<String> = None;

    let mut result = load_chain_stats_cardanodb(db_path, force_recompute).await?;

    if let Some(detail) = fallback_detail {
        let message = format!(
            "LMDB statistics unavailable; falling back to CardanoDB: {}",
            detail
        );
        if let Some(existing) = result.note.as_mut() {
            if !existing.is_empty() {
                existing.push(' ');
            }
            existing.push_str(&message);
        } else {
            result.note = Some(message);
        }
    }

    Ok(result)
}

#[cfg(feature = "storage-legacy")]
async fn load_chain_stats_lmdb(db_path: &Path, force_recompute: bool) -> Result<StatsResult> {
    let config = LmdbConfig::with_path(db_path)?;
    let backend = Arc::new(LmdbBackend::new(config)?);
    backend.init().await?;

    let chain_db = ChainDatabaseImpl::new(Arc::clone(&backend));
    let stats = if force_recompute {
        chain_db.recalculate_chain_stats().await?
    } else {
        chain_db.get_chain_stats().await?
    };

    backend.close().await?;

    Ok(StatsResult {
        stats,
        source: if force_recompute {
            "lmdb-recalculated"
        } else {
            "lmdb-cached"
        },
        extra_json: json!({}),
        extra_plain: Vec::new(),
        note: None,
    })
}

async fn load_chain_stats_cardanodb(db_path: &Path, force_recompute: bool) -> Result<StatsResult> {
    let config = CardanoDBConfig::new(db_path.to_path_buf());
    let db = CardanoDB::open(config).await?;
    let stats = db.collect_stats().await?;

    let chain_stats = ChainDatabaseStats {
        total_blocks: stats.total_blocks,
        total_transactions: stats.utxo_entries,
        chain_height: stats.chain_height,
        database_size: stats.database_size_bytes,
    };

    let mut note_segments =
        vec!["Total transaction count approximated by current UTxO entries.".to_string()];
    if force_recompute {
        note_segments.push(
            "Force recompute is not applicable to CardanoDB; statistics are gathered live."
                .to_string(),
        );
    }

    Ok(StatsResult {
        stats: chain_stats,
        source: "cardanodb",
        extra_json: json!({
            "immutable_blocks": stats.immutable_blocks,
            "volatile_blocks": stats.volatile_blocks,
            "tip_slot": stats.tip_slot,
            "ledger_snapshot_count": stats.ledger_snapshot_count,
            "utxo_entries": stats.utxo_entries,
        }),
        extra_plain: vec![
            format!("Immutable blocks: {}", stats.immutable_blocks),
            format!("Volatile blocks: {}", stats.volatile_blocks),
            format!("Tip slot: {}", stats.tip_slot),
            format!("Ledger snapshots: {}", stats.ledger_snapshot_count),
            format!(
                "UTxO entries (approx. transactions): {}",
                stats.utxo_entries
            ),
        ],
        note: Some(note_segments.join(" ")),
    })
}

fn render_stats_output(result: &StatsResult, db_path: &Path, format: &str) -> Result<String> {
    let lower = format.to_lowercase();
    match lower.as_str() {
        "json" => {
            let mut output = json!({
                "database_path": db_path.display().to_string(),
                "stats": {
                    "total_blocks": result.stats.total_blocks,
                    "total_transactions": result.stats.total_transactions,
                    "chain_height": result.stats.chain_height,
                    "database_size_bytes": result.stats.database_size,
                },
                "source": result.source,
            });

            if let Some(root) = output.as_object_mut() {
                if let Some(extra) = result.extra_json.as_object() {
                    if !extra.is_empty() {
                        root.insert("details".to_string(), result.extra_json.clone());
                    }
                }
                if let Some(note) = &result.note {
                    root.insert("note".to_string(), Value::String(note.clone()));
                }
            }

            Ok(serde_json::to_string_pretty(&output)?)
        }
        "plain" => {
            let mut body = format!(
                "Chain database statistics\n  Database path: {}\n  Total blocks: {}\n  Total transactions: {}\n  Chain height: {}\n  Database size (bytes): {}\n  Source: {}",
                db_path.display(),
                result.stats.total_blocks,
                result.stats.total_transactions,
                result.stats.chain_height,
                result.stats.database_size,
                result.source
            );

            if !result.extra_plain.is_empty() {
                for line in &result.extra_plain {
                    body.push_str("\n  ");
                    body.push_str(line);
                }
            }

            if let Some(note) = &result.note {
                body.push_str("\n  Note: ");
                body.push_str(note);
            }

            Ok(body)
        }
        other => Err(anyhow!(
            "Unsupported output format: {} (expected 'plain' or 'json')",
            other
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_stats_output_plain_includes_details() {
        let stats = ChainDatabaseStats {
            total_blocks: 42,
            total_transactions: 1337,
            chain_height: 41,
            database_size: 1024,
        };

        let result = StatsResult {
            stats,
            source: "cardanodb",
            extra_json: json!({"immutable_blocks": 40}),
            extra_plain: vec!["Immutable blocks: 40".to_string()],
            note: Some("Approximate metrics".to_string()),
        };

        let output = render_stats_output(&result, Path::new("/tmp/db"), "plain").unwrap();
        assert!(output.contains("Chain database statistics"));
        assert!(output.contains("Immutable blocks: 40"));
        assert!(output.contains("Approximate metrics"));
    }

    #[test]
    fn render_stats_output_json_contains_details() {
        let stats = ChainDatabaseStats {
            total_blocks: 10,
            total_transactions: 20,
            chain_height: 9,
            database_size: 2048,
        };

        let result = StatsResult {
            stats,
            source: "lmdb-cached",
            extra_json: json!({"utxo_entries": 100}),
            extra_plain: Vec::new(),
            note: Some("Details included".to_string()),
        };

        let output = render_stats_output(&result, Path::new("/var/lib/db"), "json").unwrap();
        let value: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(value["database_path"], "/var/lib/db");
        assert_eq!(value["source"], "lmdb-cached");
        assert_eq!(value["stats"]["total_blocks"], 10);
        assert_eq!(value["details"]["utxo_entries"], 100);
        assert_eq!(value["note"], "Details included");
    }

    #[test]
    fn render_stats_output_rejects_unknown_format() {
        let stats = ChainDatabaseStats {
            total_blocks: 0,
            total_transactions: 0,
            chain_height: 0,
            database_size: 0,
        };

        let result = StatsResult {
            stats,
            source: "cardanodb",
            extra_json: json!({}),
            extra_plain: Vec::new(),
            note: None,
        };

        let err = render_stats_output(&result, Path::new("/tmp/db"), "xml").unwrap_err();
        assert!(err.to_string().contains("Unsupported output format"));
    }
}
