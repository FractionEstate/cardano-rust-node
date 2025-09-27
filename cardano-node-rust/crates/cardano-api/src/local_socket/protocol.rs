//! Local Socket Protocol Definitions
//!
//! Defines the JSON-RPC style protocol for Unix domain socket communication
//! compatible with cardano-cli and other tools.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Request message for local socket protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalSocketRequest {
    /// Method name
    pub method: String,
    /// Request parameters
    pub params: Option<Value>,
    /// Request ID for correlation
    pub id: Option<Value>,
}

/// Response message for local socket protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalSocketResponse {
    /// Response result (success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error information (failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<LocalSocketError>,
    /// Request ID for correlation
    pub id: Option<Value>,
}

/// Error response for local socket protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalSocketError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl LocalSocketResponse {
    /// Create a success response
    pub fn success(result: Value, id: Option<Value>) -> Self {
        Self {
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response
    pub fn error(error: LocalSocketError, id: Option<Value>) -> Self {
        Self {
            result: None,
            error: Some(error),
            id,
        }
    }
}

impl LocalSocketError {
    /// Create a new error
    pub fn new(code: i32, message: String) -> Self {
        Self {
            code,
            message,
            data: None,
        }
    }

    /// Create a parse error
    pub fn parse_error() -> Self {
        Self::new(-32700, "Parse error".to_string())
    }

    /// Create an invalid request error
    pub fn invalid_request() -> Self {
        Self::new(-32600, "Invalid Request".to_string())
    }

    /// Create a method not found error
    pub fn method_not_found() -> Self {
        Self::new(-32601, "Method not found".to_string())
    }

    /// Create an invalid params error
    pub fn invalid_params() -> Self {
        Self::new(-32602, "Invalid params".to_string())
    }

    /// Create an internal error
    pub fn internal_error(message: String) -> Self {
        Self::new(-32603, format!("Internal error: {}", message))
    }

    /// Create a custom application error
    pub fn application_error(code: i32, message: String) -> Self {
        Self::new(code, message)
    }
}

/// Supported methods in the local socket protocol
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalSocketMethod {
    /// Query the current chain tip
    QueryChainTip,
    /// Query a specific block by hash
    QueryBlock,
    /// Query a specific transaction by ID
    QueryTransaction,
    /// Query UTXOs for an address
    QueryUtxos,
    /// Submit a transaction
    SubmitTransaction,
    /// Query protocol parameters
    QueryProtocolParams,
    /// Query stake pools
    QueryStakePools,
    /// Query delegation information
    QueryDelegation,
    /// Query node status
    QueryNodeStatus,
}

impl LocalSocketMethod {
    /// Parse method from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "queryChainTip" => Some(Self::QueryChainTip),
            "queryBlock" => Some(Self::QueryBlock),
            "queryTransaction" => Some(Self::QueryTransaction),
            "queryUtxos" => Some(Self::QueryUtxos),
            "submitTransaction" => Some(Self::SubmitTransaction),
            "queryProtocolParams" => Some(Self::QueryProtocolParams),
            "queryStakePools" => Some(Self::QueryStakePools),
            "queryDelegation" => Some(Self::QueryDelegation),
            "queryNodeStatus" => Some(Self::QueryNodeStatus),
            _ => None,
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::QueryChainTip => "queryChainTip",
            Self::QueryBlock => "queryBlock",
            Self::QueryTransaction => "queryTransaction",
            Self::QueryUtxos => "queryUtxos",
            Self::SubmitTransaction => "submitTransaction",
            Self::QueryProtocolParams => "queryProtocolParams",
            Self::QueryStakePools => "queryStakePools",
            Self::QueryDelegation => "queryDelegation",
            Self::QueryNodeStatus => "queryNodeStatus",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_request_serialization() {
        let request = LocalSocketRequest {
            method: "queryChainTip".to_string(),
            params: None,
            id: Some(json!(1)),
        };

        let json = serde_json::to_string(&request).expect("Failed to serialize");
        let deserialized: LocalSocketRequest = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(request.method, deserialized.method);
        assert_eq!(request.id, deserialized.id);
    }

    #[test]
    fn test_response_serialization() {
        let response = LocalSocketResponse::success(json!({"tip": "hash"}), Some(json!(1)));

        let json = serde_json::to_string(&response).expect("Failed to serialize");
        let deserialized: LocalSocketResponse = serde_json::from_str(&json).expect("Failed to deserialize");

        assert!(deserialized.result.is_some());
        assert!(deserialized.error.is_none());
    }

    #[test]
    fn test_error_response() {
        let error = LocalSocketError::method_not_found();
        let response = LocalSocketResponse::error(error, Some(json!(1)));

        assert!(response.result.is_none());
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32601);
    }

    #[test]
    fn test_method_parsing() {
        assert_eq!(LocalSocketMethod::from_str("queryChainTip"), Some(LocalSocketMethod::QueryChainTip));
        assert_eq!(LocalSocketMethod::from_str("invalidMethod"), None);
    }
}
