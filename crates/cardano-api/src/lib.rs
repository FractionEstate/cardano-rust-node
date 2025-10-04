//! Cardano Node API Layer
//!
//! Provides external interfaces for interacting with the Cardano Node.

pub mod local_socket;
pub mod mempool_bridge;
pub mod rest_api;
pub mod submit_api;

pub use mempool_bridge::{MempoolBridge, MempoolBridgeConfig, MempoolBridgeStats};

/// API error types
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Request error: {0}")]
    RequestError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        ApiError::SerializationError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ApiError>;
