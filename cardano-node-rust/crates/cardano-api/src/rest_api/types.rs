//! REST API Type Definitions
//!
//! Common types and structures used in the REST API responses,
//! matching the OpenAPI specification in contracts/api.yaml.

use serde::{Deserialize, Serialize};

/// Error response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    /// Error message
    pub message: String,
    /// Error code
    pub code: Option<String>,
    /// Additional error details
    pub details: Option<serde_json::Value>,
}

/// Chain tip information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainTip {
    /// Block hash
    pub block_hash: String,
    /// Slot number
    pub slot_no: u64,
    /// Epoch number
    pub epoch_no: u32,
    /// Block number
    pub block_no: u64,
}

/// Block information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    /// Block hash
    pub hash: String,
    /// Previous block hash
    pub previous_hash: Option<String>,
    /// Slot number
    pub slot_no: u64,
    /// Block number
    pub block_no: u64,
    /// Epoch number
    pub epoch: u32,
    /// Block size in bytes
    pub size: u32,
    /// Number of transactions
    pub transaction_count: u32,
    /// List of transaction IDs
    pub transactions: Vec<String>,
}

/// Transaction information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    /// Transaction ID
    pub id: String,
    /// Block hash containing this transaction
    pub block_hash: Option<String>,
    /// Transaction inputs
    pub inputs: Vec<TxInput>,
    /// Transaction outputs
    pub outputs: Vec<TxOutput>,
    /// Transaction fee in lovelace
    pub fee: u64,
    /// Transaction size in bytes
    pub size: u32,
    /// Script size in bytes
    pub script_size: u32,
}

/// Transaction input
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxInput {
    /// Referenced transaction ID
    pub tx_id: String,
    /// Referenced output index
    pub output_index: u32,
    /// Input amount in lovelace
    pub amount: u64,
    /// Input address
    pub address: String,
    /// Native assets
    pub assets: Vec<Asset>,
}

/// Transaction output
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxOutput {
    /// Output address
    pub address: String,
    /// Output amount in lovelace
    pub amount: u64,
    /// Native assets
    pub assets: Vec<Asset>,
    /// Datum hash (for script outputs)
    pub datum_hash: Option<String>,
}

/// UTXO information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Utxo {
    /// Transaction ID that created this UTXO
    pub tx_id: String,
    /// Output index within the transaction
    pub output_index: u32,
    /// UTXO address
    pub address: String,
    /// UTXO amount in lovelace
    pub amount: u64,
    /// Native assets
    pub assets: Vec<Asset>,
    /// Block height where this UTXO was created
    pub block_height: u64,
    /// Block hash where this UTXO was created
    pub block_hash: String,
}

/// Native asset
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    /// Asset policy ID
    pub policy_id: String,
    /// Asset name
    pub asset_name: String,
    /// Asset quantity
    pub quantity: u64,
}

/// Protocol parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolParameters {
    /// Protocol version
    pub protocol_version: ProtocolVersion,
    /// Maximum block header size
    pub max_block_header_size: u32,
    /// Maximum block size
    pub max_block_size: u32,
    /// Maximum transaction size
    pub max_tx_size: u32,
    /// Minimum fee coefficient A
    pub min_fee_a: u32,
    /// Minimum fee coefficient B
    pub min_fee_b: u32,
    /// Minimum pool cost
    pub min_pool_cost: u64,
    /// Minimum UTXO value
    pub min_utxo_value: u64,
    /// Pool deposit amount
    pub pool_deposit: u64,
    /// Stake address deposit
    pub stake_address_deposit: u64,
    /// Treasury cut ratio
    pub treasury_cut: f64,
    /// Monetary expansion rate
    pub monetary_expansion: f64,
}

/// Protocol version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolVersion {
    /// Major version
    pub major: u32,
    /// Minor version
    pub minor: u32,
}

/// Stake pool information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StakePool {
    /// Pool ID
    pub pool_id: String,
    /// Pool ticker
    pub ticker: Option<String>,
    /// Pool name
    pub name: Option<String>,
    /// Pool description
    pub description: Option<String>,
    /// Pool homepage
    pub homepage: Option<String>,
    /// Pool pledge amount
    pub pledge: u64,
    /// Pool cost per epoch
    pub cost: u64,
    /// Pool margin
    pub margin: f64,
    /// Active stake amount
    pub active_stake: u64,
    /// Number of blocks minted
    pub blocks_minted: u64,
    /// Number of delegators
    pub delegator_count: u64,
}

/// Node status information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStatus {
    /// Network identifier
    pub network_id: String,
    /// Protocol version
    pub protocol_version: ProtocolVersion,
    /// Sync progress percentage
    pub sync_progress: f64,
    /// Current block height
    pub block_height: u64,
    /// Current slot number
    pub slot_no: u64,
    /// Current epoch number
    pub epoch_no: u32,
    /// Slot within current epoch
    pub epoch_slot: u32,
    /// Node uptime in seconds
    pub uptime: u64,
    /// Number of connected peers
    pub connected_peers: u32,
    /// Mempool information
    pub mempool: MempoolInfo,
    /// Node version
    pub node_version: String,
    /// Git commit hash
    pub commit_hash: String,
}

/// Mempool information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolInfo {
    /// Number of transactions in mempool
    pub size: u32,
    /// Total bytes of mempool transactions
    pub bytes: u64,
}

/// Transaction submission result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionSubmission {
    /// Submitted transaction ID
    pub tx_id: String,
    /// Submission status
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialization() {
        let tip = ChainTip {
            block_hash: "test_hash".to_string(),
            slot_no: 123456,
            epoch_no: 456,
            block_no: 789,
        };

        let json = serde_json::to_string(&tip).unwrap();
        let deserialized: ChainTip = serde_json::from_str(&json).unwrap();

        assert_eq!(tip.block_hash, deserialized.block_hash);
        assert_eq!(tip.slot_no, deserialized.slot_no);
    }

    #[test]
    fn test_asset_serialization() {
        let asset = Asset {
            policy_id: "policy123".to_string(),
            asset_name: "TestToken".to_string(),
            quantity: 100,
        };

        let json = serde_json::to_string(&asset).unwrap();
        assert!(json.contains("policyId"));
        assert!(json.contains("assetName"));
    }

    #[test]
    fn test_protocol_version() {
        let version = ProtocolVersion {
            major: 8,
            minor: 0,
        };

        let json = serde_json::to_string(&version).unwrap();
        let deserialized: ProtocolVersion = serde_json::from_str(&json).unwrap();

        assert_eq!(version.major, deserialized.major);
        assert_eq!(version.minor, deserialized.minor);
    }
}
