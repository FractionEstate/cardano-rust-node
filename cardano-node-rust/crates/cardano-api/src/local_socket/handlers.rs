//! Local Socket Request Handlers
//!
//! Handles incoming requests from the Unix domain socket and routes them
//! to appropriate blockchain query or transaction submission handlers.

use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, warn};

use super::protocol::*;
use crate::Result;

/// Trait for handling blockchain queries
#[async_trait::async_trait]
pub trait ChainQueryHandler: Send + Sync {
    /// Get the current chain tip
    async fn get_chain_tip(&self) -> Result<Value>;

    /// Get a block by hash
    async fn get_block(&self, block_hash: &str) -> Result<Option<Value>>;

    /// Get a transaction by ID
    async fn get_transaction(&self, tx_id: &str) -> Result<Option<Value>>;

    /// Get UTXOs for an address
    async fn get_utxos(&self, address: &str) -> Result<Value>;

    /// Get protocol parameters
    async fn get_protocol_params(&self) -> Result<Value>;

    /// Get stake pools
    async fn get_stake_pools(&self) -> Result<Value>;

    /// Get delegation information for an address
    async fn get_delegation(&self, address: &str) -> Result<Option<Value>>;

    /// Get node status
    async fn get_node_status(&self) -> Result<Value>;
}

/// Trait for handling transaction submission
#[async_trait::async_trait]
pub trait TransactionSubmissionHandler: Send + Sync {
    /// Submit a transaction
    async fn submit_transaction(&self, tx_data: &Value) -> Result<Value>;
}

/// Container for all local socket handlers
pub struct LocalSocketHandlers {
    chain_query: Arc<dyn ChainQueryHandler>,
    tx_submission: Arc<dyn TransactionSubmissionHandler>,
}

impl LocalSocketHandlers {
    /// Create new handlers
    pub fn new(
        chain_query: Arc<dyn ChainQueryHandler>,
        tx_submission: Arc<dyn TransactionSubmissionHandler>,
    ) -> Self {
        Self {
            chain_query,
            tx_submission,
        }
    }

    /// Handle an incoming request string
    pub async fn handle_request(&self, request_str: &str) -> Value {
        // Parse request
        let request: LocalSocketRequest = match serde_json::from_str(request_str.trim()) {
            Ok(req) => req,
            Err(e) => {
                debug!("Failed to parse request: {}", e);
                return serde_json::to_value(LocalSocketResponse::error(
                    LocalSocketError::parse_error(),
                    None,
                ))
                .unwrap_or(json!({}));
            }
        };

        // Parse method
        let method = match request.method.parse::<LocalSocketMethod>() {
            Ok(m) => m,
            Err(_) => {
                warn!("Unknown method: {}", request.method);
                return serde_json::to_value(LocalSocketResponse::error(
                    LocalSocketError::method_not_found(),
                    request.id,
                ))
                .unwrap_or(json!({}));
            }
        };

        // Handle request
        let result = self.handle_method(method, request.params).await;

        // Create response
        let response = match result {
            Ok(value) => LocalSocketResponse::success(value, request.id),
            Err(e) => LocalSocketResponse::error(
                LocalSocketError::internal_error(e.to_string()),
                request.id,
            ),
        };

        serde_json::to_value(response).unwrap_or(json!({}))
    }

    /// Handle a specific method
    async fn handle_method(
        &self,
        method: LocalSocketMethod,
        params: Option<Value>,
    ) -> Result<Value> {
        match method {
            LocalSocketMethod::QueryChainTip => self.chain_query.get_chain_tip().await,
            LocalSocketMethod::QueryBlock => {
                let block_hash = params
                    .as_ref()
                    .and_then(|p| p.get("blockHash"))
                    .and_then(|h| h.as_str())
                    .ok_or_else(|| {
                        crate::ApiError::RequestError("Block hash parameter required".to_string())
                    })?;

                match self.chain_query.get_block(block_hash).await? {
                    Some(block) => Ok(block),
                    None => Err(crate::ApiError::RequestError("Block not found".to_string())),
                }
            }
            LocalSocketMethod::QueryTransaction => {
                let tx_id = params
                    .as_ref()
                    .and_then(|p| p.get("txId"))
                    .and_then(|t| t.as_str())
                    .ok_or_else(|| {
                        crate::ApiError::RequestError(
                            "Transaction ID parameter required".to_string(),
                        )
                    })?;

                match self.chain_query.get_transaction(tx_id).await? {
                    Some(tx) => Ok(tx),
                    None => Err(crate::ApiError::RequestError(
                        "Transaction not found".to_string(),
                    )),
                }
            }
            LocalSocketMethod::QueryUtxos => {
                let address = params
                    .as_ref()
                    .and_then(|p| p.get("address"))
                    .and_then(|a| a.as_str())
                    .ok_or_else(|| {
                        crate::ApiError::RequestError("Address parameter required".to_string())
                    })?;

                self.chain_query.get_utxos(address).await
            }
            LocalSocketMethod::SubmitTransaction => {
                let tx_data = params.ok_or_else(|| {
                    crate::ApiError::RequestError("Transaction data required".to_string())
                })?;

                self.tx_submission.submit_transaction(&tx_data).await
            }
            LocalSocketMethod::QueryProtocolParams => self.chain_query.get_protocol_params().await,
            LocalSocketMethod::QueryStakePools => self.chain_query.get_stake_pools().await,
            LocalSocketMethod::QueryDelegation => {
                let address = params
                    .as_ref()
                    .and_then(|p| p.get("address"))
                    .and_then(|a| a.as_str())
                    .ok_or_else(|| {
                        crate::ApiError::RequestError("Address parameter required".to_string())
                    })?;

                match self.chain_query.get_delegation(address).await? {
                    Some(delegation) => Ok(delegation),
                    None => Err(crate::ApiError::RequestError(
                        "No delegation found for address".to_string(),
                    )),
                }
            }
            LocalSocketMethod::QueryNodeStatus => self.chain_query.get_node_status().await,
        }
    }
}

/// Default implementation for testing
pub struct MockChainQueryHandler;

#[async_trait::async_trait]
impl ChainQueryHandler for MockChainQueryHandler {
    async fn get_chain_tip(&self) -> Result<Value> {
        Ok(json!({
            "blockHash": "a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890",
            "slotNo": 123456789,
            "epochNo": 456,
            "blockNo": 987654
        }))
    }

    async fn get_block(&self, _block_hash: &str) -> Result<Option<Value>> {
        Ok(Some(json!({
            "hash": "a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890",
            "previousHash": "b2c3d4e5f6789012345678901234567890123456789012345678901234567890a1",
            "slotNo": 123456788,
            "blockNo": 987654,
            "epoch": 456,
            "transactions": []
        })))
    }

    async fn get_transaction(&self, _tx_id: &str) -> Result<Option<Value>> {
        Ok(Some(json!({
            "id": "tx123456789abcdef",
            "inputs": [],
            "outputs": [],
            "fee": 174593,
            "size": 256
        })))
    }

    async fn get_utxos(&self, _address: &str) -> Result<Value> {
        Ok(json!([
            {
                "txId": "tx123456789abcdef",
                "outputIndex": 0,
                "address": "addr1qx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgse35a3x",
                "amount": 1500000,
                "assets": []
            }
        ]))
    }

    async fn get_protocol_params(&self) -> Result<Value> {
        Ok(json!({
            "protocolVersion": {"major": 8, "minor": 0},
            "minFeeA": 44,
            "minFeeB": 155381,
            "maxBlockSize": 90112,
            "maxTxSize": 16384,
            "maxBlockHeaderSize": 1100
        }))
    }

    async fn get_stake_pools(&self) -> Result<Value> {
        Ok(json!([]))
    }

    async fn get_delegation(&self, _address: &str) -> Result<Option<Value>> {
        Ok(Some(json!({
            "poolId": "pool1abc123def456",
            "rewards": 1234567
        })))
    }

    async fn get_node_status(&self) -> Result<Value> {
        Ok(json!({
            "syncProgress": 100.0,
            "networkId": "mainnet",
            "protocolVersion": {"major": 8, "minor": 0},
            "uptime": 3600
        }))
    }
}

/// Default implementation for testing
pub struct MockTransactionSubmissionHandler;

#[async_trait::async_trait]
impl TransactionSubmissionHandler for MockTransactionSubmissionHandler {
    async fn submit_transaction(&self, _tx_data: &Value) -> Result<Value> {
        Ok(json!({
            "txId": "submitted_tx_123456789abcdef",
            "status": "accepted"
        }))
    }
}

impl Default for LocalSocketHandlers {
    fn default() -> Self {
        Self::new(
            Arc::new(MockChainQueryHandler),
            Arc::new(MockTransactionSubmissionHandler),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_chain_tip_query() {
        let handlers = LocalSocketHandlers::default();
        let request = r#"{"method": "queryChainTip", "id": 1}"#;

        let response = handlers.handle_request(request).await;
        assert!(response.get("result").is_some());
        assert!(response.get("error").is_none());
    }

    #[tokio::test]
    async fn test_invalid_method() {
        let handlers = LocalSocketHandlers::default();
        let request = r#"{"method": "invalidMethod", "id": 1}"#;

        let response = handlers.handle_request(request).await;
        assert!(response.get("result").is_none());
        assert!(response.get("error").is_some());
    }

    #[tokio::test]
    async fn test_missing_parameters() {
        let handlers = LocalSocketHandlers::default();
        let request = r#"{"method": "queryBlock", "id": 1}"#;

        let response = handlers.handle_request(request).await;
        assert!(response.get("error").is_some());
    }
}
