//! Local Socket Client
//!
//! Client implementation for connecting to the Cardano Node local socket
//! and sending requests. Compatible with cardano-cli and other tools.

use serde_json::Value;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::debug;

use super::protocol::*;
use crate::{ApiError, Result};

/// Client for communicating with local socket
pub struct LocalSocketClient {
    socket_path: std::path::PathBuf,
}

impl LocalSocketClient {
    /// Create a new client
    pub fn new<P: AsRef<Path>>(socket_path: P) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
        }
    }

    /// Connect to the socket and send a request
    pub async fn send_request(&self, request: LocalSocketRequest) -> Result<LocalSocketResponse> {
        // Connect to socket
        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .map_err(|e| ApiError::RequestError(format!("Failed to connect: {}", e)))?;

        // Serialize request
        let request_json = serde_json::to_string(&request)
            .map_err(|e| ApiError::SerializationError(e.to_string()))?;

        debug!("Sending request: {}", request_json);

        // Send request
        stream
            .write_all(format!("{}\n", request_json).as_bytes())
            .await
            .map_err(|e| ApiError::RequestError(format!("Failed to send request: {}", e)))?;

        stream
            .flush()
            .await
            .map_err(|e| ApiError::RequestError(format!("Failed to flush request: {}", e)))?;

        // Read response
        let mut reader = BufReader::new(&mut stream);
        let mut response_line = String::new();

        reader
            .read_line(&mut response_line)
            .await
            .map_err(|e| ApiError::RequestError(format!("Failed to read response: {}", e)))?;

        debug!("Received response: {}", response_line);

        // Parse response
        let response: LocalSocketResponse =
            serde_json::from_str(response_line.trim()).map_err(|e| {
                ApiError::SerializationError(format!("Failed to parse response: {}", e))
            })?;

        Ok(response)
    }

    /// Send a simple request without parameters
    pub async fn send_simple_request(&self, method: &str) -> Result<LocalSocketResponse> {
        let request = LocalSocketRequest {
            method: method.to_string(),
            params: None,
            id: Some(serde_json::json!(1)),
        };

        self.send_request(request).await
    }

    /// Send a request with parameters
    pub async fn send_request_with_params(
        &self,
        method: &str,
        params: Value,
    ) -> Result<LocalSocketResponse> {
        let request = LocalSocketRequest {
            method: method.to_string(),
            params: Some(params),
            id: Some(serde_json::json!(1)),
        };

        self.send_request(request).await
    }

    /// Query the current chain tip
    pub async fn query_chain_tip(&self) -> Result<Value> {
        let response = self.send_simple_request("queryChainTip").await?;

        if let Some(result) = response.result {
            Ok(result)
        } else if let Some(error) = response.error {
            Err(ApiError::RequestError(error.message))
        } else {
            Err(ApiError::RequestError(
                "Invalid response format".to_string(),
            ))
        }
    }

    /// Query a block by hash
    pub async fn query_block(&self, block_hash: &str) -> Result<Value> {
        let params = serde_json::json!({
            "blockHash": block_hash
        });

        let response = self.send_request_with_params("queryBlock", params).await?;

        if let Some(result) = response.result {
            Ok(result)
        } else if let Some(error) = response.error {
            Err(ApiError::RequestError(error.message))
        } else {
            Err(ApiError::RequestError(
                "Invalid response format".to_string(),
            ))
        }
    }

    /// Query a transaction by ID
    pub async fn query_transaction(&self, tx_id: &str) -> Result<Value> {
        let params = serde_json::json!({
            "txId": tx_id
        });

        let response = self
            .send_request_with_params("queryTransaction", params)
            .await?;

        if let Some(result) = response.result {
            Ok(result)
        } else if let Some(error) = response.error {
            Err(ApiError::RequestError(error.message))
        } else {
            Err(ApiError::RequestError(
                "Invalid response format".to_string(),
            ))
        }
    }

    /// Query UTXOs for an address
    pub async fn query_utxos(&self, address: &str) -> Result<Value> {
        let params = serde_json::json!({
            "address": address
        });

        let response = self.send_request_with_params("queryUtxos", params).await?;

        if let Some(result) = response.result {
            Ok(result)
        } else if let Some(error) = response.error {
            Err(ApiError::RequestError(error.message))
        } else {
            Err(ApiError::RequestError(
                "Invalid response format".to_string(),
            ))
        }
    }

    /// Submit a transaction
    pub async fn submit_transaction(&self, tx_data: Value) -> Result<Value> {
        let response = self
            .send_request_with_params("submitTransaction", tx_data)
            .await?;

        if let Some(result) = response.result {
            Ok(result)
        } else if let Some(error) = response.error {
            Err(ApiError::RequestError(error.message))
        } else {
            Err(ApiError::RequestError(
                "Invalid response format".to_string(),
            ))
        }
    }

    /// Query protocol parameters
    pub async fn query_protocol_params(&self) -> Result<Value> {
        let response = self.send_simple_request("queryProtocolParams").await?;

        if let Some(result) = response.result {
            Ok(result)
        } else if let Some(error) = response.error {
            Err(ApiError::RequestError(error.message))
        } else {
            Err(ApiError::RequestError(
                "Invalid response format".to_string(),
            ))
        }
    }

    /// Query node status
    pub async fn query_node_status(&self) -> Result<Value> {
        let response = self.send_simple_request("queryNodeStatus").await?;

        if let Some(result) = response.result {
            Ok(result)
        } else if let Some(error) = response.error {
            Err(ApiError::RequestError(error.message))
        } else {
            Err(ApiError::RequestError(
                "Invalid response format".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn get_test_socket_path() -> std::path::PathBuf {
        env::temp_dir().join("cardano-node-client-test.sock")
    }

    #[test]
    fn test_client_creation() {
        let socket_path = get_test_socket_path();
        let _client = LocalSocketClient::new(&socket_path);
    }

    #[test]
    fn test_request_serialization() {
        let request = LocalSocketRequest {
            method: "queryChainTip".to_string(),
            params: None,
            id: Some(serde_json::json!(1)),
        };

        let json = serde_json::to_string(&request).expect("Failed to serialize request");
        assert!(json.contains("queryChainTip"));
    }
}
