//! Node Integration Tests Module
//!
//! Tests for node-level functionality including CLI parsing and configuration loading.

pub mod test_cli_parsing;
pub mod test_config_loading;
pub mod test_node_lifecycle;
pub mod integration_tests;

// Re-export test functions for integration test execution
pub use test_cli_parsing::*;
pub use test_config_loading::*;
pub use test_node_lifecycle::*;
pub use integration_tests::*;
