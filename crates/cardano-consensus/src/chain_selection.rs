//! Ouroboros chain selection rules
//!
//! Implements the Praos chain selection protocol with VRF tiebreakers,
//! chain quality checks, and fork resolution mechanisms.

use crate::ouroboros::{BlockNo, SlotNo};
use crate::{ConsensusError, Result};
use cardano_crypto::{Blake2b256Hash, Ed25519KeyHash, VrfOutput};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// VRF tiebreaker configuration
#[derive(Debug, Clone)]
pub enum VrfTiebreakerFlavor {
    /// Unrestricted VRF comparison for all chains
    Unrestricted,
    /// Only compare VRFs if chains are within max distance
    Restricted { max_distance: SlotNo },
}

/// Chain selection configuration
#[derive(Debug, Clone)]
pub struct ChainSelectionConfig {
    pub vrf_tiebreaker: VrfTiebreakerFlavor,
    pub security_parameter: u64,
    pub active_slot_coefficient: f64,
}

impl Default for ChainSelectionConfig {
    fn default() -> Self {
        Self {
            vrf_tiebreaker: VrfTiebreakerFlavor::Unrestricted,
            security_parameter: 2160,      // k parameter
            active_slot_coefficient: 0.05, // f parameter
        }
    }
}

/// Chain selection state manager
pub struct ChainSelector {
    config: ChainSelectionConfig,
    current_tip: Option<ChainTip>,
    candidate_tips: Vec<ChainCandidate>,
}

/// Chain tip information
#[derive(Debug, Clone)]
pub struct ChainTip {
    pub block_hash: Blake2b256Hash,
    pub slot_no: SlotNo,
    pub block_no: BlockNo,
    pub issuer: Ed25519KeyHash,
    pub vrf_output: VrfOutput,
}

/// A candidate chain for selection
#[derive(Debug, Clone)]
pub struct ChainCandidate {
    pub tip: ChainTip,
    pub length: u64,
    pub quality: SelectionChainQuality,
    pub blocks: Vec<BlockSummary>, // Recent blocks for quality analysis
}

/// Block summary for chain quality calculations
#[derive(Debug, Clone)]
pub struct BlockSummary {
    pub slot: SlotNo,
    pub block_hash: Blake2b256Hash,
    pub issuer: Ed25519KeyHash,
    pub vrf_output: VrfOutput,
    pub timestamp: SystemTime,
}

/// Chain quality metrics for selection
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionChainQuality {
    pub density: f64,
    pub block_count: u64,
    pub slot_range: u64,
    pub average_block_time: Duration,
}

/// Chain comparison result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainOrdering {
    Shorter,
    Equal,
    Longer,
    PreferCandidate,
    PreferCurrent,
}

impl ChainSelector {
    /// Create new chain selector with configuration
    pub fn new(config: ChainSelectionConfig) -> Self {
        Self {
            config,
            current_tip: None,
            candidate_tips: Vec::new(),
        }
    }

    /// Set the current chain tip
    pub fn set_current_tip(&mut self, tip: ChainTip) {
        self.current_tip = Some(tip);
    }

    /// Add a candidate chain for selection
    pub fn add_candidate(&mut self, candidate: ChainCandidate) {
        self.candidate_tips.push(candidate);
    }

    /// Select the preferred chain from all candidates
    pub fn select_chain(&mut self) -> Result<Option<ChainCandidate>> {
        if self.candidate_tips.is_empty() {
            return Ok(None);
        }

        // Create a separate config for comparison to avoid borrowing issues
        let config = self.config.clone();

        // Sort candidates by preference
        self.candidate_tips
            .sort_by(|a, b| Self::compare_chains_with_config(&config, a, b));

        // Return the best candidate
        let best_candidate = self.candidate_tips.last().cloned();

        // Update current tip if we selected a new chain
        if let Some(ref candidate) = best_candidate {
            if let Some(ref current) = self.current_tip {
                // Compare block hashes by bytes since VrfOutput doesn't implement PartialEq
                if candidate.tip.block_hash.as_bytes() != current.block_hash.as_bytes() {
                    self.current_tip = Some(candidate.tip.clone());
                }
            } else {
                self.current_tip = Some(candidate.tip.clone());
            }
        }

        Ok(best_candidate)
    }

    /// Compare two chains according to Ouroboros selection rules
    pub fn compare_chains(
        &self,
        chain_a: &ChainCandidate,
        chain_b: &ChainCandidate,
    ) -> std::cmp::Ordering {
        Self::compare_chains_with_config(&self.config, chain_a, chain_b)
    }

    /// Determine if VRF tiebreaker should be used for the given tips
    pub fn should_use_vrf_tiebreaker(&self, tip_a: &ChainTip, tip_b: &ChainTip) -> bool {
        Self::should_use_vrf_tiebreaker_with_config(&self.config, tip_a, tip_b)
    }

    /// Static version of chain comparison to avoid borrowing issues
    fn compare_chains_with_config(
        config: &ChainSelectionConfig,
        chain_a: &ChainCandidate,
        chain_b: &ChainCandidate,
    ) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        // Rule 1: Prefer longer chain
        match chain_a.length.cmp(&chain_b.length) {
            Ordering::Less => return Ordering::Less,
            Ordering::Greater => return Ordering::Greater,
            Ordering::Equal => {} // Continue to tiebreakers
        }

        // Rule 2: If equal length, check VRF tiebreaker
        if Self::should_use_vrf_tiebreaker_with_config(config, &chain_a.tip, &chain_b.tip) {
            return Self::compare_vrf_outputs(&chain_a.tip, &chain_b.tip);
        }

        // Rule 3: If VRF comparison not applicable, prefer higher block number
        match chain_a.tip.block_no.cmp(&chain_b.tip.block_no) {
            Ordering::Less => return Ordering::Less,
            Ordering::Greater => return Ordering::Greater,
            Ordering::Equal => {} // Continue to final tiebreaker
        }

        // Rule 4: Final tiebreaker by hash (deterministic)
        chain_a
            .tip
            .block_hash
            .as_bytes()
            .cmp(chain_b.tip.block_hash.as_bytes())
    }

    /// Static version of VRF tiebreaker check
    fn should_use_vrf_tiebreaker_with_config(
        config: &ChainSelectionConfig,
        tip_a: &ChainTip,
        tip_b: &ChainTip,
    ) -> bool {
        match &config.vrf_tiebreaker {
            VrfTiebreakerFlavor::Unrestricted => true,
            VrfTiebreakerFlavor::Restricted { max_distance } => {
                let slot_distance = Self::calculate_slot_distance(tip_a.slot_no, tip_b.slot_no);
                slot_distance <= *max_distance
            }
        }
    }

    /// Calculate distance between two slots
    fn calculate_slot_distance(slot_a: SlotNo, slot_b: SlotNo) -> SlotNo {
        if slot_a.0 >= slot_b.0 {
            SlotNo(slot_a.0 - slot_b.0)
        } else {
            SlotNo(slot_b.0 - slot_a.0)
        }
    }

    /// Compare VRF outputs for tiebreaking
    fn compare_vrf_outputs(tip_a: &ChainTip, tip_b: &ChainTip) -> std::cmp::Ordering {
        // Convert VRF outputs to comparable values
        let vrf_a_bytes = tip_a.vrf_output.to_bytes();
        let vrf_b_bytes = tip_b.vrf_output.to_bytes();

        // Compare VRF output bytes lexicographically
        vrf_a_bytes.cmp(vrf_b_bytes)
    }

    /// Calculate chain quality metrics
    pub fn calculate_chain_quality(
        &self,
        candidate: &ChainCandidate,
    ) -> Result<SelectionChainQuality> {
        if candidate.blocks.is_empty() {
            return Ok(SelectionChainQuality {
                density: 0.0,
                block_count: 0,
                slot_range: 0,
                average_block_time: Duration::from_secs(0),
            });
        }

        let first_slot = candidate.blocks.first().unwrap().slot.0;
        let last_slot = candidate.blocks.last().unwrap().slot.0;
        let slot_range = if last_slot >= first_slot {
            last_slot - first_slot + 1
        } else {
            1
        };

        let block_count = candidate.blocks.len() as u64;

        // Calculate density: actual blocks vs expected blocks
        let expected_blocks = slot_range as f64 * self.config.active_slot_coefficient;
        let density = if expected_blocks > 0.0 {
            block_count as f64 / expected_blocks
        } else {
            0.0
        };

        // Calculate average block time
        let mut total_time = Duration::from_secs(0);
        let mut time_count = 0;

        for window in candidate.blocks.windows(2) {
            if let Ok(duration) = window[1].timestamp.duration_since(window[0].timestamp) {
                total_time += duration;
                time_count += 1;
            }
        }

        let average_block_time = if time_count > 0 {
            total_time / time_count as u32
        } else {
            Duration::from_secs(20) // Default slot length
        };

        Ok(SelectionChainQuality {
            density,
            block_count,
            slot_range,
            average_block_time,
        })
    }

    /// Check if a chain meets quality thresholds
    pub fn is_chain_quality_acceptable(&self, quality: &SelectionChainQuality) -> bool {
        // Minimum density threshold (e.g., 50% of expected)
        let min_density = 0.5;

        // Ensure chain has reasonable density
        quality.density >= min_density
    }

    /// Find the best fork point between chains
    pub fn find_fork_point(
        &self,
        chain_a: &ChainCandidate,
        chain_b: &ChainCandidate,
    ) -> Result<Option<SlotNo>> {
        let mut fork_slot = None;

        // Build hash maps for efficient lookup
        let chain_a_hashes: HashMap<SlotNo, Blake2b256Hash> = chain_a
            .blocks
            .iter()
            .map(|b| (b.slot, b.block_hash))
            .collect();

        let chain_b_hashes: HashMap<SlotNo, Blake2b256Hash> = chain_b
            .blocks
            .iter()
            .map(|b| (b.slot, b.block_hash))
            .collect();

        // Find the last common slot by iterating in reverse order
        let mut all_slots: Vec<_> = chain_a_hashes.keys().cloned().collect();
        all_slots.sort_by(|a, b| b.cmp(a)); // Sort in descending order for last-first search

        for slot_a in all_slots {
            if let (Some(hash_a), Some(hash_b)) =
                (chain_a_hashes.get(&slot_a), chain_b_hashes.get(&slot_a))
            {
                if hash_a == hash_b {
                    fork_slot = Some(slot_a);
                    break; // Found the last (highest slot) common block
                }
            }
        }

        Ok(fork_slot)
    }

    /// Validate chain selection parameters
    pub fn validate_selection_parameters(&self) -> Result<()> {
        if self.config.security_parameter == 0 {
            return Err(ConsensusError::InvalidPoolParameters(
                "Security parameter cannot be zero".to_string(),
            ));
        }

        if self.config.active_slot_coefficient <= 0.0 || self.config.active_slot_coefficient > 1.0 {
            return Err(ConsensusError::InvalidPoolParameters(
                "Active slot coefficient must be between 0 and 1".to_string(),
            ));
        }

        Ok(())
    }

    /// Get current chain statistics
    pub fn get_chain_statistics(&self) -> HashMap<String, u64> {
        let mut stats = HashMap::new();

        stats.insert(
            "candidate_count".to_string(),
            self.candidate_tips.len() as u64,
        );
        stats.insert(
            "security_parameter".to_string(),
            self.config.security_parameter,
        );

        if let Some(ref tip) = self.current_tip {
            stats.insert("current_slot".to_string(), tip.slot_no.0);
            stats.insert("current_block".to_string(), tip.block_no.0);
        }

        stats
    }

    /// Clear candidate tips (e.g., after selection)
    pub fn clear_candidates(&mut self) {
        self.candidate_tips.clear();
    }
}

impl Default for ChainSelector {
    fn default() -> Self {
        Self::new(ChainSelectionConfig::default())
    }
}

/// Utility functions for chain selection
impl ChainCandidate {
    /// Create a new chain candidate
    pub fn new(tip: ChainTip, length: u64, blocks: Vec<BlockSummary>) -> Self {
        let quality = SelectionChainQuality {
            density: 0.0, // Will be calculated separately
            block_count: blocks.len() as u64,
            slot_range: 0, // Will be calculated separately
            average_block_time: Duration::from_secs(20),
        };

        Self {
            tip,
            length,
            quality,
            blocks,
        }
    }

    /// Check if this chain is longer than another
    pub fn is_longer_than(&self, other: &ChainCandidate) -> bool {
        self.length > other.length
    }

    /// Get the chain's effective length considering quality
    pub fn effective_length(&self) -> f64 {
        // Weight length by chain quality
        self.length as f64 * self.quality.density.max(0.1) // Minimum weight of 0.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tip(slot: u64, block_no: u64, hash_seed: &str) -> ChainTip {
        ChainTip {
            block_hash: Blake2b256Hash::hash(hash_seed.as_bytes()),
            slot_no: SlotNo(slot),
            block_no: BlockNo(block_no),
            issuer: Ed25519KeyHash::from_test_data(b"test_issuer"),
            vrf_output: VrfOutput::from_bytes([0u8; 64]).unwrap(),
        }
    }

    fn create_test_candidate(length: u64, tip_slot: u64, hash_seed: &str) -> ChainCandidate {
        let tip = create_test_tip(tip_slot, length, hash_seed);
        let blocks = vec![BlockSummary {
            slot: SlotNo(tip_slot),
            block_hash: tip.block_hash,
            issuer: tip.issuer,
            vrf_output: tip.vrf_output.clone(),
            timestamp: SystemTime::now(),
        }];

        ChainCandidate::new(tip, length, blocks)
    }

    #[test]
    fn test_chain_selection_basic() {
        let mut selector = ChainSelector::default();

        let candidate_a = create_test_candidate(10, 100, "chain_a");
        let candidate_b = create_test_candidate(15, 110, "chain_b");

        selector.add_candidate(candidate_a);
        selector.add_candidate(candidate_b);

        let selected = selector.select_chain().unwrap();
        assert!(selected.is_some());

        let selected_chain = selected.unwrap();
        assert_eq!(selected_chain.length, 15); // Should select longer chain
    }

    #[test]
    fn test_chain_comparison_length() {
        let selector = ChainSelector::default();

        let short_chain = create_test_candidate(5, 50, "short");
        let long_chain = create_test_candidate(10, 100, "long");

        let comparison = selector.compare_chains(&short_chain, &long_chain);
        assert_eq!(comparison, std::cmp::Ordering::Less);
    }

    #[test]
    fn test_slot_distance_calculation() {
        let distance1 = ChainSelector::calculate_slot_distance(SlotNo(100), SlotNo(90));
        assert_eq!(distance1, SlotNo(10));

        let distance2 = ChainSelector::calculate_slot_distance(SlotNo(50), SlotNo(75));
        assert_eq!(distance2, SlotNo(25));
    }

    #[test]
    fn test_vrf_tiebreaker_unrestricted() {
        let config = ChainSelectionConfig {
            vrf_tiebreaker: VrfTiebreakerFlavor::Unrestricted,
            ..Default::default()
        };
        let selector = ChainSelector::new(config);

        let tip_a = create_test_tip(100, 10, "tip_a");
        let tip_b = create_test_tip(200, 10, "tip_b");

        assert!(selector.should_use_vrf_tiebreaker(&tip_a, &tip_b));
    }

    #[test]
    fn test_vrf_tiebreaker_restricted() {
        let config = ChainSelectionConfig {
            vrf_tiebreaker: VrfTiebreakerFlavor::Restricted {
                max_distance: SlotNo(50),
            },
            ..Default::default()
        };
        let selector = ChainSelector::new(config);

        let tip_close = create_test_tip(100, 10, "tip_close");
        let tip_far = create_test_tip(200, 10, "tip_far");
        let tip_near = create_test_tip(130, 10, "tip_near");

        // Should use VRF for close tips
        assert!(selector.should_use_vrf_tiebreaker(&tip_close, &tip_near));

        // Should not use VRF for distant tips
        assert!(!selector.should_use_vrf_tiebreaker(&tip_close, &tip_far));
    }

    #[test]
    fn test_chain_quality_calculation() {
        let selector = ChainSelector::default();
        let candidate = create_test_candidate(5, 100, "test_quality");

        let quality = selector.calculate_chain_quality(&candidate).unwrap();
        assert_eq!(quality.block_count, 1);
        assert!(quality.density >= 0.0);
    }

    #[test]
    fn test_selection_parameters_validation() {
        let config = ChainSelectionConfig {
            security_parameter: 0,
            ..Default::default()
        };
        let selector = ChainSelector::new(config);

        let result = selector.validate_selection_parameters();
        assert!(result.is_err());
    }

    #[test]
    fn test_fork_point_detection() {
        let selector = ChainSelector::default();

        // Create chains with some common history
        let mut blocks_a = Vec::new();
        let mut blocks_b = Vec::new();

        // Common blocks
        for i in 1..=5 {
            let block = BlockSummary {
                slot: SlotNo(i),
                block_hash: Blake2b256Hash::hash(format!("common_{}", i).as_bytes()),
                issuer: Ed25519KeyHash::from_test_data(b"issuer"),
                vrf_output: VrfOutput::from_bytes([0u8; 64]).unwrap(),
                timestamp: SystemTime::now(),
            };
            blocks_a.push(block.clone());
            blocks_b.push(block);
        }

        // Divergent blocks
        blocks_a.push(BlockSummary {
            slot: SlotNo(6),
            block_hash: Blake2b256Hash::hash(b"diverge_a"),
            issuer: Ed25519KeyHash::from_test_data(b"issuer"),
            vrf_output: VrfOutput::from_bytes([1u8; 64]).unwrap(),
            timestamp: SystemTime::now(),
        });

        blocks_b.push(BlockSummary {
            slot: SlotNo(6),
            block_hash: Blake2b256Hash::hash(b"diverge_b"),
            issuer: Ed25519KeyHash::from_test_data(b"issuer"),
            vrf_output: VrfOutput::from_bytes([2u8; 64]).unwrap(),
            timestamp: SystemTime::now(),
        });

        let candidate_a = ChainCandidate::new(create_test_tip(6, 6, "chain_a"), 6, blocks_a);

        let candidate_b = ChainCandidate::new(create_test_tip(6, 6, "chain_b"), 6, blocks_b);

        let fork_point = selector
            .find_fork_point(&candidate_a, &candidate_b)
            .unwrap();
        assert_eq!(fork_point, Some(SlotNo(5))); // Last common slot
    }
}
