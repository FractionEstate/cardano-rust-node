//! Block Producer Configuration
//!
//! Configuration for running a Cardano node as a stake pool operator and block producer.
//! This includes management of cryptographic keys, operational certificates, and
//! block forging parameters.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Complete block producer configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockProducerConfig {
    /// Enable block production (forging)
    pub enabled: bool,

    /// Pool identification
    pub pool_id: Option<String>,

    /// VRF (Verifiable Random Function) key configuration
    pub vrf_key: VrfKeyConfig,

    /// KES (Key Evolving Signature) key configuration
    pub kes_key: KesKeyConfig,

    /// Operational certificate configuration
    pub operational_cert: OperationalCertConfig,

    /// Cold key configuration (optional, for key operations)
    pub cold_key: Option<ColdKeyConfig>,

    /// Forging behavior settings
    pub forging_behavior: ForgingBehavior,

    /// Leader schedule settings
    pub leader_schedule: LeaderScheduleConfig,
}

/// VRF key configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VrfKeyConfig {
    /// Path to VRF signing key file
    pub signing_key_file: PathBuf,

    /// Path to VRF verification key file (optional, can be derived)
    pub verification_key_file: Option<PathBuf>,

    /// Key format: "cardano-cli" (default) or "raw-hex"
    #[serde(default = "default_key_format")]
    pub format: KeyFormat,
}

/// KES key configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KesKeyConfig {
    /// Path to KES signing key file
    pub signing_key_file: PathBuf,

    /// Path to KES verification key file (optional)
    pub verification_key_file: Option<PathBuf>,

    /// Current KES period
    pub kes_period: u64,

    /// Maximum KES evolution period (typically 62 or 90 days worth of periods)
    #[serde(default = "default_max_kes_evolutions")]
    pub max_kes_evolutions: u64,

    /// Start KES period (when the key was generated)
    pub start_kes_period: u64,

    /// Key format
    #[serde(default = "default_key_format")]
    pub format: KeyFormat,

    /// Auto-rotation settings (optional)
    pub auto_rotation: Option<KesAutoRotation>,
}

/// Operational certificate configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationalCertConfig {
    /// Path to operational certificate file
    pub cert_file: PathBuf,

    /// Operational certificate issue number (counter value)
    pub issue_counter: u64,

    /// Path to counter file (tracks cert issue number)
    pub counter_file: Option<PathBuf>,
}

/// Cold key configuration (for advanced operations)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColdKeyConfig {
    /// Path to cold signing key (highly sensitive!)
    /// Should be kept offline and only used for key rotation
    pub signing_key_file: Option<PathBuf>,

    /// Path to cold verification key
    pub verification_key_file: PathBuf,

    /// Key format
    #[serde(default = "default_key_format")]
    pub format: KeyFormat,
}

/// KES key auto-rotation settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KesAutoRotation {
    /// Enable automatic KES key rotation
    pub enabled: bool,

    /// Rotate this many periods before expiration (safety margin)
    #[serde(default = "default_rotation_margin")]
    pub rotation_margin_periods: u64,

    /// Directory for storing rotated keys
    pub rotation_dir: PathBuf,

    /// Alert when this many periods remain before expiration
    #[serde(default = "default_alert_margin")]
    pub alert_margin_periods: u64,
}

/// Forging behavior configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgingBehavior {
    /// Delay before forging a block (in milliseconds)
    /// Used to gather more transactions
    #[serde(default = "default_forging_delay")]
    pub forging_delay_ms: u64,

    /// Maximum number of transactions per block
    #[serde(default = "default_max_txs_per_block")]
    pub max_txs_per_block: usize,

    /// Maximum block size in bytes
    #[serde(default = "default_max_block_size")]
    pub max_block_size_bytes: usize,

    /// Prefer higher fee transactions
    #[serde(default = "default_prefer_high_fees")]
    pub prefer_high_fees: bool,

    /// Include mempool transactions (if false, only produce empty blocks)
    #[serde(default = "default_include_txs")]
    pub include_txs: bool,

    /// Continue forging if mempool validation fails for some transactions
    #[serde(default = "default_continue_on_tx_validation_failure")]
    pub continue_on_tx_validation_failure: bool,
}

/// Leader schedule configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderScheduleConfig {
    /// Pre-calculate leader schedule for this many epochs ahead
    #[serde(default = "default_schedule_lookahead_epochs")]
    pub schedule_lookahead_epochs: u64,

    /// Log the leader schedule (can reveal slot leadership information)
    #[serde(default)]
    pub log_schedule: bool,

    /// Export leader schedule to file
    pub export_schedule_file: Option<PathBuf>,

    /// Refresh schedule calculation interval (in slots)
    #[serde(default = "default_schedule_refresh_interval")]
    pub schedule_refresh_interval: u64,
}

/// Key format enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyFormat {
    /// Cardano CLI JSON format (Haskell cardano-cli compatible)
    CardanoCli,
    /// Raw hex-encoded bytes
    RawHex,
    /// Raw binary bytes
    RawBinary,
}

// Default value functions
fn default_key_format() -> KeyFormat {
    KeyFormat::CardanoCli
}

fn default_max_kes_evolutions() -> u64 {
    62 // ~90 days for mainnet
}

fn default_rotation_margin() -> u64 {
    5 // Rotate 5 periods before expiration
}

fn default_alert_margin() -> u64 {
    10 // Alert 10 periods before expiration
}

fn default_forging_delay() -> u64 {
    100 // 100ms delay to gather transactions
}

fn default_max_txs_per_block() -> usize {
    10000 // Reasonable default
}

fn default_max_block_size() -> usize {
    90_112 // Maximum block size in bytes (current protocol)
}

fn default_prefer_high_fees() -> bool {
    true
}

fn default_include_txs() -> bool {
    true
}

fn default_continue_on_tx_validation_failure() -> bool {
    true
}

fn default_schedule_lookahead_epochs() -> u64 {
    2 // Calculate 2 epochs ahead
}

fn default_schedule_refresh_interval() -> u64 {
    2160 // Refresh every 2160 slots (~1 hour)
}

impl Default for BlockProducerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            pool_id: None,
            vrf_key: VrfKeyConfig {
                signing_key_file: PathBuf::from("keys/vrf.skey"),
                verification_key_file: Some(PathBuf::from("keys/vrf.vkey")),
                format: KeyFormat::CardanoCli,
            },
            kes_key: KesKeyConfig {
                signing_key_file: PathBuf::from("keys/kes.skey"),
                verification_key_file: Some(PathBuf::from("keys/kes.vkey")),
                kes_period: 0,
                max_kes_evolutions: default_max_kes_evolutions(),
                start_kes_period: 0,
                format: KeyFormat::CardanoCli,
                auto_rotation: None,
            },
            operational_cert: OperationalCertConfig {
                cert_file: PathBuf::from("keys/node.cert"),
                issue_counter: 0,
                counter_file: Some(PathBuf::from("keys/node.counter")),
            },
            cold_key: None,
            forging_behavior: ForgingBehavior {
                forging_delay_ms: default_forging_delay(),
                max_txs_per_block: default_max_txs_per_block(),
                max_block_size_bytes: default_max_block_size(),
                prefer_high_fees: default_prefer_high_fees(),
                include_txs: default_include_txs(),
                continue_on_tx_validation_failure: default_continue_on_tx_validation_failure(),
            },
            leader_schedule: LeaderScheduleConfig {
                schedule_lookahead_epochs: default_schedule_lookahead_epochs(),
                log_schedule: false,
                export_schedule_file: None,
                schedule_refresh_interval: default_schedule_refresh_interval(),
            },
        }
    }
}

impl BlockProducerConfig {
    /// Validate the block producer configuration
    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        // Check VRF key file exists
        if !self.vrf_key.signing_key_file.exists() {
            return Err(format!(
                "VRF signing key file not found: {:?}",
                self.vrf_key.signing_key_file
            ));
        }

        // Check KES key file exists
        if !self.kes_key.signing_key_file.exists() {
            return Err(format!(
                "KES signing key file not found: {:?}",
                self.kes_key.signing_key_file
            ));
        }

        // Check operational certificate exists
        if !self.operational_cert.cert_file.exists() {
            return Err(format!(
                "Operational certificate file not found: {:?}",
                self.operational_cert.cert_file
            ));
        }

        // Check KES period validity
        if self.kes_key.kes_period < self.kes_key.start_kes_period {
            return Err(format!(
                "KES period {} is before start period {}",
                self.kes_key.kes_period, self.kes_key.start_kes_period
            ));
        }

        let evolutions = self.kes_key.kes_period - self.kes_key.start_kes_period;
        if evolutions >= self.kes_key.max_kes_evolutions {
            return Err(format!(
                "KES key has expired: evolution {} >= max {}",
                evolutions, self.kes_key.max_kes_evolutions
            ));
        }

        // Warn if KES key is close to expiration
        let remaining = self.kes_key.max_kes_evolutions - evolutions;
        if remaining < 10 {
            eprintln!(
                "WARNING: KES key will expire in {} periods. Consider rotating!",
                remaining
            );
        }

        Ok(())
    }

    /// Check if the node is configured as a block producer
    pub fn is_block_producer(&self) -> bool {
        self.enabled
    }

    /// Get the current KES evolution number
    pub fn current_kes_evolution(&self) -> u64 {
        self.kes_key
            .kes_period
            .saturating_sub(self.kes_key.start_kes_period)
    }

    /// Check if KES key needs rotation
    pub fn needs_kes_rotation(&self) -> bool {
        if let Some(auto_rotation) = &self.kes_key.auto_rotation {
            if !auto_rotation.enabled {
                return false;
            }
            let remaining = self
                .kes_key
                .max_kes_evolutions
                .saturating_sub(self.current_kes_evolution());
            remaining <= auto_rotation.rotation_margin_periods
        } else {
            false
        }
    }

    /// Check if we should alert about KES expiration
    pub fn should_alert_kes_expiration(&self) -> bool {
        if let Some(auto_rotation) = &self.kes_key.auto_rotation {
            let remaining = self
                .kes_key
                .max_kes_evolutions
                .saturating_sub(self.current_kes_evolution());
            remaining <= auto_rotation.alert_margin_periods
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = BlockProducerConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.vrf_key.format, KeyFormat::CardanoCli);
        assert_eq!(config.kes_key.max_kes_evolutions, 62);
    }

    #[test]
    fn test_kes_evolution_calculation() {
        let config = BlockProducerConfig {
            enabled: true,
            kes_key: KesKeyConfig {
                signing_key_file: PathBuf::from("test.skey"),
                verification_key_file: None,
                kes_period: 100,
                max_kes_evolutions: 62,
                start_kes_period: 50,
                format: KeyFormat::CardanoCli,
                auto_rotation: None,
            },
            ..Default::default()
        };

        assert_eq!(config.current_kes_evolution(), 50);
    }

    #[test]
    fn test_kes_rotation_check() {
        let config = BlockProducerConfig {
            enabled: true,
            kes_key: KesKeyConfig {
                signing_key_file: PathBuf::from("test.skey"),
                verification_key_file: None,
                kes_period: 100,
                max_kes_evolutions: 62,
                start_kes_period: 50,
                format: KeyFormat::CardanoCli,
                auto_rotation: Some(KesAutoRotation {
                    enabled: true,
                    rotation_margin_periods: 15,
                    rotation_dir: PathBuf::from("rotated_keys"),
                    alert_margin_periods: 20,
                }),
            },
            ..Default::default()
        };

        // Remaining: 62 - (100 - 50) = 12
        // Margin: 15
        // Should need rotation: 12 <= 15
        assert!(config.needs_kes_rotation());
        assert!(config.should_alert_kes_expiration());
    }
}
