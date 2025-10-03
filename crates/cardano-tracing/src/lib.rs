//! Cardano Tracing and Observability
//!
//! Provides structured logging, metrics, and observability for Cardano Node.

/// Tracing error types
#[derive(Debug, thiserror::Error)]
pub enum TracingError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, TracingError>;
