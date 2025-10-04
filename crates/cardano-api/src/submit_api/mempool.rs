//! Mempool Management
//!
//! Handles transaction storage, ordering, and lifecycle management
//! in the memory pool before block inclusion.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::{ApiError, Result};
use super::{ParsedTransaction, SubmissionStatus, SubmitApiConfig};

/// Transaction information in mempool
#[derive(Debug, Clone)]
pub struct TransactionInfo {
    /// Current submission status
    pub status: SubmissionStatus,
    /// Error message if rejected
    pub error: Option<String>,
}

/// Mempool statistics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MempoolStats {
    /// Number of transactions in mempool
    pub size: usize,
    /// Total bytes of all transactions
    pub bytes: usize,
    /// Timestamp of oldest transaction
    pub oldest_timestamp: u64,
}

/// Mempool entry with metadata
#[derive(Debug, Clone)]
struct MempoolEntry {
    /// The transaction itself
    transaction: ParsedTransaction,
    /// Timestamp when added to mempool
    timestamp: u64,
    /// Priority score for ordering
    priority: f64,
    /// Number of validation attempts
    validation_attempts: u32,
}

/// Trait for mempool management
#[async_trait::async_trait]
pub trait MempoolManager: Send + Sync {
    /// Add a transaction to the mempool
    async fn add_transaction(&self, tx: ParsedTransaction) -> Result<()>;

    /// Remove a transaction from the mempool
    async fn remove_transaction(&self, tx_id: &str) -> Result<Option<ParsedTransaction>>;

    /// Check if transaction exists in mempool
    async fn contains_transaction(&self, tx_id: &str) -> Result<bool>;

    /// Get transaction information
    async fn get_transaction_info(&self, tx_id: &str) -> Result<Option<TransactionInfo>>;

    /// Get mempool statistics
    async fn get_stats(&self) -> Result<MempoolStats>;

    /// Get transactions for block building (ordered by priority)
    async fn get_transactions(&self, limit: Option<usize>) -> Result<Vec<ParsedTransaction>>;
}

/// In-memory mempool implementation
pub struct InMemoryMempool {
    config: SubmitApiConfig,
    transactions: Arc<RwLock<HashMap<String, MempoolEntry>>>,
    /// Transactions ordered by priority (fee rate)
    priority_queue: Arc<RwLock<Vec<String>>>,
}

impl InMemoryMempool {
    /// Create a new in-memory mempool
    pub fn new(config: SubmitApiConfig) -> Self {
        Self {
            config,
            transactions: Arc::new(RwLock::new(HashMap::new())),
            priority_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Calculate priority score for a transaction (fee per byte)
    fn calculate_priority(&self, tx: &ParsedTransaction) -> f64 {
        if tx.size == 0 {
            return 0.0;
        }
        tx.fee as f64 / tx.size as f64
    }

    /// Insert transaction into priority queue maintaining order
    async fn insert_into_priority_queue(&self, tx_id: String, priority: f64) {
        let mut queue = self.priority_queue.write().await;
        let transactions = self.transactions.read().await;

        // Find insertion position (maintain descending order by priority)
        let mut insert_pos = 0;
        for (i, existing_id) in queue.iter().enumerate() {
            if let Some(existing_entry) = transactions.get(existing_id) {
                if priority > existing_entry.priority {
                    break;
                }
                insert_pos = i + 1;
            }
        }

        queue.insert(insert_pos, tx_id);
    }

    /// Remove transaction from priority queue
    async fn remove_from_priority_queue(&self, tx_id: &str) {
        let mut queue = self.priority_queue.write().await;
        if let Some(pos) = queue.iter().position(|id| id == tx_id) {
            queue.remove(pos);
        }
    }

    /// Clean up expired transactions
    async fn cleanup_expired_transactions(&self) -> Result<()> {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut expired_txs = Vec::new();

        {
            let transactions = self.transactions.read().await;
            for (tx_id, entry) in transactions.iter() {
                if current_time.saturating_sub(entry.timestamp) > self.config.tx_timeout {
                    expired_txs.push(tx_id.clone());
                }
            }
        }

        for tx_id in expired_txs {
            warn!("Removing expired transaction: {}", tx_id);
            self.remove_transaction(&tx_id).await?;
        }

        Ok(())
    }

    /// Enforce mempool size limits
    async fn enforce_size_limits(&self) -> Result<()> {
        let queue_len = self.priority_queue.read().await.len();

        if queue_len > self.config.max_mempool_size {
            let excess = queue_len - self.config.max_mempool_size;
            info!("Mempool over limit, removing {} lowest priority transactions", excess);

            // Remove lowest priority transactions
            let to_remove = {
                let queue = self.priority_queue.read().await;
                queue[queue.len() - excess..].to_vec()
            };

            for tx_id in to_remove {
                self.remove_transaction(&tx_id).await?;
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl MempoolManager for InMemoryMempool {
    async fn add_transaction(&self, tx: ParsedTransaction) -> Result<()> {
        let tx_id = tx.id.clone();
        let priority = self.calculate_priority(&tx);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        debug!("Adding transaction {} to mempool with priority {}", tx_id, priority);

        let entry = MempoolEntry {
            transaction: tx,
            timestamp,
            priority,
            validation_attempts: 0,
        };

        // Add to transactions map
        {
            let mut transactions = self.transactions.write().await;
            if transactions.contains_key(&tx_id) {
                return Err(ApiError::RequestError("Transaction already in mempool".to_string()));
            }
            transactions.insert(tx_id.clone(), entry);
        }

        // Add to priority queue
        self.insert_into_priority_queue(tx_id.clone(), priority).await;

        // Cleanup and enforce limits
        self.cleanup_expired_transactions().await?;
        self.enforce_size_limits().await?;

        info!("Transaction {} added to mempool", tx_id);
        Ok(())
    }

    async fn remove_transaction(&self, tx_id: &str) -> Result<Option<ParsedTransaction>> {
        debug!("Removing transaction {} from mempool", tx_id);

        let removed_tx = {
            let mut transactions = self.transactions.write().await;
            transactions.remove(tx_id).map(|entry| entry.transaction)
        };

        if removed_tx.is_some() {
            self.remove_from_priority_queue(tx_id).await;
            info!("Transaction {} removed from mempool", tx_id);
        }

        Ok(removed_tx)
    }

    async fn contains_transaction(&self, tx_id: &str) -> Result<bool> {
        let transactions = self.transactions.read().await;
        Ok(transactions.contains_key(tx_id))
    }

    async fn get_transaction_info(&self, tx_id: &str) -> Result<Option<TransactionInfo>> {
        let transactions = self.transactions.read().await;
        if let Some(_entry) = transactions.get(tx_id) {
            Ok(Some(TransactionInfo {
                status: SubmissionStatus::Accepted,
                error: None,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_stats(&self) -> Result<MempoolStats> {
        let transactions = self.transactions.read().await;

        let size = transactions.len();
        let bytes = transactions.values().map(|entry| entry.transaction.size).sum();
        let oldest_timestamp = transactions.values()
            .map(|entry| entry.timestamp)
            .min()
            .unwrap_or(0);

        Ok(MempoolStats {
            size,
            bytes,
            oldest_timestamp,
        })
    }

    async fn get_transactions(&self, limit: Option<usize>) -> Result<Vec<ParsedTransaction>> {
        let queue = self.priority_queue.read().await;
        let transactions = self.transactions.read().await;

        let limit = limit.unwrap_or(queue.len());
        let mut result = Vec::new();

        for tx_id in queue.iter().take(limit) {
            if let Some(entry) = transactions.get(tx_id) {
                result.push(entry.transaction.clone());
            }
        }

        Ok(result)
    }
}

/// Mempool event types for monitoring
#[derive(Debug, Clone)]
pub enum MempoolEvent {
    /// Transaction added to mempool
    TransactionAdded(String),
    /// Transaction removed from mempool
    TransactionRemoved(String),
    /// Transaction expired
    TransactionExpired(String),
    /// Mempool size limit reached
    SizeLimitReached(usize),
}

/// Mempool event listener trait
pub trait MempoolEventListener: Send + Sync {
    /// Handle mempool event
    fn on_mempool_event(&self, event: MempoolEvent);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::submit_api::{TransactionInput, TransactionOutput};

    #[tokio::test]
    async fn test_mempool_creation() {
        let config = SubmitApiConfig::default();
        let _mempool = InMemoryMempool::new(config);
    }

    #[tokio::test]
    async fn test_add_transaction() {
        let config = SubmitApiConfig::default();
        let mempool = InMemoryMempool::new(config);

        let tx = ParsedTransaction {
            id: "test_tx".to_string(),
            cbor_data: vec![0u8; 256],
            size: 256,
            fee: 200000,
            inputs: vec![],
            outputs: vec![],
        };

        assert!(mempool.add_transaction(tx).await.is_ok());
        assert!(mempool.contains_transaction("test_tx").await.unwrap());
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let config = SubmitApiConfig::default();
        let mempool = InMemoryMempool::new(config);

        // Add transactions with different fee rates
        let high_priority_tx = ParsedTransaction {
            id: "high_priority".to_string(),
            cbor_data: vec![0u8; 256],
            size: 256,
            fee: 400000, // Higher fee rate
            inputs: vec![],
            outputs: vec![],
        };

        let low_priority_tx = ParsedTransaction {
            id: "low_priority".to_string(),
            cbor_data: vec![0u8; 256],
            size: 256,
            fee: 200000, // Lower fee rate
            inputs: vec![],
            outputs: vec![],
        };

        mempool.add_transaction(low_priority_tx).await.unwrap();
        mempool.add_transaction(high_priority_tx).await.unwrap();

        let transactions = mempool.get_transactions(Some(2)).await.unwrap();
        assert_eq!(transactions[0].id, "high_priority");
        assert_eq!(transactions[1].id, "low_priority");
    }

    #[tokio::test]
    async fn test_mempool_stats() {
        let config = SubmitApiConfig::default();
        let mempool = InMemoryMempool::new(config);

        let tx = ParsedTransaction {
            id: "test_tx".to_string(),
            cbor_data: vec![0u8; 256],
            size: 256,
            fee: 200000,
            inputs: vec![],
            outputs: vec![],
        };

        mempool.add_transaction(tx).await.unwrap();

        let stats = mempool.get_stats().await.unwrap();
        assert_eq!(stats.size, 1);
        assert_eq!(stats.bytes, 256);
    }

    #[tokio::test]
    async fn test_remove_transaction() {
        let config = SubmitApiConfig::default();
        let mempool = InMemoryMempool::new(config);

        let tx = ParsedTransaction {
            id: "test_tx".to_string(),
            cbor_data: vec![0u8; 256],
            size: 256,
            fee: 200000,
            inputs: vec![],
            outputs: vec![],
        };

        mempool.add_transaction(tx).await.unwrap();
        assert!(mempool.contains_transaction("test_tx").await.unwrap());

        let removed = mempool.remove_transaction("test_tx").await.unwrap();
        assert!(removed.is_some());
        assert!(!mempool.contains_transaction("test_tx").await.unwrap());
    }
}
