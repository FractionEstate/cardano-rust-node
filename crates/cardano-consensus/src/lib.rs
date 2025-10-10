//! Cardano Ouroboros Consensus Protocol
//!
//! This crate implements the Ouroboros family of consensus protocols used in Cardano.
//! It provides:
//! - Chain selection rules (longest valid chain)
//! - Block production and validation
//! - Slot leadership calculation
//! - Fork choice decisions
//! - Multi-era protocol support
//!
//! The implementation must maintain complete compatibility with the Haskell
//! consensus layer to ensure network consensus safety.

pub mod block_broadcaster;
pub mod block_forging;
pub mod block_production;
pub mod block_production_integration;
pub mod block_production_service;
pub mod chain_selection;
pub mod epoch_transition;
pub mod leadership;
pub mod ledger_state;
pub mod metrics;
pub mod ouroboros;
pub mod slot_notifier;
pub mod slots;
pub mod validation;

pub use block_broadcaster::{
    BlockBroadcaster, BlockBroadcasterConfig, BroadcastEvent, BroadcastStats, PeerConnection,
};
pub use block_forging::{BlockForger, ForgingConfig};
pub use block_production::{
    BlockBody, BlockProducer, ForgedBlock, ForgingContext, KesKey,
    OperationalCertificate as BlockProductionOperationalCertificate, ProducedBlock,
    ProductionScheduler, SimplifiedLedgerState, Transaction, TxInput, TxOutput, VrfKey,
};
pub use block_production_integration::{AutoRefreshIntegrator, BlockProductionIntegrator};
pub use block_production_service::{
    BlockProductionConfig, BlockProductionEvent, BlockProductionService, BlockProductionStats,
};
pub use chain_selection::{
    BlockSummary, ChainCandidate, ChainOrdering, ChainSelectionConfig, ChainSelector, ChainTip,
    SelectionChainQuality, VrfTiebreakerFlavor,
};
pub use epoch_transition::{
    EpochRewards, EpochTransitionHandler, PoolRewardDistribution, StakeSnapshot,
};
pub use leadership::{
    min_stake_for_expected_blocks, vrf_output_to_probability, LeadershipCalculator,
    LeadershipCheck, LeadershipProof,
};
pub use ledger_state::{LedgerState, ProtocolParameters as LedgerProtocolParameters, UtxoEntry};
pub use metrics::{
    BroadcastMetrics, LedgerMetrics, MetricsAggregator, MetricsSummary, PerformanceMetrics,
    PipelineMetrics, ProductionMetrics, PrometheusExporter, SlotMetrics,
};
pub use ouroboros::{
    BlockNo, ChainDensityCalculator, ChainQuality, EpochNo, EpochTransition, KesManager, KesVkey,
    OperationalCertificate, OuroborosState, PoolId, ProtocolParameters, SlotLeadershipCalculator,
    SlotLeadershipTest, SlotNo, StakeDistribution, StakePool, VrfVkey,
};
pub use slot_notifier::{SlotEvent, SlotNotifier, SlotNotifierConfig, SlotNotifierStats};
pub use slots::*;
pub use validation::*;

use std::fmt;

/// Result type for consensus operations
pub type Result<T> = std::result::Result<T, ConsensusError>;

/// Consensus-related errors
#[derive(Debug, Clone)]
pub enum ConsensusError {
    InvalidBlock(String),
    ValidationFailed(String),
    ChainSelectionError(String),
    BlockProductionError(String),
    InvalidKesSignature(String),
    ExpiredKesKey(String),
    InvalidSlotProgression(String),
    InvalidPoolParameters(String),
    TimeCalculationError(String),
    SlotError(String),
    KesKeyExpired(String),
    InvalidKesEvolution(String),
    InvalidKesKey(String),
    NotSlotLeader(String),
    InvalidTransaction(String),
    InvalidSlot(String),
    InvalidProtocolMagic(String),
    InvalidOperationalCert(String),
    InvalidVrfProof(String),
    PoolNotFound(String),
    InvalidInput(String),
    InvalidScript(String),
    InvalidSignature(String),
    InvalidStake(String),
    StorageError(String),
    MissingStakeSnapshot(String),
}

impl fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConsensusError::InvalidBlock(msg) => write!(f, "Invalid block: {}", msg),
            ConsensusError::ValidationFailed(msg) => write!(f, "Validation failed: {}", msg),
            ConsensusError::ChainSelectionError(msg) => write!(f, "Chain selection error: {}", msg),
            ConsensusError::BlockProductionError(msg) => {
                write!(f, "Block production error: {}", msg)
            }
            ConsensusError::InvalidKesSignature(msg) => write!(f, "Invalid KES signature: {}", msg),
            ConsensusError::ExpiredKesKey(msg) => write!(f, "Expired KES key: {}", msg),
            ConsensusError::InvalidSlotProgression(msg) => {
                write!(f, "Invalid slot progression: {}", msg)
            }
            ConsensusError::InvalidPoolParameters(msg) => {
                write!(f, "Invalid pool parameters: {}", msg)
            }
            ConsensusError::TimeCalculationError(msg) => {
                write!(f, "Time calculation error: {}", msg)
            }
            ConsensusError::SlotError(msg) => write!(f, "Slot error: {}", msg),
            ConsensusError::KesKeyExpired(msg) => write!(f, "KES key expired: {}", msg),
            ConsensusError::InvalidKesEvolution(msg) => write!(f, "Invalid KES evolution: {}", msg),
            ConsensusError::InvalidKesKey(msg) => write!(f, "Invalid KES key: {}", msg),
            ConsensusError::NotSlotLeader(msg) => write!(f, "Not slot leader: {}", msg),
            ConsensusError::InvalidTransaction(msg) => write!(f, "Invalid transaction: {}", msg),
            ConsensusError::InvalidSlot(msg) => write!(f, "Invalid slot: {}", msg),
            ConsensusError::InvalidProtocolMagic(msg) => {
                write!(f, "Invalid protocol magic: {}", msg)
            }
            ConsensusError::InvalidOperationalCert(msg) => {
                write!(f, "Invalid operational certificate: {}", msg)
            }
            ConsensusError::InvalidVrfProof(msg) => write!(f, "Invalid VRF proof: {}", msg),
            ConsensusError::PoolNotFound(msg) => write!(f, "Pool not found: {}", msg),
            ConsensusError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            ConsensusError::InvalidScript(msg) => write!(f, "Invalid script: {}", msg),
            ConsensusError::InvalidSignature(msg) => write!(f, "Invalid signature: {}", msg),
            ConsensusError::InvalidStake(msg) => write!(f, "Invalid stake: {}", msg),
            ConsensusError::StorageError(msg) => write!(f, "Storage error: {}", msg),
            ConsensusError::MissingStakeSnapshot(msg) => {
                write!(f, "Missing stake snapshot: {}", msg)
            }
        }
    }
}

// R2 Roadmap: Forging context integration tests
#[cfg(test)]
mod tests;
