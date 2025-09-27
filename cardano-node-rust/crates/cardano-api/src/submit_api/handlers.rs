//! Submit API Request Handlers
//!
//! HTTP and local socket request handlers for transaction submission.

use std::sync::Arc;
use serde_json::Value;
use tracing::{debug, error};

use crate::{ApiError, Result};
use super::{
    SubmitApiService, SubmissionResult, MempoolStats,
    validation::{CardanoTransactionValidator, MockUtxoProvider, MockScriptValidator},
    mempool::InMemoryMempool,
    SubmitApiConfig,
};

/// Submit API handlers for different interfaces
pub struct SubmitApiHandlers {
    service: Arc<SubmitApiService>,
}

impl SubmitApiHandlers {
    /// Create new submit API handlers
    pub fn new(service: Arc<SubmitApiService>) -> Self {
        Self { service }
    }

    /// Create handlers with default configuration
    pub fn with_default_config() -> Self {
        let config = SubmitApiConfig::default();

        let validator = Arc::new(CardanoTransactionValidator::new(
            config.clone(),
            Box::new(MockUtxoProvider),
            Box::new(MockScriptValidator),
        ));

        let mempool = Arc::new(InMemoryMempool::new(config.clone()));

        let service = Arc::new(SubmitApiService::new(config, validator, mempool));

        Self::new(service)
    }

    /// Handle transaction submission
    pub async fn handle_submit_transaction(&self, tx_data: &Value) -> Result<SubmissionResult> {
        debug!("Handling transaction submission");
        self.service.submit_transaction(tx_data).await
    }

    /// Handle transaction status query
    pub async fn handle_transaction_status(&self, tx_id: &str) -> Result<Option<SubmissionResult>> {
        debug!("Handling transaction status query for {}", tx_id);
        self.service.get_transaction_status(tx_id).await
    }

    /// Handle mempool statistics request
    pub async fn handle_mempool_stats(&self) -> Result<MempoolStats> {
        debug!("Handling mempool statistics request");
        self.service.get_mempool_stats().await
    }

    /// Handle batch transaction submission
    pub async fn handle_batch_submit(&self, transactions: &[Value]) -> Result<Vec<SubmissionResult>> {
        debug!("Handling batch transaction submission of {} transactions", transactions.len());

        let mut results = Vec::new();
        for tx_data in transactions {
            match self.service.submit_transaction(tx_data).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    error!("Failed to submit transaction in batch: {}", e);
                    // Continue with other transactions even if one fails
                    results.push(SubmissionResult {
                        tx_id: "unknown".to_string(),
                        status: super::SubmissionStatus::Rejected,
                        error: Some(e.to_string()),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    });
                }
            }
        }

        Ok(results)
    }
}

/// REST API handlers implementation
impl SubmitApiHandlers {
    /// Handle REST API transaction submission
    pub async fn rest_submit_transaction(
        &self,
        body: &[u8],
        content_type: Option<&str>,
    ) -> Result<Value> {
        let tx_data = match content_type {
            Some("application/cbor") | Some("application/octet-stream") => {
                // Binary CBOR data
                serde_json::json!({
                    "cborData": hex::encode(body)
                })
            }
            Some("application/json") | _ => {
                // JSON data
                let json_str = std::str::from_utf8(body)
                    .map_err(|e| ApiError::RequestError(format!("Invalid UTF-8: {}", e)))?;

                serde_json::from_str::<Value>(json_str)
                    .map_err(|e| ApiError::RequestError(format!("Invalid JSON: {}", e)))?
            }
        };

        let result = self.handle_submit_transaction(&tx_data).await?;
        Ok(serde_json::to_value(result)?)
    }

    /// Handle REST API mempool query
    pub async fn rest_get_mempool(&self) -> Result<Value> {
        let stats = self.handle_mempool_stats().await?;
        Ok(serde_json::to_value(stats)?)
    }

    /// Handle REST API transaction status query
    pub async fn rest_get_transaction_status(&self, tx_id: &str) -> Result<Option<Value>> {
        let result = self.handle_transaction_status(tx_id).await?;
        Ok(result.map(|r| serde_json::to_value(r).unwrap_or(serde_json::json!({}))))
    }
}

/// Local socket handlers implementation
impl SubmitApiHandlers {
    /// Handle local socket transaction submission
    pub async fn socket_submit_transaction(&self, params: &Value) -> Result<Value> {
        let result = self.handle_submit_transaction(params).await?;
        Ok(serde_json::to_value(result)?)
    }

    /// Handle local socket mempool query
    pub async fn socket_get_mempool(&self) -> Result<Value> {
        let stats = self.handle_mempool_stats().await?;
        Ok(serde_json::to_value(stats)?)
    }

    /// Handle local socket transaction status query
    pub async fn socket_get_transaction_status(&self, tx_id: &str) -> Result<Value> {
        match self.handle_transaction_status(tx_id).await? {
            Some(result) => Ok(serde_json::to_value(result)?),
            None => Err(ApiError::RequestError("Transaction not found".to_string())),
        }
    }

    /// Handle local socket batch submission
    pub async fn socket_batch_submit(&self, params: &Value) -> Result<Value> {
        let transactions = params.get("transactions")
            .and_then(|t| t.as_array())
            .ok_or_else(|| ApiError::RequestError("Missing transactions array".to_string()))?;

        let results = self.handle_batch_submit(transactions).await?;
        Ok(serde_json::to_value(results)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handlers_creation() {
        let _handlers = SubmitApiHandlers::with_default_config();
    }

    #[tokio::test]
    async fn test_submit_transaction_handler() {
        let handlers = SubmitApiHandlers::with_default_config();

        let tx_data = serde_json::json!({
            "id": "test_tx_123",
            "cborData": "84a400818258203b40265111d8bb3c3c608d95b3a0bf83461ace32d79336579a1939b3aad1c0b700018282583900e9c31e11199b16b8d59a97e85133187b6a84e18b7d2deeac1cf7dc40a64154ab7d1857b9b5b57a985bbe5179c4a86e3239a676b18b860f4d055a100a1581ce31e11199b16b8d59a97e85133187b6a84e18b7d2deeac1cf7dc40a641013a21a3ae41000021a0002a1a1a028d5190102198282582069e13e1ae5bcd57c21dd5b8a8c3c01607cd6dd8bbbf888b1df7bc8c7f4b9b7e45840123456789abcdef123456789abcdef123456789abcdef123456789abcdef123456789abcdef123456789abcdef123456789abcdef123456789abcdef12345678f6",
            "size": 256,
            "fee": 174593
        });

        let result = handlers.handle_submit_transaction(&tx_data).await.unwrap();
        assert_eq!(result.tx_id, "test_tx_123");
        assert_eq!(result.status, super::SubmissionStatus::Accepted);
    }

    #[tokio::test]
    async fn test_mempool_stats_handler() {
        let handlers = SubmitApiHandlers::with_default_config();

        let stats = handlers.handle_mempool_stats().await.unwrap();
        assert_eq!(stats.size, 0); // Empty mempool initially
    }

    #[tokio::test]
    async fn test_rest_submit_json() {
        let handlers = SubmitApiHandlers::with_default_config();

        let json_body = r#"{"id": "test_tx", "fee": 174593}"#;
        let result = handlers.rest_submit_transaction(
            json_body.as_bytes(),
            Some("application/json")
        ).await.unwrap();

        assert!(result.get("txId").is_some());
        assert!(result.get("status").is_some());
    }

    #[tokio::test]
    async fn test_rest_submit_cbor() {
        let handlers = SubmitApiHandlers::with_default_config();

        let cbor_data = hex::decode("84a400818258203b40265111d8bb3c3c608d95b3a0bf83461ace32d79336579a1939b3aad1c0b70001").unwrap();
        let result = handlers.rest_submit_transaction(
            &cbor_data,
            Some("application/cbor")
        ).await.unwrap();

        assert!(result.get("txId").is_some());
        assert!(result.get("status").is_some());
    }

    #[tokio::test]
    async fn test_batch_submission() {
        let handlers = SubmitApiHandlers::with_default_config();

        let transactions = vec![
            serde_json::json!({"id": "tx1", "fee": 174593}),
            serde_json::json!({"id": "tx2", "fee": 200000}),
        ];

        let results = handlers.handle_batch_submit(&transactions).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].tx_id, "tx1");
        assert_eq!(results[1].tx_id, "tx2");
    }

    #[tokio::test]
    async fn test_socket_methods() {
        let handlers = SubmitApiHandlers::with_default_config();

        // Test socket transaction submission
        let tx_params = serde_json::json!({"id": "socket_tx", "fee": 174593});
        let submit_result = handlers.socket_submit_transaction(&tx_params).await.unwrap();
        assert!(submit_result.get("txId").is_some());

        // Test socket mempool query
        let mempool_result = handlers.socket_get_mempool().await.unwrap();
        assert!(mempool_result.get("size").is_some());
    }
}
