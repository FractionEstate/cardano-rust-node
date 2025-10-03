//! API Integration Tests Module
//!
//! This module contains comprehensive integration tests for all Cardano Node
//! API interfaces including local socket communication, CLI interface, and
//! transaction submission API.

pub mod test_local_socket;
pub mod test_cli_interface;
pub mod test_submit_api;

// Re-export commonly used types for convenience
pub use test_local_socket::{MockLocalSocketServer, LocalSocketClient};
pub use test_cli_interface::{CliExecutor, CommandOutput, MockNodeConfig};
pub use test_submit_api::{MockSubmitApi, MockMempool, MockTransaction, ValidationError};
