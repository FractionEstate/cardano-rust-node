//! REST API Handler Traits
//!
//! Defines the traits for handling REST API requests for blockchain
//! queries and transaction submission.

use serde_json::Value;
use crate::Result;

/// Trait for handling blockchain queries via REST API
#[async_trait::async_trait]
pub trait ChainRestHandler: Send + Sync {
    /// Get the current chain tip
    async fn get_chain_tip(&self) -> Result<Value>;

    /// Get a block by hash
    async fn get_block(&self, block_hash: &str) -> Result<Option<Value>>;

    /// Get a transaction by ID
    async fn get_transaction(&self, tx_id: &str) -> Result<Option<Value>>;

    /// Get UTXOs for an address with optional asset filter
    async fn get_address_utxos(&self, address: &str, asset: Option<&str>) -> Result<Value>;

    /// Get protocol parameters
    async fn get_protocol_parameters(&self) -> Result<Value>;

    /// Get stake pools
    async fn get_stake_pools(&self) -> Result<Value>;

    /// Get node status
    async fn get_node_status(&self) -> Result<Value>;
}

/// Trait for handling transaction submission via REST API
#[async_trait::async_trait]
pub trait TransactionRestHandler: Send + Sync {
    /// Submit a transaction (JSON or CBOR)
    async fn submit_transaction(&self, tx_data: &Value) -> Result<Value>;
}

/// Default implementation for testing
pub struct MockChainRestHandler;

#[async_trait::async_trait]
impl ChainRestHandler for MockChainRestHandler {
    async fn get_chain_tip(&self) -> Result<Value> {
        Ok(serde_json::json!({
            "blockHash": "a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890",
            "slotNo": 123456789,
            "epochNo": 456,
            "blockNo": 987654
        }))
    }

    async fn get_block(&self, _block_hash: &str) -> Result<Option<Value>> {
        Ok(Some(serde_json::json!({
            "hash": "a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890",
            "previousHash": "b2c3d4e5f6789012345678901234567890123456789012345678901234567890a1",
            "slotNo": 123456788,
            "blockNo": 987654,
            "epoch": 456,
            "size": 1024,
            "transactionCount": 3,
            "transactions": []
        })))
    }

    async fn get_transaction(&self, _tx_id: &str) -> Result<Option<Value>> {
        Ok(Some(serde_json::json!({
            "id": "tx123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "blockHash": "a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890",
            "inputs": [
                {
                    "txId": "prev_tx_123456789abcdef",
                    "outputIndex": 0,
                    "amount": 2000000,
                    "address": "addr1qx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgse35a3x"
                }
            ],
            "outputs": [
                {
                    "address": "addr1qy2jt0qpqz2cjlqaj9dnq6qpae9jff0e6c9qt5dqc9q8lqc9w8k8j8t8t8t8t8t8t8t8t8t8t8t8t8t8t8t8t8t8t8t8",
                    "amount": 1500000,
                    "assets": []
                },
                {
                    "address": "addr1qx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgse35a3x",
                    "amount": 325407,
                    "assets": []
                }
            ],
            "fee": 174593,
            "size": 256,
            "scriptSize": 0
        })))
    }

    async fn get_address_utxos(&self, _address: &str, _asset: Option<&str>) -> Result<Value> {
        Ok(serde_json::json!([
            {
                "txId": "tx123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "outputIndex": 0,
                "address": "addr1qx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgse35a3x",
                "amount": 1500000,
                "assets": [],
                "blockHeight": 987654,
                "blockHash": "a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890"
            },
            {
                "txId": "tx987654321fedcba0987654321fedcba0987654321fedcba0987654321fedcba",
                "outputIndex": 1,
                "address": "addr1qx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgse35a3x",
                "amount": 2500000,
                "assets": [
                    {
                        "policyId": "policy123456789abcdef",
                        "assetName": "TestToken",
                        "quantity": 100
                    }
                ],
                "blockHeight": 987655,
                "blockHash": "b2c3d4e5f6789012345678901234567890123456789012345678901234567890a1"
            }
        ]))
    }

    async fn get_protocol_parameters(&self) -> Result<Value> {
        Ok(serde_json::json!({
            "protocolVersion": {
                "major": 8,
                "minor": 0
            },
            "decentralization": null,
            "extraPraosEntropy": null,
            "maxBlockHeaderSize": 1100,
            "maxBlockSize": 90112,
            "maxTxSize": 16384,
            "minFeeA": 44,
            "minFeeB": 155381,
            "minPoolCost": 340000000,
            "minUTxOValue": 1000000,
            "poolDeposit": 500000000,
            "treasuryCut": 0.2,
            "monetaryExpansion": 0.003,
            "stakeAddressDeposit": 2000000,
            "stakePoolDeposit": 500000000,
            "minPoolCost": 340000000,
            "stakePoolTargetNum": 150,
            "poolRetireMaxEpoch": 18,
            "stakePoolPledgeInfluence": 0.3,
            "utxoCostPerWord": 4310
        }))
    }

    async fn get_stake_pools(&self) -> Result<Value> {
        Ok(serde_json::json!([
            {
                "poolId": "pool1abc123def456789012345678901234567890123456789012345678901234",
                "ticker": "TEST1",
                "name": "Test Pool 1",
                "description": "A test stake pool",
                "homepage": "https://testpool1.com",
                "pledge": 1000000000000_u64,
                "cost": 340000000,
                "margin": 0.05,
                "activeStake": 50000000000000_u64,
                "blocksMinted": 1234,
                "delegatorCount": 567
            },
            {
                "poolId": "pool2def456abc789012345678901234567890123456789012345678901234567",
                "ticker": "TEST2",
                "name": "Test Pool 2",
                "description": "Another test stake pool",
                "homepage": "https://testpool2.com",
                "pledge": 2000000000000_u64,
                "cost": 340000000,
                "margin": 0.03,
                "activeStake": 75000000000000_u64,
                "blocksMinted": 2345,
                "delegatorCount": 890
            }
        ]))
    }

    async fn get_node_status(&self) -> Result<Value> {
        Ok(serde_json::json!({
            "networkId": "mainnet",
            "protocolVersion": {
                "major": 8,
                "minor": 0
            },
            "syncProgress": 100.0,
            "blockHeight": 987654,
            "slotNo": 123456789,
            "epochNo": 456,
            "epochSlot": 12345,
            "uptime": 86400,
            "connectedPeers": 42,
            "mempool": {
                "size": 156,
                "bytes": 45678
            },
            "nodeVersion": "8.7.3",
            "commitHash": "abc123def456"
        }))
    }
}

/// Default implementation for testing
pub struct MockTransactionRestHandler;

#[async_trait::async_trait]
impl TransactionRestHandler for MockTransactionRestHandler {
    async fn submit_transaction(&self, _tx_data: &Value) -> Result<Value> {
        Ok(serde_json::json!({
            "txId": "submitted_tx_123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "status": "accepted"
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_chain_handler() {
        let handler = MockChainRestHandler;

        let tip = handler.get_chain_tip().await.unwrap();
        assert!(tip.get("blockHash").is_some());

        let block = handler.get_block("test_hash").await.unwrap();
        assert!(block.is_some());

        let tx = handler.get_transaction("test_tx").await.unwrap();
        assert!(tx.is_some());

        let utxos = handler.get_address_utxos("test_addr", None).await.unwrap();
        assert!(utxos.is_array());
    }

    #[tokio::test]
    async fn test_mock_transaction_handler() {
        let handler = MockTransactionRestHandler;
        let tx_data = serde_json::json!({"test": "data"});

        let result = handler.submit_transaction(&tx_data).await.unwrap();
        assert!(result.get("txId").is_some());
    }
}
