//! Cardano Node API Layer
//!
//! Provides external interfaces for interacting with the Cardano Node.

pub mod local_socket;
pub mod rest_api;

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

pub type Result<T> = std::result::Result<T, ApiError>;
