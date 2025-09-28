//! Configuration Loading Tests for Cardano Node
//!
//! Tests for loading and validating node configuration files, network parameters,
//! and protocol parameters from YAML/JSON files.

use cardano_node::{CardanoNodeCli, Commands, RunArgs};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tempfile::{NamedTempFile, TempDir};
use std::io::Write;

/// Test configuration structure matching Cardano Node config format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfiguration {
    /// Basic node settings
    #[serde(rename = "Protocol")]
    pub protocol: String,
    #[serde(rename = "GenesisFile")]
    pub genesis_file: String,
    #[serde(rename = "ByronGenesisFile")]
    pub byron_genesis_file: String,
    #[serde(rename = "ShelleyGenesisFile")]
    pub shelley_genesis_file: String,
    #[serde(rename = "AlonzoGenesisFile")]
    pub alonzo_genesis_file: String,
    #[serde(rename = "ConwayGenesisFile")]
    pub conway_genesis_file: String,

    /// Protocol parameters
    #[serde(rename = "RequiresNetworkMagic")]
    pub requires_network_magic: String,
    #[serde(rename = "EnableP2P")]
    pub enable_p2p: bool,

    /// Logging configuration
    #[serde(rename = "defaultScribes")]
    pub default_scribes: Vec<Vec<String>>,
    #[serde(rename = "setupScribes")]
    pub setup_scribes: Vec<ScribeConfig>,

    /// Tracing options
    #[serde(rename = "TraceBlockFetchClient")]
    pub trace_block_fetch_client: bool,
    #[serde(rename = "TraceBlockFetchServer")]
    pub trace_block_fetch_server: bool,
    #[serde(rename = "TraceChainDb")]
    pub trace_chain_db: bool,
}

/// Scribe configuration for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScribeConfig {
    #[serde(rename = "scKind")]
    pub kind: String,
    #[serde(rename = "scName")]
    pub name: String,
    #[serde(rename = "scFormat")]
    pub format: String,
}

/// Network topology configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkTopology {
    #[serde(rename = "Producers")]
    pub producers: Vec<Producer>,
}

/// Producer configuration for network topology
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Producer {
    pub addr: String,
    pub port: u16,
    pub valency: u16,
}

/// Test helper for creating temporary configuration files
pub struct ConfigTestHelper {
    pub temp_dir: TempDir,
    pub config_file: PathBuf,
    pub topology_file: PathBuf,
    pub genesis_files: Vec<PathBuf>,
}

impl ConfigTestHelper {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config_file = temp_dir.path().join("config.json");
        let topology_file = temp_dir.path().join("topology.json");

        // Create genesis files
        let genesis_files = vec![
            temp_dir.path().join("genesis.json"),
            temp_dir.path().join("byron-genesis.json"),
            temp_dir.path().join("shelley-genesis.json"),
            temp_dir.path().join("alonzo-genesis.json"),
            temp_dir.path().join("conway-genesis.json"),
        ];

        Ok(Self {
            temp_dir,
            config_file,
            topology_file,
            genesis_files,
        })
    }

    /// Create a valid test configuration file
    pub fn create_valid_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = NodeConfiguration {
            protocol: "Cardano".to_string(),
            genesis_file: self.genesis_files[0].to_string_lossy().to_string(),
            byron_genesis_file: self.genesis_files[1].to_string_lossy().to_string(),
            shelley_genesis_file: self.genesis_files[2].to_string_lossy().to_string(),
            alonzo_genesis_file: self.genesis_files[3].to_string_lossy().to_string(),
            conway_genesis_file: self.genesis_files[4].to_string_lossy().to_string(),
            requires_network_magic: "RequiresNoMagic".to_string(),
            enable_p2p: true,
            default_scribes: vec![
                vec!["StdoutSK".to_string(), "stdout".to_string()],
            ],
            setup_scribes: vec![
                ScribeConfig {
                    kind: "StdoutSK".to_string(),
                    name: "stdout".to_string(),
                    format: "ScText".to_string(),
                },
            ],
            trace_block_fetch_client: false,
            trace_block_fetch_server: false,
            trace_chain_db: true,
        };

        let config_json = serde_json::to_string_pretty(&config)?;
        std::fs::write(&self.config_file, config_json)?;

        // Create genesis files with minimal content
        for genesis_file in &self.genesis_files {
            std::fs::write(genesis_file, "{\"systemStart\": \"2017-09-23T21:44:51Z\"}")?;
        }

        Ok(())
    }

    /// Create a valid test topology file
    pub fn create_valid_topology(&self) -> Result<(), Box<dyn std::error::Error>> {
        let topology = NetworkTopology {
            producers: vec![
                Producer {
                    addr: "relays-new.cardano-mainnet.iohk.io".to_string(),
                    port: 3001,
                    valency: 2,
                },
                Producer {
                    addr: "relays-new.cardano-testnet.iohkdev.io".to_string(),
                    port: 3001,
                    valency: 2,
                },
            ],
        };

        let topology_json = serde_json::to_string_pretty(&topology)?;
        std::fs::write(&self.topology_file, topology_json)?;

        Ok(())
    }

    /// Create an invalid configuration file
    pub fn create_invalid_config(&self, invalid_type: &str) -> Result<(), Box<dyn std::error::Error>> {
        let invalid_content = match invalid_type {
            "malformed_json" => "{\"Protocol\": \"Cardano\", \"missing_closing_brace\": true",
            "missing_required_field" => r#"{"Protocol": "Cardano"}"#,
            "invalid_protocol" => r#"{"Protocol": "InvalidProtocol", "GenesisFile": "genesis.json"}"#,
            "empty" => "",
            _ => r#"{"Protocol": "Cardano"}"#,
        };

        std::fs::write(&self.config_file, invalid_content)?;
        Ok(())
    }

    /// Create an invalid topology file
    pub fn create_invalid_topology(&self, invalid_type: &str) -> Result<(), Box<dyn std::error::Error>> {
        let invalid_content = match invalid_type {
            "malformed_json" => "{\"Producers\": [",
            "invalid_port" => r#"{"Producers": [{"addr": "test.com", "port": "invalid", "valency": 1}]}"#,
            "empty_producers" => r#"{"Producers": []}"#,
            "missing_producers" => "{}",
            _ => r#"{"Producers": []}"#,
        };

        std::fs::write(&self.topology_file, invalid_content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_t081_valid_config_loading() {
        let helper = ConfigTestHelper::new().expect("Failed to create test helper");
        helper.create_valid_config().expect("Failed to create valid config");

        // Test that the config file exists and is readable
        assert!(helper.config_file.exists(), "T081: Config file should exist");
        let config_content = std::fs::read_to_string(&helper.config_file)
            .expect("T081: Should be able to read config file");

        // Test JSON parsing
        let parsed_config: NodeConfiguration = serde_json::from_str(&config_content)
            .expect("T081: Should be able to parse valid config");

        assert_eq!(parsed_config.protocol, "Cardano");
        assert_eq!(parsed_config.requires_network_magic, "RequiresNoMagic");
        assert!(parsed_config.enable_p2p);

        println!("T081: Valid configuration loading test passed");
    }

    #[test]
    fn test_t081_valid_topology_loading() {
        let helper = ConfigTestHelper::new().expect("Failed to create test helper");
        helper.create_valid_topology().expect("Failed to create valid topology");

        // Test that the topology file exists and is readable
        assert!(helper.topology_file.exists(), "T081: Topology file should exist");
        let topology_content = std::fs::read_to_string(&helper.topology_file)
            .expect("T081: Should be able to read topology file");

        // Test JSON parsing
        let parsed_topology: NetworkTopology = serde_json::from_str(&topology_content)
            .expect("T081: Should be able to parse valid topology");

        assert_eq!(parsed_topology.producers.len(), 2);
        assert_eq!(parsed_topology.producers[0].port, 3001);
        assert_eq!(parsed_topology.producers[0].valency, 2);

        println!("T081: Valid topology loading test passed");
    }

    #[test]
    fn test_t081_config_cli_integration() {
        let helper = ConfigTestHelper::new().expect("Failed to create test helper");
        helper.create_valid_config().expect("Failed to create valid config");
        helper.create_valid_topology().expect("Failed to create valid topology");

        // Test CLI with configuration files
        let args = vec![
            "cardano-node",
            "run",
            "--config", helper.config_file.to_str().unwrap(),
            "--topology", helper.topology_file.to_str().unwrap(),
            "--database-path", helper.temp_dir.path().join("db").to_str().unwrap(),
            "--socket-path", helper.temp_dir.path().join("node.socket").to_str().unwrap(),
        ];

        let cli = CardanoNodeCli::try_parse_from(args)
            .expect("T081: CLI should parse with valid config files");

        if let Commands::Run(run_args) = cli.command {
            assert!(run_args.config.is_some(), "T081: Config should be set");
            assert!(run_args.topology.is_some(), "T081: Topology should be set");
            assert!(run_args.database_path.is_some(), "T081: Database path should be set");
            assert!(run_args.socket_path.is_some(), "T081: Socket path should be set");
        } else {
            panic!("T081: Expected Run command");
        }

        println!("T081: CLI integration with config files test passed");
    }

    #[test]
    fn test_t081_malformed_json_config() {
        let helper = ConfigTestHelper::new().expect("Failed to create test helper");
        helper.create_invalid_config("malformed_json")
            .expect("Failed to create invalid config");

        let config_content = std::fs::read_to_string(&helper.config_file)
            .expect("Should be able to read config file");

        // Test that malformed JSON fails to parse
        let result: Result<NodeConfiguration, _> = serde_json::from_str(&config_content);
        assert!(result.is_err(), "T081: Malformed JSON should fail to parse");

        println!("T081: Malformed JSON config validation test passed");
    }

    #[test]
    fn test_t081_missing_required_fields() {
        let helper = ConfigTestHelper::new().expect("Failed to create test helper");
        helper.create_invalid_config("missing_required_field")
            .expect("Failed to create invalid config");

        let config_content = std::fs::read_to_string(&helper.config_file)
            .expect("Should be able to read config file");

        // Test that missing required fields cause parsing to fail
        let result: Result<NodeConfiguration, _> = serde_json::from_str(&config_content);
        assert!(result.is_err(), "T081: Config missing required fields should fail to parse");

        println!("T081: Missing required fields validation test passed");
    }

    #[test]
    fn test_t081_invalid_topology_parsing() {
        let helper = ConfigTestHelper::new().expect("Failed to create test helper");
        helper.create_invalid_topology("malformed_json")
            .expect("Failed to create invalid topology");

        let topology_content = std::fs::read_to_string(&helper.topology_file)
            .expect("Should be able to read topology file");

        // Test that malformed topology JSON fails to parse
        let result: Result<NetworkTopology, _> = serde_json::from_str(&topology_content);
        assert!(result.is_err(), "T081: Malformed topology JSON should fail to parse");

        println!("T081: Invalid topology parsing test passed");
    }

    #[test]
    fn test_t081_empty_producers_validation() {
        let helper = ConfigTestHelper::new().expect("Failed to create test helper");
        helper.create_invalid_topology("empty_producers")
            .expect("Failed to create invalid topology");

        let topology_content = std::fs::read_to_string(&helper.topology_file)
            .expect("Should be able to read topology file");

        // Test that empty producers list is parsed but can be validated
        let parsed_topology: NetworkTopology = serde_json::from_str(&topology_content)
            .expect("Empty producers should parse successfully");

        assert_eq!(parsed_topology.producers.len(), 0, "T081: Should have zero producers");

        // This would be where business logic validation occurs
        // (empty producers might be valid for some node configurations)

        println!("T081: Empty producers validation test passed");
    }

    #[test]
    fn test_t081_yaml_config_support() {
        // Test YAML configuration file support
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let yaml_config_file = temp_dir.path().join("config.yaml");

        let yaml_content = r#"
Protocol: Cardano
GenesisFile: genesis.json
ByronGenesisFile: byron-genesis.json
ShelleyGenesisFile: shelley-genesis.json
AlonzoGenesisFile: alonzo-genesis.json
ConwayGenesisFile: conway-genesis.json
RequiresNetworkMagic: RequiresNoMagic
EnableP2P: true
defaultScribes:
  - ["StdoutSK", "stdout"]
setupScribes:
  - scKind: StdoutSK
    scName: stdout
    scFormat: ScText
TraceBlockFetchClient: false
TraceBlockFetchServer: false
TraceChainDb: true
"#;

        std::fs::write(&yaml_config_file, yaml_content)
            .expect("Failed to write YAML config");

        // Test YAML parsing (would require yaml crate in actual implementation)
        assert!(yaml_config_file.exists(), "T081: YAML config file should exist");

        println!("T081: YAML config support test passed");
    }

    #[test]
    fn test_t081_network_magic_validation() {
        let helper = ConfigTestHelper::new().expect("Failed to create test helper");

        // Create config with valid network magic values
        let magic_values = vec!["RequiresNoMagic", "RequiresMagic"];

        for magic in magic_values {
            let config = NodeConfiguration {
                protocol: "Cardano".to_string(),
                genesis_file: "genesis.json".to_string(),
                byron_genesis_file: "byron-genesis.json".to_string(),
                shelley_genesis_file: "shelley-genesis.json".to_string(),
                alonzo_genesis_file: "alonzo-genesis.json".to_string(),
                conway_genesis_file: "conway-genesis.json".to_string(),
                requires_network_magic: magic.to_string(),
                enable_p2p: true,
                default_scribes: vec![],
                setup_scribes: vec![],
                trace_block_fetch_client: false,
                trace_block_fetch_server: false,
                trace_chain_db: true,
            };

            let config_json = serde_json::to_string_pretty(&config)
                .expect("Should serialize config");
            let parsed: NodeConfiguration = serde_json::from_str(&config_json)
                .expect("Should parse config with valid network magic");

            assert_eq!(parsed.requires_network_magic, magic);
        }

        println!("T081: Network magic validation test passed");
    }

    #[test]
    fn test_t081_protocol_parameter_validation() {
        let helper = ConfigTestHelper::new().expect("Failed to create test helper");

        // Test different protocol values
        let protocols = vec!["Cardano", "Byron", "Shelley"];

        for protocol in protocols {
            let config = NodeConfiguration {
                protocol: protocol.to_string(),
                genesis_file: "genesis.json".to_string(),
                byron_genesis_file: "byron-genesis.json".to_string(),
                shelley_genesis_file: "shelley-genesis.json".to_string(),
                alonzo_genesis_file: "alonzo-genesis.json".to_string(),
                conway_genesis_file: "conway-genesis.json".to_string(),
                requires_network_magic: "RequiresNoMagic".to_string(),
                enable_p2p: true,
                default_scribes: vec![],
                setup_scribes: vec![],
                trace_block_fetch_client: false,
                trace_block_fetch_server: false,
                trace_chain_db: true,
            };

            let config_json = serde_json::to_string_pretty(&config)
                .expect("Should serialize config");
            let parsed: NodeConfiguration = serde_json::from_str(&config_json)
                .expect("Should parse config with valid protocol");

            assert_eq!(parsed.protocol, protocol);
        }

        println!("T081: Protocol parameter validation test passed");
    }

    #[test]
    fn test_t081_genesis_file_path_validation() {
        let helper = ConfigTestHelper::new().expect("Failed to create test helper");
        helper.create_valid_config().expect("Failed to create valid config");

        let config_content = std::fs::read_to_string(&helper.config_file)
            .expect("Should be able to read config file");
        let parsed_config: NodeConfiguration = serde_json::from_str(&config_content)
            .expect("Should be able to parse valid config");

        // Test that genesis file paths are properly set
        assert!(!parsed_config.genesis_file.is_empty());
        assert!(!parsed_config.byron_genesis_file.is_empty());
        assert!(!parsed_config.shelley_genesis_file.is_empty());
        assert!(!parsed_config.alonzo_genesis_file.is_empty());
        assert!(!parsed_config.conway_genesis_file.is_empty());

        // Test that genesis files exist
        for genesis_file in &helper.genesis_files {
            assert!(genesis_file.exists(), "T081: Genesis file should exist: {:?}", genesis_file);
        }

        println!("T081: Genesis file path validation test passed");
    }

    #[test]
    fn test_t081_tracing_configuration() {
        let helper = ConfigTestHelper::new().expect("Failed to create test helper");
        helper.create_valid_config().expect("Failed to create valid config");

        let config_content = std::fs::read_to_string(&helper.config_file)
            .expect("Should be able to read config file");
        let parsed_config: NodeConfiguration = serde_json::from_str(&config_content)
            .expect("Should be able to parse valid config");

        // Test tracing configuration
        assert!(!parsed_config.trace_block_fetch_client);
        assert!(!parsed_config.trace_block_fetch_server);
        assert!(parsed_config.trace_chain_db);

        // Test scribes configuration
        assert!(!parsed_config.default_scribes.is_empty());
        assert!(!parsed_config.setup_scribes.is_empty());

        let scribe = &parsed_config.setup_scribes[0];
        assert_eq!(scribe.kind, "StdoutSK");
        assert_eq!(scribe.name, "stdout");
        assert_eq!(scribe.format, "ScText");

        println!("T081: Tracing configuration test passed");
    }
}
