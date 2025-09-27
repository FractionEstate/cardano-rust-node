//! Transaction Validation
//!
//! Comprehensive transaction validation including syntax, semantics,
//! and script execution validation.

use crate::{ApiError, Result};
use super::{ParsedTransaction, SubmitApiConfig};
use serde_json::Value;
use tracing::{debug, warn};

/// Transaction validation error types
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// Invalid transaction syntax
    SyntaxError(String),
    /// Semantic validation failure
    SemanticError(String),
    /// Script execution failure
    ScriptError(String),
    /// Insufficient funds
    InsufficientFunds(String),
    /// Invalid signature
    InvalidSignature(String),
    /// UTxO not found
    UtxoNotFound(String),
    /// Expired transaction
    TransactionExpired(String),
    /// Fee calculation error
    FeeCalculationError(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::SyntaxError(msg) => write!(f, "Syntax error: {}", msg),
            ValidationError::SemanticError(msg) => write!(f, "Semantic error: {}", msg),
            ValidationError::ScriptError(msg) => write!(f, "Script error: {}", msg),
            ValidationError::InsufficientFunds(msg) => write!(f, "Insufficient funds: {}", msg),
            ValidationError::InvalidSignature(msg) => write!(f, "Invalid signature: {}", msg),
            ValidationError::UtxoNotFound(msg) => write!(f, "UTxO not found: {}", msg),
            ValidationError::TransactionExpired(msg) => write!(f, "Transaction expired: {}", msg),
            ValidationError::FeeCalculationError(msg) => write!(f, "Fee calculation error: {}", msg),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Trait for transaction validation
#[async_trait::async_trait]
pub trait TransactionValidator: Send + Sync {
    /// Validate a parsed transaction
    async fn validate_transaction(&self, tx: &ParsedTransaction) -> Result<()>;
}

/// Comprehensive transaction validator
pub struct CardanoTransactionValidator {
    config: SubmitApiConfig,
    utxo_provider: Box<dyn UtxoProvider>,
    script_validator: Box<dyn ScriptValidator>,
}

impl CardanoTransactionValidator {
    /// Create a new transaction validator
    pub fn new(
        config: SubmitApiConfig,
        utxo_provider: Box<dyn UtxoProvider>,
        script_validator: Box<dyn ScriptValidator>,
    ) -> Self {
        Self {
            config,
            utxo_provider,
            script_validator,
        }
    }

    /// Validate transaction syntax
    async fn validate_syntax(&self, tx: &ParsedTransaction) -> Result<()> {
        debug!("Validating transaction syntax for {}", tx.id);

        // Check transaction size
        if tx.size > self.config.max_tx_size {
            return Err(ApiError::RequestError(format!(
                "Transaction size {} exceeds maximum {}",
                tx.size, self.config.max_tx_size
            )));
        }

        // Check minimum fee
        if tx.fee < 155381 { // Minimum fee constant
            return Err(ApiError::RequestError(
                "Transaction fee below minimum".to_string()
            ));
        }

        // Validate inputs are not empty
        if tx.inputs.is_empty() {
            return Err(ApiError::RequestError(
                "Transaction must have at least one input".to_string()
            ));
        }

        // Validate outputs are not empty
        if tx.outputs.is_empty() {
            return Err(ApiError::RequestError(
                "Transaction must have at least one output".to_string()
            ));
        }

        Ok(())
    }

    /// Validate transaction semantics (UTxO availability, balance, etc.)
    async fn validate_semantics(&self, tx: &ParsedTransaction) -> Result<()> {
        debug!("Validating transaction semantics for {}", tx.id);

        let mut total_input_value = 0u64;
        let mut total_output_value = 0u64;

        // Validate inputs and calculate total input value
        for input in &tx.inputs {
            match self.utxo_provider.get_utxo(&input.tx_id, input.output_index).await {
                Ok(Some(utxo)) => {
                    total_input_value = total_input_value.checked_add(utxo.value)
                        .ok_or_else(|| ApiError::RequestError("Input value overflow".to_string()))?;
                }
                Ok(None) => {
                    return Err(ApiError::RequestError(format!(
                        "UTxO not found: {}#{}", input.tx_id, input.output_index
                    )));
                }
                Err(e) => {
                    return Err(ApiError::RequestError(format!(
                        "Failed to fetch UTxO: {}", e
                    )));
                }
            }
        }

        // Calculate total output value
        for output in &tx.outputs {
            total_output_value = total_output_value.checked_add(output.value)
                .ok_or_else(|| ApiError::RequestError("Output value overflow".to_string()))?;
        }

        // Validate balance equation: inputs = outputs + fee
        let expected_input = total_output_value.checked_add(tx.fee)
            .ok_or_else(|| ApiError::RequestError("Fee calculation overflow".to_string()))?;

        if total_input_value != expected_input {
            return Err(ApiError::RequestError(format!(
                "Transaction not balanced: inputs={}, outputs={}, fee={}",
                total_input_value, total_output_value, tx.fee
            )));
        }

        Ok(())
    }

    /// Validate scripts and signatures
    async fn validate_scripts(&self, tx: &ParsedTransaction) -> Result<()> {
        debug!("Validating transaction scripts for {}", tx.id);

        // For each input, validate any required scripts
        for input in &tx.inputs {
            if let Some(witness) = &input.witness {
                // Validate script witness
                self.script_validator.validate_script_witness(
                    &input.tx_id,
                    input.output_index,
                    witness,
                    tx,
                ).await?;
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl TransactionValidator for CardanoTransactionValidator {
    async fn validate_transaction(&self, tx: &ParsedTransaction) -> Result<()> {
        // Validate transaction syntax
        self.validate_syntax(tx).await?;

        // Validate transaction semantics
        self.validate_semantics(tx).await?;

        // Validate scripts and signatures
        if self.config.strict_validation {
            self.validate_scripts(tx).await?;
        }

        debug!("Transaction {} passed all validation", tx.id);
        Ok(())
    }
}

/// UTxO information for validation
#[derive(Debug, Clone)]
pub struct UtxoInfo {
    /// UTxO value in lovelace
    pub value: u64,
    /// UTxO address
    pub address: String,
    /// Native assets (optional)
    pub assets: Option<Value>,
    /// Datum hash (optional)
    pub datum_hash: Option<String>,
    /// Script reference (optional)
    pub script_ref: Option<String>,
}

/// Trait for UTxO provider
#[async_trait::async_trait]
pub trait UtxoProvider: Send + Sync {
    /// Get UTxO information by transaction ID and output index
    async fn get_utxo(&self, tx_id: &str, output_index: u32) -> Result<Option<UtxoInfo>>;
}

/// Trait for script validation
#[async_trait::async_trait]
pub trait ScriptValidator: Send + Sync {
    /// Validate script witness for a given input
    async fn validate_script_witness(
        &self,
        tx_id: &str,
        output_index: u32,
        witness: &str,
        tx: &ParsedTransaction,
    ) -> Result<()>;
}

/// Mock UTxO provider for testing
pub struct MockUtxoProvider;

#[async_trait::async_trait]
impl UtxoProvider for MockUtxoProvider {
    async fn get_utxo(&self, _tx_id: &str, _output_index: u32) -> Result<Option<UtxoInfo>> {
        // Mock implementation - always return a valid UTxO with 2 ADA
        Ok(Some(UtxoInfo {
            value: 2000000, // 2 ADA in lovelace
            address: "addr1qx2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3n0d3vllmyqwsx5wktcd8cc3sq835lu7drv2xwl2wywfgse35a3x".to_string(),
            assets: None,
            datum_hash: None,
            script_ref: None,
        }))
    }
}

/// Mock script validator for testing
pub struct MockScriptValidator;

#[async_trait::async_trait]
impl ScriptValidator for MockScriptValidator {
    async fn validate_script_witness(
        &self,
        _tx_id: &str,
        _output_index: u32,
        _witness: &str,
        _tx: &ParsedTransaction,
    ) -> Result<()> {
        // Mock implementation - always pass
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::submit_api::{TransactionInput, TransactionOutput};

    #[tokio::test]
    async fn test_syntax_validation() {
        let config = SubmitApiConfig::default();
        let utxo_provider = Box::new(MockUtxoProvider);
        let script_validator = Box::new(MockScriptValidator);

        let validator = CardanoTransactionValidator::new(config, utxo_provider, script_validator);

        let tx = ParsedTransaction {
            id: "test_tx".to_string(),
            cbor_data: vec![0u8; 256],
            size: 256,
            fee: 200000,
            inputs: vec![TransactionInput {
                tx_id: "input_tx".to_string(),
                output_index: 0,
                witness: None,
            }],
            outputs: vec![TransactionOutput {
                address: "addr1test".to_string(),
                value: 1000000,
                assets: None,
                datum_hash: None,
            }],
        };

        assert!(validator.validate_syntax(&tx).await.is_ok());
    }

    #[tokio::test]
    async fn test_invalid_fee() {
        let config = SubmitApiConfig::default();
        let utxo_provider = Box::new(MockUtxoProvider);
        let script_validator = Box::new(MockScriptValidator);

        let validator = CardanoTransactionValidator::new(config, utxo_provider, script_validator);

        let tx = ParsedTransaction {
            id: "test_tx".to_string(),
            cbor_data: vec![0u8; 256],
            size: 256,
            fee: 100000, // Below minimum
            inputs: vec![TransactionInput {
                tx_id: "input_tx".to_string(),
                output_index: 0,
                witness: None,
            }],
            outputs: vec![TransactionOutput {
                address: "addr1test".to_string(),
                value: 1000000,
                assets: None,
                datum_hash: None,
            }],
        };

        assert!(validator.validate_syntax(&tx).await.is_err());
    }

    #[tokio::test]
    async fn test_semantic_validation() {
        let config = SubmitApiConfig::default();
        let utxo_provider = Box::new(MockUtxoProvider);
        let script_validator = Box::new(MockScriptValidator);

        let validator = CardanoTransactionValidator::new(config, utxo_provider, script_validator);

        // Valid balanced transaction
        let tx = ParsedTransaction {
            id: "test_tx".to_string(),
            cbor_data: vec![0u8; 256],
            size: 256,
            fee: 174593,
            inputs: vec![TransactionInput {
                tx_id: "input_tx".to_string(),
                output_index: 0,
                witness: None,
            }],
            outputs: vec![TransactionOutput {
                address: "addr1test".to_string(),
                value: 1825407, // 2000000 - 174593 (fee)
                assets: None,
                datum_hash: None,
            }],
        };

        assert!(validator.validate_semantics(&tx).await.is_ok());
    }
}
