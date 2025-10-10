//! Submit API Implementation
//!
//! Provides transaction validation pipeline, mempool integration, and
//! comprehensive error handling for transaction submission.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::{ApiError, Result};

pub mod handlers;
pub mod mempool;
pub mod validation;

pub use handlers::SubmitApiHandlers;
pub use mempool::*;
pub use validation::*;

/// Transaction submission configuration
#[derive(Debug, Clone)]
pub struct SubmitApiConfig {
    /// Maximum transaction size in bytes
    pub max_tx_size: usize,
    /// Maximum mempool size
    pub max_mempool_size: usize,
    /// Transaction timeout in seconds
    pub tx_timeout: u64,
    /// Enable strict validation
    pub strict_validation: bool,
}

impl Default for SubmitApiConfig {
    fn default() -> Self {
        Self {
            max_tx_size: 16384,     // 16KB max transaction size
            max_mempool_size: 1000, // Max 1000 transactions in mempool
            tx_timeout: 300,        // 5 minute timeout
            strict_validation: true,
        }
    }
}

/// Transaction submission status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SubmissionStatus {
    /// Transaction accepted into mempool
    Accepted,
    /// Transaction rejected due to validation error
    Rejected,
    /// Transaction already exists in mempool
    Duplicate,
    /// Transaction pending validation
    Pending,
    /// Transaction included in block
    Confirmed,
    /// Transaction expired from mempool
    Expired,
}

/// Transaction submission result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionResult {
    /// Transaction ID
    pub tx_id: String,
    /// Submission status
    pub status: SubmissionStatus,
    /// Optional error message
    pub error: Option<String>,
    /// Submission timestamp
    pub timestamp: u64,
}

/// Submit API service
pub struct SubmitApiService {
    config: SubmitApiConfig,
    validator: Arc<dyn TransactionValidator>,
    mempool: Arc<dyn MempoolManager>,
}

impl SubmitApiService {
    /// Create a new submit API service
    pub fn new(
        config: SubmitApiConfig,
        validator: Arc<dyn TransactionValidator>,
        mempool: Arc<dyn MempoolManager>,
    ) -> Self {
        Self {
            config,
            validator,
            mempool,
        }
    }

    /// Get a handle to the underlying mempool manager
    pub fn mempool(&self) -> Arc<dyn MempoolManager> {
        Arc::clone(&self.mempool)
    }

    /// Submit a transaction for validation and inclusion
    pub async fn submit_transaction(&self, tx_data: &Value) -> Result<SubmissionResult> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        debug!("Submitting transaction for validation");

        // Parse transaction data
        let parsed_tx = self.parse_transaction_data(tx_data)?;
        let tx_id = parsed_tx.id.clone();

        // Check if transaction already exists in mempool
        if self.mempool.contains_transaction(&tx_id).await? {
            info!("Transaction {} already exists in mempool", tx_id);
            return Ok(SubmissionResult {
                tx_id,
                status: SubmissionStatus::Duplicate,
                error: None,
                timestamp,
            });
        }

        // Validate transaction
        match self.validator.validate_transaction(&parsed_tx).await {
            Ok(()) => {
                debug!("Transaction {} passed validation", tx_id);

                // Add to mempool
                match self.mempool.add_transaction(parsed_tx).await {
                    Ok(()) => {
                        info!("Transaction {} accepted into mempool", tx_id);
                        Ok(SubmissionResult {
                            tx_id,
                            status: SubmissionStatus::Accepted,
                            error: None,
                            timestamp,
                        })
                    }
                    Err(e) => {
                        error!("Failed to add transaction {} to mempool: {}", tx_id, e);
                        Ok(SubmissionResult {
                            tx_id,
                            status: SubmissionStatus::Rejected,
                            error: Some(format!("Mempool error: {}", e)),
                            timestamp,
                        })
                    }
                }
            }
            Err(e) => {
                warn!("Transaction {} validation failed: {}", tx_id, e);
                Ok(SubmissionResult {
                    tx_id,
                    status: SubmissionStatus::Rejected,
                    error: Some(e.to_string()),
                    timestamp,
                })
            }
        }
    }

    /// Get transaction status from mempool
    pub async fn get_transaction_status(&self, tx_id: &str) -> Result<Option<SubmissionResult>> {
        if let Some(tx_info) = self.mempool.get_transaction_info(tx_id).await? {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            Ok(Some(SubmissionResult {
                tx_id: tx_id.to_string(),
                status: tx_info.status,
                error: tx_info.error,
                timestamp,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get mempool statistics
    pub async fn get_mempool_stats(&self) -> Result<MempoolStats> {
        self.mempool.get_stats().await
    }

    /// Parse transaction data from various formats (JSON, CBOR hex)
    fn parse_transaction_data(&self, tx_data: &Value) -> Result<ParsedTransaction> {
        // Try to parse as structured JSON transaction
        if let Ok(tx) = serde_json::from_value::<ParsedTransaction>(tx_data.clone()) {
            return Ok(tx);
        }

        // Try to parse as CBOR hex string
        if let Some(cbor_hex) = tx_data.get("cborData").and_then(|v| v.as_str()) {
            return self.parse_cbor_transaction(cbor_hex);
        }

        // Try direct hex string
        if let Some(hex_str) = tx_data.as_str() {
            return self.parse_cbor_transaction(hex_str);
        }

        Err(ApiError::RequestError(
            "Invalid transaction data format".to_string(),
        ))
    }

    /// Parse CBOR transaction from hex string
    fn parse_cbor_transaction(&self, hex_str: &str) -> Result<ParsedTransaction> {
        let cbor_bytes = hex::decode(hex_str)
            .map_err(|e| ApiError::RequestError(format!("Invalid hex data: {}", e)))?;

        if cbor_bytes.len() > self.config.max_tx_size {
            return Err(ApiError::RequestError(format!(
                "Transaction size {} exceeds maximum {}",
                cbor_bytes.len(),
                self.config.max_tx_size
            )));
        }

        // For now, create a mock transaction from CBOR data
        // In a real implementation, this would use minicbor to decode the transaction
        let tx_id = format!(
            "cbor_tx_{}",
            hex::encode(&cbor_bytes[..std::cmp::min(32, cbor_bytes.len())])
        );
        let size = cbor_bytes.len();

        Ok(ParsedTransaction {
            id: tx_id,
            cbor_data: cbor_bytes,
            size,
            fee: 174593, // Mock fee
            inputs: vec![],
            outputs: vec![],
        })
    }
}

/// Parsed transaction representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTransaction {
    /// Transaction ID
    pub id: String,
    /// Raw CBOR data
    pub cbor_data: Vec<u8>,
    /// Transaction size in bytes
    pub size: usize,
    /// Transaction fee
    pub fee: u64,
    /// Transaction inputs
    pub inputs: Vec<TransactionInput>,
    /// Transaction outputs
    pub outputs: Vec<TransactionOutput>,
}

/// Transaction input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    /// Referenced transaction ID
    pub tx_id: String,
    /// Referenced output index
    pub output_index: u32,
    /// Script witness (optional)
    pub witness: Option<String>,
}

/// Transaction output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionOutput {
    /// Output address
    pub address: String,
    /// Output value in lovelace
    pub value: u64,
    /// Native assets (optional)
    pub assets: Option<Value>,
    /// Datum hash (optional)
    pub datum_hash: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_submit_api_service_creation() {
        let config = SubmitApiConfig::default();
        let validator = Arc::new(MockTransactionValidator::default());
        let mempool = Arc::new(MockMempoolManager::default());

        let _service = SubmitApiService::new(config, validator, mempool);
    }

    #[tokio::test]
    async fn test_transaction_submission() {
        let config = SubmitApiConfig::default();
        let validator = Arc::new(MockTransactionValidator::default());
        let mempool = Arc::new(MockMempoolManager::default());

        let service = SubmitApiService::new(config, validator, mempool);

        let tx_data = serde_json::json!({
            "id": "test_tx_123",
            "cborData": "84a400818258203b40265111d8bb3c3c608d95b3a0bf83461ace32d79336579a1939b3aad1c0b700018282583900e9c31e11199b16b8d59a97e85133187b6a84e18b7d2deeac1cf7dc40a64154ab7d1857b9b5b57a985bbe5179c4a86e3239a676b18b860f4d055a100a1581ce31e11199b16b8d59a97e85133187b6a84e18b7d2deeac1cf7dc40a641013a21a3ae41000021a0002a1a1a028d5190102198282582069e13e1ae5bcd57c21dd5b8a8c3c01607cd6dd8bbbf888b1df7bc8c7f4b9b7e45840123456789abcdef123456789abcdef123456789abcdef123456789abcdef123456789abcdef123456789abcdef123456789abcdef123456789abcdef12345678f6",
            "size": 256,
            "fee": 174593
        });

        let result = service.submit_transaction(&tx_data).await.unwrap();
        assert_eq!(result.status, SubmissionStatus::Accepted);
        // TX ID is generated from CBOR data, not from user input
        assert!(result.tx_id.starts_with("cbor_tx_"));
    }

    #[tokio::test]
    async fn test_cbor_transaction_parsing() {
        let config = SubmitApiConfig::default();
        let validator = Arc::new(MockTransactionValidator::default());
        let mempool = Arc::new(MockMempoolManager::default());

        let service = SubmitApiService::new(config, validator, mempool);

        let hex_data =
            "84a400818258203b40265111d8bb3c3c608d95b3a0bf83461ace32d79336579a1939b3aad1c0b70001";
        let tx_data = serde_json::json!({
            "cborData": hex_data
        });

        let result = service.submit_transaction(&tx_data).await.unwrap();
        assert_eq!(result.status, SubmissionStatus::Accepted);
    }

    // Mock implementations for testing
    #[derive(Default)]
    struct MockTransactionValidator;

    #[async_trait::async_trait]
    impl TransactionValidator for MockTransactionValidator {
        async fn validate_transaction(&self, _tx: &ParsedTransaction) -> Result<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockMempoolManager {
        transactions: tokio::sync::Mutex<HashMap<String, ParsedTransaction>>,
    }

    #[async_trait::async_trait]
    impl MempoolManager for MockMempoolManager {
        async fn add_transaction(&self, tx: ParsedTransaction) -> Result<()> {
            let mut transactions = self.transactions.lock().await;
            transactions.insert(tx.id.clone(), tx);
            Ok(())
        }

        async fn remove_transaction(&self, tx_id: &str) -> Result<Option<ParsedTransaction>> {
            let mut transactions = self.transactions.lock().await;
            Ok(transactions.remove(tx_id))
        }

        async fn contains_transaction(&self, tx_id: &str) -> Result<bool> {
            let transactions = self.transactions.lock().await;
            Ok(transactions.contains_key(tx_id))
        }

        async fn get_transaction_info(&self, tx_id: &str) -> Result<Option<TransactionInfo>> {
            let transactions = self.transactions.lock().await;
            if transactions.contains_key(tx_id) {
                Ok(Some(TransactionInfo {
                    status: SubmissionStatus::Accepted,
                    error: None,
                }))
            } else {
                Ok(None)
            }
        }

        async fn get_stats(&self) -> Result<MempoolStats> {
            let transactions = self.transactions.lock().await;
            Ok(MempoolStats {
                size: transactions.len(),
                bytes: transactions.values().map(|tx| tx.size).sum(),
                oldest_timestamp: 0,
            })
        }

        async fn get_transactions(&self, _limit: Option<usize>) -> Result<Vec<ParsedTransaction>> {
            let transactions = self.transactions.lock().await;
            Ok(transactions.values().cloned().collect())
        }
    }
}
