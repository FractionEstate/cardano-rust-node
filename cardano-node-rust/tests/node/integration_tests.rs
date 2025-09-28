//! Node Integration Tests
//!
//! Integration test wrappers for T081 Configuration Loading Tests

use super::test_config_loading::*;

#[test]
fn test_t081_valid_config_loading_integration() {
    test_t081_valid_config_loading();
}

#[test]
fn test_t081_malformed_json_config_integration() {
    test_t081_malformed_json_config();
}

#[test]
fn test_t081_missing_required_fields_integration() {
    test_t081_missing_required_fields();
}

#[test]
fn test_t081_invalid_network_magic_integration() {
    test_t081_invalid_network_magic();
}

#[test]
fn test_t081_invalid_listening_port_integration() {
    test_t081_invalid_listening_port();
}

#[test]
fn test_t081_missing_config_file_integration() {
    test_t081_missing_config_file();
}

#[test]
fn test_t081_config_file_path_validation_integration() {
    test_t081_config_file_path_validation();
}

#[test]
fn test_t081_network_topology_validation_integration() {
    test_t081_network_topology_validation();
}

#[test]
fn test_t081_empty_network_topology_integration() {
    test_t081_empty_network_topology();
}

#[test]
fn test_t081_invalid_peer_addresses_integration() {
    test_t081_invalid_peer_addresses();
}

#[test]
fn test_t081_yaml_config_loading_integration() {
    test_t081_yaml_config_loading();
}

#[test]
fn test_t081_config_reload_functionality_integration() {
    test_t081_config_reload_functionality();
}
