//! Mempool Bridge
//!
//! Bridges the API's mempool (with ParsedTransaction) to the consensus layer's
//! block production service (expecting consensus::Transaction).
//!
//! This adapter continuously polls the mempool for new transactions, converts them
//! from the API's string-based format to the consensus hash-based format, and
//! sends them to the block production service for inclusion in blocks.

use crate::submit_api::{mempool::MempoolManager, ParsedTransaction};
use cardano_consensus::{Transaction, TxInput, TxOutput};
use cardano_crypto::hash::Blake2b256Hash;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, sleep, Duration};

/// Configuration for the mempool bridge
#[derive(Debug, Clone)]
pub struct MempoolBridgeConfig {
    /// How often to poll the mempool for new transactions (in milliseconds)
    pub poll_interval_ms: u64,
    /// Maximum number of transactions to fetch per update
    pub max_transactions_per_update: usize,
    /// Channel size for sending transactions to block production
    pub channel_size: usize,
}

impl Default for MempoolBridgeConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 100, // Check every 100ms
            max_transactions_per_update: 100,
            channel_size: 1000,
        }
    }
}

/// Statistics tracked by the mempool bridge
#[derive(Debug, Clone, Default)]
pub struct MempoolBridgeStats {
    /// Total number of transactions converted
    pub transactions_converted: u64,
    /// Total number of conversion errors
    pub conversion_errors: u64,
    /// Total number of updates sent to block production
    pub updates_sent: u64,
    /// Total bytes of transaction data processed
    pub total_bytes_processed: u64,
    /// Number of times mempool was polled
    pub polls_performed: u64,
    /// Number of duplicate transactions filtered out
    pub duplicates_filtered: u64,
    /// Number of times the outbound channel was full
    pub channel_backpressure_events: u64,
}

/// Mempool bridge that converts API transactions to consensus transactions
pub struct MempoolBridge {
    config: MempoolBridgeConfig,
    mempool: Arc<dyn MempoolManager>,
    stats: Arc<RwLock<MempoolBridgeStats>>,
    seen_transactions: Arc<RwLock<HashSet<String>>>,
}

impl MempoolBridge {
    /// Create a new mempool bridge
    pub fn new(config: MempoolBridgeConfig, mempool: Arc<dyn MempoolManager>) -> Self {
        Self {
            config,
            mempool,
            stats: Arc::new(RwLock::new(MempoolBridgeStats::default())),
            seen_transactions: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Get current statistics
    pub async fn stats(&self) -> MempoolBridgeStats {
        self.stats.read().await.clone()
    }

    /// Convert a ParsedTransaction to a consensus Transaction
    fn convert_transaction(tx: &ParsedTransaction) -> Result<Transaction, String> {
        // Parse transaction ID (may have "0x" prefix)
        let id_hex = if tx.id.starts_with("0x") || tx.id.starts_with("0X") {
            &tx.id[2..]
        } else {
            &tx.id
        };

        let id_bytes = hex::decode(id_hex)
            .map_err(|e| format!("Invalid transaction ID hex '{}': {}", tx.id, e))?;
        let tx_id = Blake2b256Hash::from_bytes(&id_bytes)
            .map_err(|e| format!("Invalid transaction ID '{}': {}", tx.id, e))?;

        // Convert inputs
        let mut inputs = Vec::with_capacity(tx.inputs.len());
        for input in &tx.inputs {
            let tx_hash_hex = if input.tx_id.starts_with("0x") || input.tx_id.starts_with("0X") {
                &input.tx_id[2..]
            } else {
                &input.tx_id
            };

            let hash_bytes = hex::decode(tx_hash_hex)
                .map_err(|e| format!("Invalid input tx_id hex '{}': {}", input.tx_id, e))?;
            let tx_hash = Blake2b256Hash::from_bytes(&hash_bytes)
                .map_err(|e| format!("Invalid input tx_id '{}': {}", input.tx_id, e))?;

            inputs.push(TxInput {
                tx_hash,
                output_index: input.output_index,
            });
        }

        // Convert outputs
        let mut outputs = Vec::with_capacity(tx.outputs.len());
        for output in &tx.outputs {
            let address_hex =
                if output.address.starts_with("0x") || output.address.starts_with("0X") {
                    &output.address[2..]
                } else {
                    &output.address
                };

            let address_bytes = hex::decode(address_hex)
                .map_err(|e| format!("Invalid output address hex '{}': {}", output.address, e))?;
            let address = Blake2b256Hash::from_bytes(&address_bytes)
                .map_err(|e| format!("Invalid output address '{}': {}", output.address, e))?;

            outputs.push(TxOutput {
                address,
                value: output.value,
            });
        }

        Ok(Transaction {
            tx_id,
            inputs,
            outputs,
            fee: tx.fee,
            size: tx.size as u32, // Convert usize to u32
        })
    }

    /// Run the mempool bridge, continuously polling and converting transactions
    pub async fn run(
        self: Arc<Self>,
        tx_sender: mpsc::Sender<Vec<Transaction>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut ticker = interval(Duration::from_millis(self.config.poll_interval_ms));
        let backoff_delay = Duration::from_millis(self.config.poll_interval_ms.max(10));
        let mut pending_batch: Option<Vec<Transaction>> = None;

        loop {
            if let Some(batch) = pending_batch.take() {
                match tx_sender.try_send(batch) {
                    Ok(()) => {
                        let mut stats = self.stats.write().await;
                        stats.updates_sent += 1;
                    }
                    Err(mpsc::error::TrySendError::Full(batch)) => {
                        {
                            let mut stats = self.stats.write().await;
                            stats.channel_backpressure_events += 1;
                        }
                        pending_batch = Some(batch);
                        sleep(backoff_delay).await;
                        continue;
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        break;
                    }
                }
            }

            ticker.tick().await;

            {
                let mut stats = self.stats.write().await;
                stats.polls_performed += 1;
            }

            let parsed_txs = match self
                .mempool
                .get_transactions(Some(self.config.max_transactions_per_update))
                .await
            {
                Ok(txs) => txs,
                Err(e) => {
                    tracing::error!("Failed to fetch transactions from mempool: {}", e);
                    continue;
                }
            };

            if parsed_txs.is_empty() {
                continue;
            }

            let new_transactions = self.filter_new_transactions(&parsed_txs).await;

            if new_transactions.is_empty() {
                continue;
            }

            let converted = self.convert_and_record_transactions(new_transactions).await;

            if converted.is_empty() {
                continue;
            }

            match tx_sender.try_send(converted) {
                Ok(()) => {
                    let mut stats = self.stats.write().await;
                    stats.updates_sent += 1;
                }
                Err(mpsc::error::TrySendError::Full(batch)) => {
                    {
                        let mut stats = self.stats.write().await;
                        stats.channel_backpressure_events += 1;
                    }
                    pending_batch = Some(batch);
                    sleep(backoff_delay).await;
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    break;
                }
            }
        }

        Ok(())
    }

    /// Fetch and convert a batch of transactions
    async fn convert_and_record_transactions(
        &self,
        parsed_txs: Vec<ParsedTransaction>,
    ) -> Vec<Transaction> {
        let mut converted = Vec::new();
        let mut stats = self.stats.write().await;
        let mut failed_ids: Vec<String> = Vec::new();

        for tx in parsed_txs {
            match Self::convert_transaction(&tx) {
                Ok(consensus_tx) => {
                    stats.transactions_converted += 1;
                    stats.total_bytes_processed += tx.size as u64;
                    converted.push(consensus_tx);
                }
                Err(e) => {
                    stats.conversion_errors += 1;
                    tracing::warn!("Failed to convert transaction {}: {}", tx.id, e);
                    failed_ids.push(tx.id.clone());
                }
            }
        }

        drop(stats);

        if !failed_ids.is_empty() {
            let mut seen = self.seen_transactions.write().await;
            for tx_id in failed_ids {
                seen.remove(&tx_id);
            }
        }

        converted
    }

    /// Filter out transactions that have already been forwarded to consensus
    pub(crate) async fn filter_new_transactions(
        &self,
        parsed_txs: &[ParsedTransaction],
    ) -> Vec<ParsedTransaction> {
        let mut seen = self.seen_transactions.write().await;
        let mut new_transactions = Vec::new();
        let mut duplicates = 0u64;

        for tx in parsed_txs {
            if seen.insert(tx.id.clone()) {
                new_transactions.push(tx.clone());
            } else {
                duplicates += 1;
            }
        }

        drop(seen);

        if duplicates > 0 {
            let mut stats = self.stats.write().await;
            stats.duplicates_filtered += duplicates;
        }

        new_transactions
    }

    /// Remove transactions from the mempool after they've been included in a block
    pub async fn remove_transactions(&self, tx_ids: &[String]) -> Result<(), String> {
        for tx_id in tx_ids {
            self.mempool
                .remove_transaction(tx_id)
                .await
                .map_err(|e| format!("Failed to remove transaction {}: {}", tx_id, e))?;
        }

        if !tx_ids.is_empty() {
            let mut seen = self.seen_transactions.write().await;
            for tx_id in tx_ids {
                seen.remove(tx_id);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::submit_api::{
        mempool::{MempoolManager, MempoolStats, TransactionInfo},
        TransactionInput, TransactionOutput,
    };
    use crate::Result;
    use async_trait::async_trait;

    #[test]
    fn test_convert_valid_transaction() {
        let parsed = ParsedTransaction {
            id: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
            cbor_data: vec![],
            size: 123,
            fee: 170000,
            inputs: vec![TransactionInput {
                tx_id: "0xfedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321"
                    .to_string(),
                output_index: 0,
                witness: None,
            }],
            outputs: vec![TransactionOutput {
                address: "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
                    .to_string(),
                value: 1000000,
                assets: None,
                datum_hash: None,
            }],
        };

        let result = MempoolBridge::convert_transaction(&parsed);
        assert!(result.is_ok());

        let tx = result.unwrap();
        assert_eq!(tx.fee, 170000);
        assert_eq!(tx.size, 123);
        assert_eq!(tx.inputs.len(), 1);
        assert_eq!(tx.outputs.len(), 1);
        assert_eq!(tx.inputs[0].output_index, 0);
        assert_eq!(tx.outputs[0].value, 1000000);
    }

    #[test]
    fn test_convert_invalid_tx_id() {
        let parsed = ParsedTransaction {
            id: "0xINVALID".to_string(),
            cbor_data: vec![],
            size: 123,
            fee: 170000,
            inputs: vec![],
            outputs: vec![],
        };

        let result = MempoolBridge::convert_transaction(&parsed);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid transaction ID"));
    }

    #[test]
    fn test_convert_without_0x_prefix() {
        let parsed = ParsedTransaction {
            id: "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
            cbor_data: vec![],
            size: 100,
            fee: 150000,
            inputs: vec![TransactionInput {
                tx_id: "fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321"
                    .to_string(),
                output_index: 1,
                witness: None,
            }],
            outputs: vec![TransactionOutput {
                address: "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
                    .to_string(),
                value: 2000000,
                assets: None,
                datum_hash: None,
            }],
        };

        let result = MempoolBridge::convert_transaction(&parsed);
        assert!(result.is_ok());

        let tx = result.unwrap();
        assert_eq!(tx.inputs[0].output_index, 1);
        assert_eq!(tx.outputs[0].value, 2000000);
    }

    #[tokio::test]
    async fn test_filter_new_transactions() {
        let bridge = Arc::new(MempoolBridge::new(
            MempoolBridgeConfig::default(),
            Arc::new(DummyMempoolManager::default()),
        ));

        let tx = ParsedTransaction {
            id: "deadbeef".to_string(),
            cbor_data: vec![],
            size: 10,
            fee: 1,
            inputs: vec![],
            outputs: vec![],
        };

        let first = bridge.filter_new_transactions(&[tx.clone()]).await;
        assert_eq!(first.len(), 1);

        let second = bridge.filter_new_transactions(&[tx.clone()]).await;
        assert!(second.is_empty());
    }

    #[tokio::test]
    async fn test_remove_transactions_clears_seen() {
        let bridge = Arc::new(MempoolBridge::new(
            MempoolBridgeConfig::default(),
            Arc::new(DummyMempoolManager::default()),
        ));

        let tx = ParsedTransaction {
            id: "cafebabe".to_string(),
            cbor_data: vec![],
            size: 10,
            fee: 1,
            inputs: vec![],
            outputs: vec![],
        };

        let _ = bridge.filter_new_transactions(&[tx.clone()]).await;

        bridge.remove_transactions(&[tx.id.clone()]).await.unwrap();

        let after_removal = bridge.filter_new_transactions(&[tx.clone()]).await;
        assert_eq!(after_removal.len(), 1);
    }

    #[derive(Default)]
    struct DummyMempoolManager;

    #[async_trait]
    impl MempoolManager for DummyMempoolManager {
        async fn add_transaction(&self, _tx: ParsedTransaction) -> Result<()> {
            Ok(())
        }

        async fn remove_transaction(&self, _tx_id: &str) -> Result<Option<ParsedTransaction>> {
            Ok(None)
        }

        async fn contains_transaction(&self, _tx_id: &str) -> Result<bool> {
            Ok(false)
        }

        async fn get_transaction_info(&self, _tx_id: &str) -> Result<Option<TransactionInfo>> {
            Ok(None)
        }

        async fn get_stats(&self) -> Result<MempoolStats> {
            Ok(MempoolStats {
                size: 0,
                bytes: 0,
                oldest_timestamp: 0,
            })
        }

        async fn get_transactions(&self, _limit: Option<usize>) -> Result<Vec<ParsedTransaction>> {
            Ok(Vec::new())
        }
    }
}
