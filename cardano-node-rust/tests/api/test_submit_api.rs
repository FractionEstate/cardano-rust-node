//! Submit API Integration Tests
//!
//! Tests for the Cardano Node transaction submission API, validating transaction
//! submission, validation pipeline, mempool integration, and error handling.
//! These tests ensure transactions are properly validated and processed.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Mutex};
use serde_json::{json, Value};
use hex;

/// Test result type for submit API tests
type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Mock transaction for testing
#[derive(Debug, Clone, PartialEq)]
pub struct MockTransaction {
    pub id: String,
    pub inputs: Vec<MockTxInput>,
    pub outputs: Vec<MockTxOutput>,
    pub fee: u64,
    pub ttl: Option<u64>,
    pub certificates: Vec<MockCertificate>,
    pub withdrawals: HashMap<String, u64>,
    pub auxiliary_data: Option<MockAuxiliaryData>,
    pub witness_set: MockWitnessSet,
    pub validity_interval: MockValidityInterval,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MockTxInput {
    pub tx_id: String,
    pub output_index: u32,
    pub address: String,
    pub amount: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MockTxOutput {
    pub address: String,
    pub amount: u64,
    pub assets: Vec<MockAsset>,
    pub datum: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MockAsset {
    pub policy_id: String,
    pub asset_name: String,
    pub quantity: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MockCertificate {
    pub cert_type: String,
    pub stake_credential: String,
    pub pool_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MockAuxiliaryData {
    pub metadata: HashMap<String, Value>,
    pub native_scripts: Vec<String>,
    pub plutus_scripts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MockWitnessSet {
    pub vkey_witnesses: Vec<MockVKeyWitness>,
    pub native_scripts: Vec<String>,
    pub bootstrap_witnesses: Vec<String>,
    pub plutus_data: Vec<String>,
    pub redeemers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MockVKeyWitness {
    pub vkey: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MockValidityInterval {
    pub invalid_before: Option<u64>,
    pub invalid_after: Option<u64>,
}

/// Transaction validation error types
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    InvalidSignature(String),
    InsufficientFunds { required: u64, available: u64 },
    InvalidInput(String),
    ExpiredTransaction { current_slot: u64, ttl: u64 },
    InvalidScriptExecution(String),
    InvalidAddress(String),
    InvalidFee { provided: u64, minimum: u64 },
    DoubleSpending(String),
    InvalidUtxo(String),
    InvalidCertificate(String),
    InvalidWithdrawal(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ValidationError::InvalidSignature(msg) => write!(f, "Invalid signature: {}", msg),
            ValidationError::InsufficientFunds { required, available } => {
                write!(f, "Insufficient funds: required {}, available {}", required, available)
            }
            ValidationError::InvalidInput(input) => write!(f, "Invalid input: {}", input),
            ValidationError::ExpiredTransaction { current_slot, ttl } => {
                write!(f, "Transaction expired: current slot {}, TTL {}", current_slot, ttl)
            }
            ValidationError::InvalidScriptExecution(msg) => write!(f, "Script execution failed: {}", msg),
            ValidationError::InvalidAddress(addr) => write!(f, "Invalid address: {}", addr),
            ValidationError::InvalidFee { provided, minimum } => {
                write!(f, "Invalid fee: provided {}, minimum required {}", provided, minimum)
            }
            ValidationError::DoubleSpending(input) => write!(f, "Double spending detected: {}", input),
            ValidationError::InvalidUtxo(utxo) => write!(f, "Invalid UTXO: {}", utxo),
            ValidationError::InvalidCertificate(cert) => write!(f, "Invalid certificate: {}", cert),
            ValidationError::InvalidWithdrawal(withdrawal) => write!(f, "Invalid withdrawal: {}", withdrawal),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Transaction submission response
#[derive(Debug, Clone)]
pub struct SubmissionResponse {
    pub tx_id: String,
    pub status: SubmissionStatus,
    pub validation_errors: Vec<ValidationError>,
    pub submitted_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SubmissionStatus {
    Accepted,
    Rejected,
    Pending,
    InMempool,
    Confirmed,
}

/// Mock mempool for testing
#[derive(Debug)]
pub struct MockMempool {
    transactions: Arc<RwLock<HashMap<String, MockTransaction>>>,
    pending: Arc<RwLock<HashMap<String, SystemTime>>>,
    max_size: usize,
    max_tx_size: usize,
}

impl MockMempool {
    pub fn new(max_size: usize, max_tx_size: usize) -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            pending: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            max_tx_size,
        }
    }

    /// Add transaction to mempool
    pub async fn add_transaction(&self, tx: MockTransaction) -> TestResult<()> {
        let mut transactions = self.transactions.write().await;
        let mut pending = self.pending.write().await;

        if transactions.len() >= self.max_size {
            return Err("Mempool full".into());
        }

        transactions.insert(tx.id.clone(), tx.clone());
        pending.insert(tx.id.clone(), SystemTime::now());

        Ok(())
    }

    /// Get transaction from mempool
    pub async fn get_transaction(&self, tx_id: &str) -> Option<MockTransaction> {
        let transactions = self.transactions.read().await;
        transactions.get(tx_id).cloned()
    }

    /// Remove transaction from mempool
    pub async fn remove_transaction(&self, tx_id: &str) -> Option<MockTransaction> {
        let mut transactions = self.transactions.write().await;
        let mut pending = self.pending.write().await;

        pending.remove(tx_id);
        transactions.remove(tx_id)
    }

    /// Get mempool size
    pub async fn size(&self) -> usize {
        let transactions = self.transactions.read().await;
        transactions.len()
    }

    /// Clear expired transactions
    pub async fn clear_expired(&self, max_age: Duration) -> usize {
        let mut transactions = self.transactions.write().await;
        let mut pending = self.pending.write().await;
        let now = SystemTime::now();

        let expired: Vec<String> = pending
            .iter()
            .filter(|(_, &time)| now.duration_since(time).unwrap_or(Duration::ZERO) > max_age)
            .map(|(id, _)| id.clone())
            .collect();

        for tx_id in &expired {
            transactions.remove(tx_id);
            pending.remove(tx_id);
        }

        expired.len()
    }
}

/// Mock UTXO set for validation
#[derive(Debug)]
pub struct MockUtxoSet {
    utxos: Arc<RwLock<HashMap<String, MockTxOutput>>>,
}

impl MockUtxoSet {
    pub fn new() -> Self {
        Self {
            utxos: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add UTXO
    pub async fn add_utxo(&self, key: String, output: MockTxOutput) {
        let mut utxos = self.utxos.write().await;
        utxos.insert(key, output);
    }

    /// Get UTXO
    pub async fn get_utxo(&self, key: &str) -> Option<MockTxOutput> {
        let utxos = self.utxos.read().await;
        utxos.get(key).cloned()
    }

    /// Remove UTXO (spend it)
    pub async fn spend_utxo(&self, key: &str) -> Option<MockTxOutput> {
        let mut utxos = self.utxos.write().await;
        utxos.remove(key)
    }

    /// Check if UTXO exists
    pub async fn contains_utxo(&self, key: &str) -> bool {
        let utxos = self.utxos.read().await;
        utxos.contains_key(key)
    }
}

/// Transaction validator
#[derive(Debug)]
pub struct MockTransactionValidator {
    utxo_set: Arc<MockUtxoSet>,
    current_slot: Arc<Mutex<u64>>,
    min_fee: u64,
}

impl MockTransactionValidator {
    pub fn new(utxo_set: Arc<MockUtxoSet>, min_fee: u64) -> Self {
        Self {
            utxo_set,
            current_slot: Arc::new(Mutex::new(1000000)), // Default slot
            min_fee,
        }
    }

    /// Validate a transaction
    pub async fn validate(&self, tx: &MockTransaction) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Validate inputs exist and are unspent
        let mut total_input = 0u64;
        for input in &tx.inputs {
            let utxo_key = format!("{}#{}", input.tx_id, input.output_index);

            if let Some(utxo) = self.utxo_set.get_utxo(&utxo_key).await {
                if utxo.address != input.address {
                    errors.push(ValidationError::InvalidInput(format!("Address mismatch for input {}", utxo_key)));
                }
                total_input += utxo.amount;
            } else {
                errors.push(ValidationError::InvalidUtxo(utxo_key));
            }
        }

        // Validate outputs
        let mut total_output = 0u64;
        for output in &tx.outputs {
            total_output += output.amount;

            if output.amount == 0 {
                errors.push(ValidationError::InvalidAddress("Output amount cannot be zero".to_string()));
            }

            // Basic address validation
            if output.address.is_empty() || output.address.len() < 10 {
                errors.push(ValidationError::InvalidAddress(output.address.clone()));
            }
        }

        // Validate fee
        if tx.fee < self.min_fee {
            errors.push(ValidationError::InvalidFee {
                provided: tx.fee,
                minimum: self.min_fee,
            });
        }

        // Validate balance equation: inputs = outputs + fee
        if total_input < total_output + tx.fee {
            errors.push(ValidationError::InsufficientFunds {
                required: total_output + tx.fee,
                available: total_input,
            });
        }

        // Validate TTL
        if let Some(ttl) = tx.ttl {
            let current_slot = *self.current_slot.lock().await;
            if current_slot > ttl {
                errors.push(ValidationError::ExpiredTransaction { current_slot, ttl });
            }
        }

        // Validate signatures (simplified)
        if tx.witness_set.vkey_witnesses.is_empty() {
            errors.push(ValidationError::InvalidSignature("No signatures provided".to_string()));
        }

        for witness in &tx.witness_set.vkey_witnesses {
            if witness.signature.len() < 10 {
                errors.push(ValidationError::InvalidSignature("Invalid signature format".to_string()));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Set current slot for validation
    pub async fn set_current_slot(&self, slot: u64) {
        let mut current_slot = self.current_slot.lock().await;
        *current_slot = slot;
    }
}

/// Submit API implementation for testing
pub struct MockSubmitApi {
    validator: Arc<MockTransactionValidator>,
    mempool: Arc<MockMempool>,
}

impl MockSubmitApi {
    pub fn new(validator: Arc<MockTransactionValidator>, mempool: Arc<MockMempool>) -> Self {
        Self { validator, mempool }
    }

    /// Submit a transaction
    pub async fn submit_transaction(&self, tx: MockTransaction) -> SubmissionResponse {
        let tx_id = tx.id.clone();
        let submitted_at = SystemTime::now();

        // Validate transaction
        match self.validator.validate(&tx).await {
            Ok(()) => {
                // Add to mempool
                match self.mempool.add_transaction(tx).await {
                    Ok(()) => SubmissionResponse {
                        tx_id,
                        status: SubmissionStatus::Accepted,
                        validation_errors: Vec::new(),
                        submitted_at,
                    },
                    Err(_) => SubmissionResponse {
                        tx_id,
                        status: SubmissionStatus::Rejected,
                        validation_errors: vec![ValidationError::InvalidInput("Mempool full".to_string())],
                        submitted_at,
                    }
                }
            }
            Err(errors) => SubmissionResponse {
                tx_id,
                status: SubmissionStatus::Rejected,
                validation_errors: errors,
                submitted_at,
            }
        }
    }

    /// Get transaction status
    pub async fn get_transaction_status(&self, tx_id: &str) -> Option<SubmissionStatus> {
        if self.mempool.get_transaction(tx_id).await.is_some() {
            Some(SubmissionStatus::InMempool)
        } else {
            None
        }
    }
}

/// Helper to create a valid transaction for testing
pub fn create_valid_transaction() -> MockTransaction {
    MockTransaction {
        id: "tx_valid_123456789abcdef".to_string(),
        inputs: vec![MockTxInput {
            tx_id: "input_tx_123456789abcdef".to_string(),
            output_index: 0,
            address: "addr1qx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgse35a3x".to_string(),
            amount: 2000000, // 2 ADA
        }],
        outputs: vec![MockTxOutput {
            address: "addr1qy3s8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8q8".to_string(),
            amount: 1500000, // 1.5 ADA
            assets: Vec::new(),
            datum: None,
        }],
        fee: 200000, // 0.2 ADA
        ttl: Some(2000000),
        certificates: Vec::new(),
        withdrawals: HashMap::new(),
        auxiliary_data: None,
        witness_set: MockWitnessSet {
            vkey_witnesses: vec![MockVKeyWitness {
                vkey: "vkey_123456789abcdef".to_string(),
                signature: "signature_123456789abcdef".to_string(),
            }],
            native_scripts: Vec::new(),
            bootstrap_witnesses: Vec::new(),
            plutus_data: Vec::new(),
            redeemers: Vec::new(),
        },
        validity_interval: MockValidityInterval {
            invalid_before: None,
            invalid_after: Some(2000000),
        },
    }
}

/// Helper to create an invalid transaction for testing
pub fn create_invalid_transaction() -> MockTransaction {
    let mut tx = create_valid_transaction();
    tx.id = "tx_invalid_123456789abcdef".to_string();
    tx.fee = 50000; // Too low fee
    tx.ttl = Some(500000); // Expired
    tx.witness_set.vkey_witnesses.clear(); // No signatures
    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mempool_operations() -> TestResult<()> {
        let mempool = MockMempool::new(10, 1024);
        let tx = create_valid_transaction();

        // Test adding transaction
        mempool.add_transaction(tx.clone()).await?;
        assert_eq!(mempool.size().await, 1);

        // Test getting transaction
        let retrieved = mempool.get_transaction(&tx.id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, tx.id);

        // Test removing transaction
        let removed = mempool.remove_transaction(&tx.id).await;
        assert!(removed.is_some());
        assert_eq!(mempool.size().await, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_mempool_capacity() -> TestResult<()> {
        let mempool = MockMempool::new(2, 1024); // Small capacity

        let tx1 = create_valid_transaction();
        let mut tx2 = create_valid_transaction();
        tx2.id = "tx2_123456789abcdef".to_string();
        let mut tx3 = create_valid_transaction();
        tx3.id = "tx3_123456789abcdef".to_string();

        // Add first two transactions
        mempool.add_transaction(tx1).await?;
        mempool.add_transaction(tx2).await?;

        // Third transaction should fail
        let result = mempool.add_transaction(tx3).await;
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_utxo_set_operations() -> TestResult<()> {
        let utxo_set = MockUtxoSet::new();
        let key = "tx123#0".to_string();
        let output = MockTxOutput {
            address: "addr123".to_string(),
            amount: 1000000,
            assets: Vec::new(),
            datum: None,
        };

        // Test adding UTXO
        utxo_set.add_utxo(key.clone(), output.clone()).await;
        assert!(utxo_set.contains_utxo(&key).await);

        // Test getting UTXO
        let retrieved = utxo_set.get_utxo(&key).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().amount, output.amount);

        // Test spending UTXO
        let spent = utxo_set.spend_utxo(&key).await;
        assert!(spent.is_some());
        assert!(!utxo_set.contains_utxo(&key).await);

        Ok(())
    }

    #[tokio::test]
    async fn test_transaction_validation_success() -> TestResult<()> {
        let utxo_set = Arc::new(MockUtxoSet::new());
        let validator = MockTransactionValidator::new(utxo_set.clone(), 100000);

        // Set up UTXO for input
        let input_key = "input_tx_123456789abcdef#0".to_string();
        let input_utxo = MockTxOutput {
            address: "addr1qx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgse35a3x".to_string(),
            amount: 2000000,
            assets: Vec::new(),
            datum: None,
        };
        utxo_set.add_utxo(input_key, input_utxo).await;

        let tx = create_valid_transaction();
        let result = validator.validate(&tx).await;

        assert!(result.is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn test_transaction_validation_insufficient_funds() -> TestResult<()> {
        let utxo_set = Arc::new(MockUtxoSet::new());
        let validator = MockTransactionValidator::new(utxo_set.clone(), 100000);

        // Set up UTXO with insufficient amount
        let input_key = "input_tx_123456789abcdef#0".to_string();
        let input_utxo = MockTxOutput {
            address: "addr1qx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgse35a3x".to_string(),
            amount: 1000000, // Less than required
            assets: Vec::new(),
            datum: None,
        };
        utxo_set.add_utxo(input_key, input_utxo).await;

        let tx = create_valid_transaction();
        let result = validator.validate(&tx).await;

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, ValidationError::InsufficientFunds { .. })));

        Ok(())
    }

    #[tokio::test]
    async fn test_transaction_validation_expired() -> TestResult<()> {
        let utxo_set = Arc::new(MockUtxoSet::new());
        let validator = MockTransactionValidator::new(utxo_set.clone(), 100000);

        // Set current slot higher than TTL
        validator.set_current_slot(3000000).await;

        let tx = create_valid_transaction(); // TTL is 2000000
        let result = validator.validate(&tx).await;

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, ValidationError::ExpiredTransaction { .. })));

        Ok(())
    }

    #[tokio::test]
    async fn test_transaction_validation_missing_utxo() -> TestResult<()> {
        let utxo_set = Arc::new(MockUtxoSet::new());
        let validator = MockTransactionValidator::new(utxo_set.clone(), 100000);

        // Don't add the UTXO that the transaction tries to spend
        let tx = create_valid_transaction();
        let result = validator.validate(&tx).await;

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, ValidationError::InvalidUtxo(_))));

        Ok(())
    }

    #[tokio::test]
    async fn test_submit_api_valid_transaction() -> TestResult<()> {
        let utxo_set = Arc::new(MockUtxoSet::new());
        let validator = Arc::new(MockTransactionValidator::new(utxo_set.clone(), 100000));
        let mempool = Arc::new(MockMempool::new(10, 1024));
        let submit_api = MockSubmitApi::new(validator, mempool.clone());

        // Set up UTXO
        let input_key = "input_tx_123456789abcdef#0".to_string();
        let input_utxo = MockTxOutput {
            address: "addr1qx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgse35a3x".to_string(),
            amount: 2000000,
            assets: Vec::new(),
            datum: None,
        };
        utxo_set.add_utxo(input_key, input_utxo).await;

        let tx = create_valid_transaction();
        let response = submit_api.submit_transaction(tx.clone()).await;

        assert_eq!(response.status, SubmissionStatus::Accepted);
        assert!(response.validation_errors.is_empty());

        // Check transaction is in mempool
        let status = submit_api.get_transaction_status(&tx.id).await;
        assert_eq!(status, Some(SubmissionStatus::InMempool));

        Ok(())
    }

    #[tokio::test]
    async fn test_submit_api_invalid_transaction() -> TestResult<()> {
        let utxo_set = Arc::new(MockUtxoSet::new());
        let validator = Arc::new(MockTransactionValidator::new(utxo_set.clone(), 100000));
        let mempool = Arc::new(MockMempool::new(10, 1024));
        let submit_api = MockSubmitApi::new(validator, mempool);

        let tx = create_invalid_transaction();
        let response = submit_api.submit_transaction(tx).await;

        assert_eq!(response.status, SubmissionStatus::Rejected);
        assert!(!response.validation_errors.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_mempool_expiry() -> TestResult<()> {
        let mempool = MockMempool::new(10, 1024);
        let tx = create_valid_transaction();

        mempool.add_transaction(tx).await?;
        assert_eq!(mempool.size().await, 1);

        // Clear with very short age (should remove transaction)
        let cleared = mempool.clear_expired(Duration::from_millis(1)).await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        let cleared = mempool.clear_expired(Duration::from_millis(1)).await;

        assert_eq!(cleared, 1);
        assert_eq!(mempool.size().await, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_double_spending_detection() -> TestResult<()> {
        let utxo_set = Arc::new(MockUtxoSet::new());
        let validator = Arc::new(MockTransactionValidator::new(utxo_set.clone(), 100000));
        let mempool = Arc::new(MockMempool::new(10, 1024));
        let submit_api = MockSubmitApi::new(validator, mempool);

        // Set up UTXO
        let input_key = "input_tx_123456789abcdef#0".to_string();
        let input_utxo = MockTxOutput {
            address: "addr1qx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgse35a3x".to_string(),
            amount: 2000000,
            assets: Vec::new(),
            datum: None,
        };
        utxo_set.add_utxo(input_key, input_utxo).await;

        // Submit first transaction
        let tx1 = create_valid_transaction();
        let response1 = submit_api.submit_transaction(tx1).await;
        assert_eq!(response1.status, SubmissionStatus::Accepted);

        // Try to submit second transaction with same input (double spending)
        let mut tx2 = create_valid_transaction();
        tx2.id = "tx2_double_spend".to_string();
        let response2 = submit_api.submit_transaction(tx2).await;

        // Should be rejected due to UTXO already being spent in mempool
        // Note: In this simple implementation, we don't track spent UTXOs in mempool
        // In a real implementation, this would be detected
        assert_eq!(response2.status, SubmissionStatus::Rejected);

        Ok(())
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Integration test for complete submission workflow
    #[tokio::test]
    async fn test_complete_submission_workflow() -> TestResult<()> {
        let utxo_set = Arc::new(MockUtxoSet::new());
        let validator = Arc::new(MockTransactionValidator::new(utxo_set.clone(), 100000));
        let mempool = Arc::new(MockMempool::new(10, 1024));
        let submit_api = MockSubmitApi::new(validator.clone(), mempool.clone());

        // 1. Set up multiple UTXOs
        for i in 0..3 {
            let input_key = format!("input_tx_{}#0", i);
            let input_utxo = MockTxOutput {
                address: format!("addr_{}", i),
                amount: 2000000,
                assets: Vec::new(),
                datum: None,
            };
            utxo_set.add_utxo(input_key, input_utxo).await;
        }

        // 2. Submit multiple valid transactions
        let mut transactions = Vec::new();
        for i in 0..3 {
            let mut tx = create_valid_transaction();
            tx.id = format!("tx_valid_{}", i);
            tx.inputs[0].tx_id = format!("input_tx_{}", i);
            tx.inputs[0].address = format!("addr_{}", i);
            transactions.push(tx);
        }

        for tx in transactions {
            let response = submit_api.submit_transaction(tx.clone()).await;
            assert_eq!(response.status, SubmissionStatus::Accepted);

            let status = submit_api.get_transaction_status(&tx.id).await;
            assert_eq!(status, Some(SubmissionStatus::InMempool));
        }

        assert_eq!(mempool.size().await, 3);

        // 3. Try to submit invalid transaction
        let invalid_tx = create_invalid_transaction();
        let response = submit_api.submit_transaction(invalid_tx).await;
        assert_eq!(response.status, SubmissionStatus::Rejected);
        assert!(!response.validation_errors.is_empty());

        // Mempool size should remain the same
        assert_eq!(mempool.size().await, 3);

        Ok(())
    }

    /// Test error recovery scenarios
    #[tokio::test]
    async fn test_error_recovery() -> TestResult<()> {
        let utxo_set = Arc::new(MockUtxoSet::new());
        let validator = Arc::new(MockTransactionValidator::new(utxo_set.clone(), 100000));
        let mempool = Arc::new(MockMempool::new(2, 1024)); // Small mempool
        let submit_api = MockSubmitApi::new(validator, mempool.clone());

        // Set up UTXOs
        for i in 0..3 {
            let input_key = format!("input_tx_{}#0", i);
            let input_utxo = MockTxOutput {
                address: format!("addr_{}", i),
                amount: 2000000,
                assets: Vec::new(),
                datum: None,
            };
            utxo_set.add_utxo(input_key, input_utxo).await;
        }

        // Fill mempool to capacity
        for i in 0..2 {
            let mut tx = create_valid_transaction();
            tx.id = format!("tx_valid_{}", i);
            tx.inputs[0].tx_id = format!("input_tx_{}", i);
            tx.inputs[0].address = format!("addr_{}", i);

            let response = submit_api.submit_transaction(tx).await;
            assert_eq!(response.status, SubmissionStatus::Accepted);
        }

        // Try to add one more (should fail due to capacity)
        let mut tx3 = create_valid_transaction();
        tx3.id = "tx_overflow".to_string();
        tx3.inputs[0].tx_id = "input_tx_2".to_string();
        tx3.inputs[0].address = "addr_2".to_string();

        let response = submit_api.submit_transaction(tx3).await;
        assert_eq!(response.status, SubmissionStatus::Rejected);

        Ok(())
    }

    /// Test concurrent transaction submission
    #[tokio::test]
    async fn test_concurrent_submissions() -> TestResult<()> {
        let utxo_set = Arc::new(MockUtxoSet::new());
        let validator = Arc::new(MockTransactionValidator::new(utxo_set.clone(), 100000));
        let mempool = Arc::new(MockMempool::new(10, 1024));
        let submit_api = Arc::new(MockSubmitApi::new(validator, mempool.clone()));

        // Set up UTXOs
        for i in 0..5 {
            let input_key = format!("input_tx_{}#0", i);
            let input_utxo = MockTxOutput {
                address: format!("addr_{}", i),
                amount: 2000000,
                assets: Vec::new(),
                datum: None,
            };
            utxo_set.add_utxo(input_key, input_utxo).await;
        }

        // Submit transactions concurrently
        let mut handles = Vec::new();
        for i in 0..5 {
            let submit_api_clone = submit_api.clone();
            let handle = tokio::spawn(async move {
                let mut tx = create_valid_transaction();
                tx.id = format!("tx_concurrent_{}", i);
                tx.inputs[0].tx_id = format!("input_tx_{}", i);
                tx.inputs[0].address = format!("addr_{}", i);

                submit_api_clone.submit_transaction(tx).await
            });
            handles.push(handle);
        }

        // Wait for all submissions
        let mut accepted_count = 0;
        for handle in handles {
            let response = handle.await?;
            if response.status == SubmissionStatus::Accepted {
                accepted_count += 1;
            }
        }

        assert_eq!(accepted_count, 5);
        assert_eq!(mempool.size().await, 5);

        Ok(())
    }
}
