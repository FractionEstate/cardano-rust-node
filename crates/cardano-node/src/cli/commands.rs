/// Extended CLI command structures for cardano-node
use clap::Parser;
use std::path::PathBuf;

/// Arguments for query command
#[derive(Parser, Debug, Clone)]
pub struct QueryArgs {
    #[command(subcommand)]
    pub command: QueryCommands,

    /// Socket path to connect to the node
    #[arg(long, help = "Path to the node socket")]
    pub socket_path: Option<PathBuf>,
}

#[derive(Parser, Debug, Clone)]
pub enum QueryCommands {
    /// Query the current chain tip
    ChainTip,

    /// Query protocol parameters
    ProtocolParameters {
        /// Output file path
        #[arg(long, help = "Write output to file")]
        out_file: Option<PathBuf>,
    },

    /// Query UTxOs for an address
    Utxo {
        /// Cardano address to query
        #[arg(help = "Cardano address")]
        address: String,

        /// Output file path
        #[arg(long, help = "Write output to file")]
        out_file: Option<PathBuf>,
    },

    /// Query stake pool information
    StakePool {
        /// Pool ID
        #[arg(help = "Stake pool ID")]
        pool_id: String,
    },

    /// Query ledger state
    LedgerState {
        /// Output file path
        #[arg(long, help = "Write output to file")]
        out_file: Option<PathBuf>,
    },

    /// Query stake distribution
    StakeDistribution,

    /// Query leadership schedule
    LeadershipSchedule {
        /// Stake pool ID
        #[arg(help = "Stake pool ID")]
        pool_id: String,

        /// VRF signing key file
        #[arg(long, help = "VRF signing key file path")]
        vrf_signing_key_file: PathBuf,
    },
}

/// Arguments for transaction command
#[derive(Parser, Debug, Clone)]
pub struct TransactionArgs {
    #[command(subcommand)]
    pub command: TransactionCommands,
}

#[derive(Parser, Debug, Clone)]
pub enum TransactionCommands {
    /// Build a transaction
    Build {
        /// Transaction input (TxHash#TxIx)
        #[arg(long, help = "Transaction input")]
        tx_in: Vec<String>,

        /// Transaction output (Address+Lovelace)
        #[arg(long, help = "Transaction output")]
        tx_out: Vec<String>,

        /// Change address
        #[arg(long, help = "Change address")]
        change_address: Option<String>,

        /// Output file
        #[arg(long, help = "Output transaction file")]
        out_file: PathBuf,

        /// Protocol parameters file
        #[arg(long, help = "Protocol parameters JSON file")]
        protocol_params_file: Option<PathBuf>,
    },

    /// Sign a transaction
    Sign {
        /// Transaction body file
        #[arg(long, help = "Transaction body file")]
        tx_body_file: PathBuf,

        /// Signing key files
        #[arg(long, help = "Signing key file")]
        signing_key_file: Vec<PathBuf>,

        /// Output file
        #[arg(long, help = "Signed transaction output file")]
        out_file: PathBuf,
    },

    /// Submit a transaction
    Submit {
        /// Signed transaction file
        #[arg(long, help = "Signed transaction file")]
        tx_file: PathBuf,

        /// Socket path
        #[arg(long, help = "Node socket path")]
        socket_path: Option<PathBuf>,
    },

    /// Calculate transaction ID
    TxId {
        /// Transaction body or signed tx file
        #[arg(long, help = "Transaction file")]
        tx_file: PathBuf,
    },

    /// View transaction
    View {
        /// Transaction file
        #[arg(help = "Transaction file path")]
        tx_file: PathBuf,
    },
}

/// Arguments for stake pool command
#[derive(Parser, Debug, Clone)]
pub struct StakePoolArgs {
    #[command(subcommand)]
    pub command: StakePoolCommands,
}

#[derive(Parser, Debug, Clone)]
pub enum StakePoolCommands {
    /// Register a stake pool
    Register {
        /// Pool registration certificate file
        #[arg(long, help = "Pool registration certificate")]
        pool_registration_cert: PathBuf,

        /// Pool owner signing keys
        #[arg(long, help = "Pool owner signing key")]
        signing_key_file: Vec<PathBuf>,

        /// Output transaction file
        #[arg(long, help = "Output file")]
        out_file: PathBuf,
    },

    /// Deregister a stake pool
    Deregister {
        /// Pool ID
        #[arg(help = "Pool ID to deregister")]
        pool_id: String,

        /// Epoch to retire in
        #[arg(long, help = "Retirement epoch")]
        epoch: u64,

        /// Output file
        #[arg(long, help = "Output file")]
        out_file: PathBuf,
    },

    /// Create pool metadata hash
    MetadataHash {
        /// Metadata JSON file
        #[arg(help = "Pool metadata JSON file")]
        metadata_file: PathBuf,
    },

    /// Generate pool ID from verification key
    Id {
        /// Cold verification key file
        #[arg(long, help = "Cold verification key file")]
        cold_verification_key_file: PathBuf,
    },
}

/// Arguments for stake address command
#[derive(Parser, Debug, Clone)]
pub struct StakeAddressArgs {
    #[command(subcommand)]
    pub command: StakeAddressCommands,
}

#[derive(Parser, Debug, Clone)]
pub enum StakeAddressCommands {
    /// Build a stake address
    Build {
        /// Stake verification key file
        #[arg(long, help = "Stake verification key file")]
        stake_verification_key_file: PathBuf,

        /// Network ID (mainnet, testnet, etc.)
        #[arg(long, help = "Network ID")]
        network_id: String,

        /// Output file
        #[arg(long, help = "Output file")]
        out_file: Option<PathBuf>,
    },

    /// Register stake address
    Register {
        /// Stake address
        #[arg(help = "Stake address")]
        stake_address: String,

        /// Key deposit amount
        #[arg(long, help = "Key deposit in lovelace")]
        key_deposit: u64,

        /// Output certificate file
        #[arg(long, help = "Output certificate file")]
        out_file: PathBuf,
    },

    /// Deregister stake address
    Deregister {
        /// Stake address
        #[arg(help = "Stake address")]
        stake_address: String,

        /// Output certificate file
        #[arg(long, help = "Output certificate file")]
        out_file: PathBuf,
    },

    /// Delegate stake
    Delegate {
        /// Stake address
        #[arg(help = "Stake address")]
        stake_address: String,

        /// Pool ID to delegate to
        #[arg(long, help = "Target stake pool ID")]
        stake_pool_id: String,

        /// Output certificate file
        #[arg(long, help = "Output certificate file")]
        out_file: PathBuf,
    },
}

/// Arguments for address command
#[derive(Parser, Debug, Clone)]
pub struct AddressArgs {
    #[command(subcommand)]
    pub command: AddressCommands,
}

#[derive(Parser, Debug, Clone)]
pub enum AddressCommands {
    /// Build a payment address
    Build {
        /// Payment verification key file
        #[arg(long, help = "Payment verification key file")]
        payment_verification_key_file: Option<PathBuf>,

        /// Stake verification key file
        #[arg(long, help = "Stake verification key file")]
        stake_verification_key_file: Option<PathBuf>,

        /// Network ID
        #[arg(long, help = "Network ID (mainnet, testnet, etc.)")]
        network_id: String,

        /// Output file
        #[arg(long, help = "Output file")]
        out_file: Option<PathBuf>,
    },

    /// Get address info
    Info {
        /// Address to analyze
        #[arg(help = "Cardano address")]
        address: String,
    },
}

/// Arguments for governance command (Conway era)
#[derive(Parser, Debug, Clone)]
pub struct GovernanceArgs {
    #[command(subcommand)]
    pub command: GovernanceCommands,
}

#[derive(Parser, Debug, Clone)]
pub enum GovernanceCommands {
    /// Create a governance action
    CreateAction {
        /// Action type (parameter-change, hard-fork, treasury, etc.)
        #[arg(long, help = "Governance action type")]
        action_type: String,

        /// Anchor URL
        #[arg(long, help = "Anchor URL for governance action")]
        anchor_url: Option<String>,

        /// Anchor hash
        #[arg(long, help = "Anchor data hash")]
        anchor_hash: Option<String>,

        /// Output file
        #[arg(long, help = "Output file")]
        out_file: PathBuf,
    },

    /// Vote on a governance action
    Vote {
        /// Governance action ID
        #[arg(help = "Governance action ID")]
        action_id: String,

        /// Vote (yes, no, abstain)
        #[arg(long, help = "Vote choice")]
        vote: String,

        /// Voter key file
        #[arg(long, help = "Voter signing key file")]
        signing_key_file: PathBuf,

        /// Output file
        #[arg(long, help = "Output vote file")]
        out_file: PathBuf,
    },

    /// Query governance state
    Query {
        /// Socket path
        #[arg(long, help = "Node socket path")]
        socket_path: Option<PathBuf>,
    },
}

/// Arguments for dashboard command
#[derive(Parser, Debug, Clone)]
pub struct DashboardArgs {
    /// Socket path to connect to the node
    #[arg(long, help = "Path to the node socket")]
    pub socket_path: Option<PathBuf>,

    /// Refresh interval in seconds
    #[arg(long, default_value = "2", help = "Dashboard refresh interval")]
    pub refresh_interval: u64,
}

/// Arguments for admin command
#[derive(Parser, Debug, Clone)]
pub struct AdminArgs {
    #[command(subcommand)]
    pub command: AdminCommands,
}

#[derive(Parser, Debug, Clone)]
pub enum AdminCommands {
    /// Shutdown the node gracefully
    Shutdown {
        /// Socket path
        #[arg(long, help = "Node socket path")]
        socket_path: Option<PathBuf>,
    },

    /// Restart the node
    Restart {
        /// Socket path
        #[arg(long, help = "Node socket path")]
        socket_path: Option<PathBuf>,
    },

    /// Export node metrics
    Metrics {
        /// Output format (json, prometheus)
        #[arg(long, default_value = "json", help = "Output format")]
        format: String,

        /// Output file
        #[arg(long, help = "Output file")]
        out_file: Option<PathBuf>,
    },

    /// Database operations
    Db {
        #[command(subcommand)]
        command: DbCommands,
    },
}

#[derive(Parser, Debug, Clone)]
pub enum DbCommands {
    /// Validate database integrity
    Validate {
        /// Database path
        #[arg(help = "Database directory path")]
        db_path: PathBuf,
    },

    /// Compact database
    Compact {
        /// Database path
        #[arg(help = "Database directory path")]
        db_path: PathBuf,
    },

    /// Export database snapshot
    Export {
        /// Database path
        #[arg(help = "Database directory path")]
        db_path: PathBuf,

        /// Output file
        #[arg(long, help = "Output snapshot file")]
        out_file: PathBuf,
    },

    /// Import database snapshot
    Import {
        /// Snapshot file
        #[arg(help = "Snapshot file path")]
        snapshot_file: PathBuf,

        /// Database path
        #[arg(long, help = "Target database directory")]
        db_path: PathBuf,
    },
}
