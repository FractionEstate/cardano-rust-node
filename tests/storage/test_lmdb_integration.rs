//! Integration tests for LMDB backend
//!
//! This binary test file runs the T066 LMDB backend test suite.

#[cfg(test)]
mod tests {
    use super::test_lmdb_backend::*;

    // Re-export all tests from the LMDB backend module
    // This allows cargo test to discover them when running integration tests
}
