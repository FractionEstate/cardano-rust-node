//! Conway Era Ledger Rules
//!
//! The Conway era introduces on-chain governance to Cardano, implementing CIP-1694.
//! Building on Babbage, it adds comprehensive governance features:
//! - Governance actions (parameter changes, hard forks, treasury withdrawals)
//! - Voting mechanisms for DReps, SPOs, and Constitutional Committee
//! - Constitutional Committee with hot/cold keys
//! - Delegated Representatives (DReps) system
//! - Treasury management and governance-controlled parameter updates
//! - Info actions for governance information dissemination

use crate::{LedgerError, Result};
use crate::mary::{Coin, Slot, Address, RewardAddress, Certificate, ValidityInterval, Ed25519KeyHash};
use crate::babbage::{
    BabbageTransaction, BabbageTransactionOutput, OutputDatum, ScriptReference,
    BabbageWitnessSet, BabbageRedeemer
};
use cardano_crypto::{Blake2b256Hash, Ed25519Signature, VrfProof};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Conway Era Transaction with governance support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConwayTransaction {
    /// Standard Babbage transaction fields
    pub babbage_tx: BabbageTransaction,

    /// Conway-specific governance fields
    pub voting_procedures: Vec<VotingProcedure>,
    pub proposal_procedures: Vec<ProposalProcedure>,
    pub current_treasury_value: Option<Coin>,
    pub treasury_donation: Option<Coin>,
}

/// Governance action types defined in CIP-1694
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GovernanceActionType {
    /// Parameter changes for protocol parameters
    ParameterChange,
    /// Hard fork initiation
    HardForkInitiation,
    /// Treasury withdrawal to specified addresses
    TreasuryWithdrawals,
    /// No confidence motion against Constitutional Committee
    NoConfidence,
    /// Update Constitutional Committee members
    UpdateCommittee,
    /// New constitution hash
    NewConstitution,
    /// Informational action (no on-chain effect)
    InfoAction,
}

/// Governance action with all necessary data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceAction {
    pub action_type: GovernanceActionType,
    pub action_id: GovernanceActionId,
    pub parameter_changes: Option<ProtocolParameterUpdate>,
    pub hard_fork_info: Option<HardForkInfo>,
    pub treasury_withdrawals: Option<HashMap<RewardAddress, Coin>>,
    pub committee_update: Option<CommitteeUpdate>,
    pub constitution_update: Option<ConstitutionUpdate>,
    pub info_data: Option<AnchorData>,
    pub guardrails_policy: Option<Blake2b256Hash>, // Script hash for guardrails
}

/// Unique identifier for governance actions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct GovernanceActionId {
    pub transaction_id: Blake2b256Hash,
    pub governance_action_index: u32,
}

/// Protocol parameter updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolParameterUpdate {
    pub min_fee_a: Option<u64>,
    pub min_fee_b: Option<u64>,
    pub max_block_body_size: Option<u64>,
    pub max_transaction_size: Option<u64>,
    pub max_block_header_size: Option<u64>,
    pub key_deposit: Option<Coin>,
    pub pool_deposit: Option<Coin>,
    pub min_pool_cost: Option<Coin>,
    pub price_memory: Option<f64>,
    pub price_steps: Option<f64>,
    pub max_tx_execution_units: Option<ExUnits>,
    pub max_block_execution_units: Option<ExUnits>,
    pub max_value_size: Option<u64>,
    pub collateral_percentage: Option<u64>,
    pub max_collateral_inputs: Option<u64>,
    pub cost_model_plutus_v1: Option<Vec<i64>>,
    pub cost_model_plutus_v2: Option<Vec<i64>>,
    pub cost_model_plutus_v3: Option<Vec<i64>>, // Future Plutus version
    pub governance_voting_thresholds: Option<VotingThresholds>,
    pub governance_action_lifetime: Option<u64>, // In epochs
    pub governance_action_deposit: Option<Coin>,
    pub drep_deposit: Option<Coin>,
    pub drep_activity: Option<u64>, // In epochs
    pub min_committee_size: Option<u64>,
    pub committee_max_term_length: Option<u64>, // In epochs
}

/// Hard fork information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardForkInfo {
    pub major_version: u32,
    pub minor_version: u32,
}

/// Constitutional Committee update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitteeUpdate {
    pub members_to_remove: Vec<Ed25519KeyHash>, // Cold keys to remove
    pub members_to_add: HashMap<Ed25519KeyHash, Slot>, // Cold key -> expiry slot
    pub threshold: Option<Rational>, // New threshold (numerator/denominator)
}

/// Rational number for thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rational {
    pub numerator: u64,
    pub denominator: u64,
}

/// Constitution update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstitutionUpdate {
    pub constitution_hash: Blake2b256Hash,
    pub guardrails_script_hash: Option<Blake2b256Hash>,
}

/// Anchor data for off-chain information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorData {
    pub url: String,
    pub data_hash: Blake2b256Hash,
}

/// Voting procedure for governance actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingProcedure {
    pub governance_action_id: GovernanceActionId,
    pub voter: Voter,
    pub vote: Vote,
    pub anchor: Option<AnchorData>,
}

/// Types of voters in Conway governance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Voter {
    /// Constitutional Committee member (cold key)
    ConstitutionalCommittee(Ed25519KeyHash),
    /// Delegated Representative
    DRep(DRepId),
    /// Stake Pool Operator
    StakePool(Ed25519KeyHash), // Pool ID
}

/// Delegated Representative identifier
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DRepId {
    /// Key hash of DRep
    KeyHash(Ed25519KeyHash),
    /// Script hash of DRep
    ScriptHash(Blake2b256Hash),
    /// Special DRep: Abstain
    Abstain,
    /// Special DRep: No Confidence
    NoConfidence,
}

/// Vote on a governance action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Vote {
    Yes,
    No,
    Abstain,
}

/// Proposal procedure for submitting governance actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalProcedure {
    pub deposit: Coin,
    pub return_address: RewardAddress,
    pub governance_action: GovernanceAction,
    pub anchor: Option<AnchorData>,
}

/// Voting thresholds for different governance actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingThresholds {
    pub motion_no_confidence: ThresholdGroup,
    pub committee_normal: ThresholdGroup,
    pub committee_no_confidence: ThresholdGroup,
    pub update_constitution: ThresholdGroup,
    pub hard_fork_initiation: ThresholdGroup,
    pub pparam_economic_group: ThresholdGroup,
    pub pparam_network_group: ThresholdGroup,
    pub pparam_technical_group: ThresholdGroup,
    pub pparam_governance_group: ThresholdGroup,
    pub treasury_withdrawal: ThresholdGroup,
}

/// Thresholds for Constitutional Committee, DReps, and SPOs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdGroup {
    pub committee_threshold: Option<Rational>,
    pub drep_threshold: Rational,
    pub spo_threshold: Rational,
}

/// Conway-specific certificates for governance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConwayCertificate {
    /// Standard certificates from previous eras
    Standard(Certificate),

    /// DRep registration
    RegDRep {
        drep_id: DRepId,
        deposit: Coin,
        anchor: Option<AnchorData>,
    },

    /// DRep unregistration
    UnregDRep {
        drep_id: DRepId,
        refund: Coin,
    },

    /// Update DRep information
    UpdateDRep {
        drep_id: DRepId,
        anchor: Option<AnchorData>,
    },

    /// Constitutional Committee hot key authorization
    AuthorizeCommitteeHot {
        cold_key: Ed25519KeyHash,
        hot_key: Ed25519KeyHash,
    },

    /// Constitutional Committee hot key resignation
    ResignCommitteeCold {
        cold_key: Ed25519KeyHash,
        anchor: Option<AnchorData>,
    },

    /// Delegate stake to DRep (replaces standard delegation)
    DelegateStake {
        stake_credential: Ed25519KeyHash,
        delegatee: DelegationTarget,
    },
}

/// Delegation targets in Conway era
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DelegationTarget {
    /// Delegate voting power to a DRep
    DRep(DRepId),
    /// Delegate stake to pool (for rewards) and DRep (for voting)
    PoolAndDRep {
        pool_id: Ed25519KeyHash,
        drep_id: DRepId,
    },
    /// Delegate to pool only (abstain from governance)
    PoolOnly(Ed25519KeyHash),
}

/// Governance state tracking
#[derive(Debug, Clone)]
pub struct GovernanceState {
    /// Active governance actions by ID
    pub active_actions: HashMap<GovernanceActionId, GovernanceAction>,
    /// Constitutional Committee members and their expiry
    pub committee_members: HashMap<Ed25519KeyHash, Slot>, // Cold key -> expiry
    /// Committee hot keys
    pub committee_hot_keys: HashMap<Ed25519KeyHash, Ed25519KeyHash>, // Cold -> hot
    /// Current committee threshold
    pub committee_threshold: Rational,
    /// Registered DReps
    pub dreps: HashMap<DRepId, DRepInfo>,
    /// Delegations from stake credentials to DReps/pools
    pub delegations: HashMap<Ed25519KeyHash, DelegationTarget>,
    /// Votes cast on governance actions
    pub votes: HashMap<GovernanceActionId, Vec<VotingProcedure>>,
    /// Current protocol parameters
    pub protocol_parameters: ProtocolParameters,
    /// Current constitution hash
    pub constitution_hash: Blake2b256Hash,
    /// Treasury amount
    pub treasury: Coin,
}

/// DRep information
#[derive(Debug, Clone)]
pub struct DRepInfo {
    pub deposit: Coin,
    pub anchor: Option<AnchorData>,
    pub voting_power: Coin, // Total delegated stake
    pub last_activity: Slot,
}

/// Protocol parameters (simplified)
#[derive(Debug, Clone)]
pub struct ProtocolParameters {
    pub min_fee_a: u64,
    pub min_fee_b: u64,
    pub max_block_body_size: u64,
    pub governance_action_lifetime: u64,
    pub governance_action_deposit: Coin,
    pub drep_deposit: Coin,
    pub drep_activity_period: u64,
    pub min_committee_size: u64,
    pub committee_max_term_length: u64,
    pub voting_thresholds: VotingThresholds,
}

// Re-export execution units from Alonzo
use crate::alonzo::ExUnits;

impl ConwayTransaction {
    /// Create new Conway transaction from Babbage transaction
    pub fn from_babbage(babbage_tx: BabbageTransaction) -> Self {
        Self {
            babbage_tx,
            voting_procedures: vec![],
            proposal_procedures: vec![],
            current_treasury_value: None,
            treasury_donation: None,
        }
    }

    /// Add voting procedure
    pub fn add_vote(mut self, vote: VotingProcedure) -> Self {
        self.voting_procedures.push(vote);
        self
    }

    /// Add governance proposal
    pub fn add_proposal(mut self, proposal: ProposalProcedure) -> Self {
        self.proposal_procedures.push(proposal);
        self
    }

    /// Add treasury donation
    pub fn with_treasury_donation(mut self, amount: Coin) -> Self {
        self.treasury_donation = Some(amount);
        self
    }
}

impl Rational {
    pub fn new(numerator: u64, denominator: u64) -> Result<Self> {
        if denominator == 0 {
            return Err(LedgerError::InvalidTransaction(
                "Denominator cannot be zero".to_string()
            ));
        }
        Ok(Self { numerator, denominator })
    }

    pub fn as_percentage(&self) -> f64 {
        (self.numerator as f64 / self.denominator as f64) * 100.0
    }

    pub fn meets_threshold(&self, votes_for: u64, total_votes: u64) -> bool {
        if total_votes == 0 {
            return false;
        }

        // Check if votes_for / total_votes >= self
        votes_for * self.denominator >= self.numerator * total_votes
    }
}

impl Vote {
    pub fn is_yes(&self) -> bool {
        matches!(self, Vote::Yes)
    }

    pub fn is_abstain(&self) -> bool {
        matches!(self, Vote::Abstain)
    }
}

impl DRepId {
    pub fn is_special(&self) -> bool {
        matches!(self, DRepId::Abstain | DRepId::NoConfidence)
    }

    pub fn abstain() -> Self {
        Self::Abstain
    }

    pub fn no_confidence() -> Self {
        Self::NoConfidence
    }
}

/// Conway era ledger validation
pub struct ConwayLedger;

impl ConwayLedger {
    /// Validate a Conway era transaction
    pub fn validate_transaction(
        tx: &ConwayTransaction,
        governance_state: &GovernanceState
    ) -> Result<()> {
        // 1. Validate underlying Babbage transaction
        // Note: Would use BabbageLedger::validate_transaction(&tx.babbage_tx)

        // 2. Validate governance-specific fields
        Self::validate_voting_procedures(tx, governance_state)?;
        Self::validate_proposal_procedures(tx, governance_state)?;
        Self::validate_treasury_operations(tx, governance_state)?;

        Ok(())
    }

    fn validate_voting_procedures(
        tx: &ConwayTransaction,
        governance_state: &GovernanceState
    ) -> Result<()> {
        for vote_proc in &tx.voting_procedures {
            // Check that governance action exists and is active
            if !governance_state.active_actions.contains_key(&vote_proc.governance_action_id) {
                return Err(LedgerError::GovernanceError(
                    format!("Governance action {:?} does not exist", vote_proc.governance_action_id)
                ));
            }

            // Validate voter authorization
            Self::validate_voter_authorization(&vote_proc.voter, governance_state)?;

            // Check for duplicate votes (one vote per voter per action)
            if let Some(existing_votes) = governance_state.votes.get(&vote_proc.governance_action_id) {
                for existing_vote in existing_votes {
                    if Self::same_voter(&vote_proc.voter, &existing_vote.voter) {
                        return Err(LedgerError::GovernanceError(
                            "Voter has already voted on this action".to_string()
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    fn validate_voter_authorization(
        voter: &Voter,
        governance_state: &GovernanceState
    ) -> Result<()> {
        match voter {
            Voter::ConstitutionalCommittee(cold_key) => {
                if !governance_state.committee_members.contains_key(cold_key) {
                    return Err(LedgerError::GovernanceError(
                        "Not a valid Constitutional Committee member".to_string()
                    ));
                }
                // Check if not expired
                let expiry = governance_state.committee_members[cold_key];
                // Would check against current slot
            }
            Voter::DRep(drep_id) => {
                match drep_id {
                    DRepId::Abstain | DRepId::NoConfidence => {} // Always valid
                    DRepId::KeyHash(_) | DRepId::ScriptHash(_) => {
                        if !governance_state.dreps.contains_key(drep_id) {
                            return Err(LedgerError::GovernanceError(
                                "DRep is not registered".to_string()
                            ));
                        }
                        // Check if DRep is active (voted or updated within activity period)
                        let drep_info = &governance_state.dreps[drep_id];
                        // Would check drep_info.last_activity against current slot
                    }
                }
            }
            Voter::StakePool(pool_id) => {
                // Would validate that pool_id is a registered stake pool
                // This requires access to pool registration certificates
            }
        }

        Ok(())
    }

    fn same_voter(voter1: &Voter, voter2: &Voter) -> bool {
        match (voter1, voter2) {
            (Voter::ConstitutionalCommittee(k1), Voter::ConstitutionalCommittee(k2)) => k1 == k2,
            (Voter::DRep(d1), Voter::DRep(d2)) => d1 == d2,
            (Voter::StakePool(p1), Voter::StakePool(p2)) => p1 == p2,
            _ => false,
        }
    }

    fn validate_proposal_procedures(
        tx: &ConwayTransaction,
        governance_state: &GovernanceState
    ) -> Result<()> {
        for proposal in &tx.proposal_procedures {
            // Validate deposit amount meets minimum
            if proposal.deposit < governance_state.protocol_parameters.governance_action_deposit {
                return Err(LedgerError::GovernanceError(
                    format!("Governance action deposit {} is below minimum {}",
                        proposal.deposit,
                        governance_state.protocol_parameters.governance_action_deposit)
                ));
            }

            // Validate governance action structure
            Self::validate_governance_action(&proposal.governance_action, governance_state)?;

            // Validate return address format
            // Would validate that return_address is a proper reward address
        }

        Ok(())
    }

    fn validate_governance_action(
        action: &GovernanceAction,
        _governance_state: &GovernanceState
    ) -> Result<()> {
        match action.action_type {
            GovernanceActionType::ParameterChange => {
                if action.parameter_changes.is_none() {
                    return Err(LedgerError::GovernanceError(
                        "Parameter change action must include parameter changes".to_string()
                    ));
                }
            }
            GovernanceActionType::HardForkInitiation => {
                if action.hard_fork_info.is_none() {
                    return Err(LedgerError::GovernanceError(
                        "Hard fork action must include version info".to_string()
                    ));
                }
            }
            GovernanceActionType::TreasuryWithdrawals => {
                if action.treasury_withdrawals.is_none() {
                    return Err(LedgerError::GovernanceError(
                        "Treasury withdrawal must specify withdrawals".to_string()
                    ));
                }
                // Validate withdrawal amounts don't exceed treasury
                if let Some(withdrawals) = &action.treasury_withdrawals {
                    let total_withdrawal: Coin = withdrawals.values().sum();
                    // Would check against governance_state.treasury
                }
            }
            GovernanceActionType::UpdateCommittee => {
                if action.committee_update.is_none() {
                    return Err(LedgerError::GovernanceError(
                        "Committee update must specify changes".to_string()
                    ));
                }
            }
            GovernanceActionType::NewConstitution => {
                if action.constitution_update.is_none() {
                    return Err(LedgerError::GovernanceError(
                        "Constitution update must specify new constitution hash".to_string()
                    ));
                }
            }
            GovernanceActionType::InfoAction => {
                // Info actions can have any content, mainly anchor data
            }
            GovernanceActionType::NoConfidence => {
                // No confidence requires no additional data
            }
        }

        Ok(())
    }

    fn validate_treasury_operations(
        tx: &ConwayTransaction,
        governance_state: &GovernanceState
    ) -> Result<()> {
        if let Some(donation) = tx.treasury_donation {
            if donation == 0 {
                return Err(LedgerError::InvalidTransaction(
                    "Treasury donation must be positive".to_string()
                ));
            }
        }

        // Validate current treasury value if provided
        if let Some(claimed_treasury) = tx.current_treasury_value {
            if claimed_treasury != governance_state.treasury {
                return Err(LedgerError::GovernanceError(
                    format!("Incorrect treasury value: claimed {}, actual {}",
                        claimed_treasury, governance_state.treasury)
                ));
            }
        }

        Ok(())
    }

    /// Check if governance action has reached required thresholds
    pub fn check_governance_thresholds(
        action: &GovernanceAction,
        votes: &[VotingProcedure],
        governance_state: &GovernanceState,
    ) -> bool {
        let thresholds = Self::get_thresholds_for_action(
            &action.action_type,
            &governance_state.protocol_parameters.voting_thresholds
        );

        // Calculate votes by voter type
        let mut committee_yes = 0u64;
        let mut committee_total = 0u64;
        let mut drep_yes_stake = 0u64;
        let mut drep_total_stake = 0u64;
        let mut spo_yes_stake = 0u64;
        let mut spo_total_stake = 0u64;

        for vote in votes {
            match &vote.voter {
                Voter::ConstitutionalCommittee(cold_key) => {
                    if governance_state.committee_members.contains_key(cold_key) {
                        committee_total += 1;
                        if vote.vote.is_yes() {
                            committee_yes += 1;
                        }
                    }
                }
                Voter::DRep(drep_id) => {
                    let voting_power = Self::get_drep_voting_power(drep_id, governance_state);
                    drep_total_stake += voting_power;
                    if vote.vote.is_yes() {
                        drep_yes_stake += voting_power;
                    }
                }
                Voter::StakePool(pool_id) => {
                    let pool_stake = Self::get_pool_stake(pool_id, governance_state);
                    spo_total_stake += pool_stake;
                    if vote.vote.is_yes() {
                        spo_yes_stake += pool_stake;
                    }
                }
            }
        }

        // Check each required threshold
        let mut requirements_met = true;

        if let Some(committee_threshold) = &thresholds.committee_threshold {
            if !committee_threshold.meets_threshold(committee_yes, committee_total) {
                requirements_met = false;
            }
        }

        if !thresholds.drep_threshold.meets_threshold(drep_yes_stake, drep_total_stake) {
            requirements_met = false;
        }

        if !thresholds.spo_threshold.meets_threshold(spo_yes_stake, spo_total_stake) {
            requirements_met = false;
        }

        requirements_met
    }

    fn get_thresholds_for_action(
        action_type: &GovernanceActionType,
        voting_thresholds: &VotingThresholds,
    ) -> &ThresholdGroup {
        match action_type {
            GovernanceActionType::NoConfidence => &voting_thresholds.motion_no_confidence,
            GovernanceActionType::UpdateCommittee => &voting_thresholds.committee_normal,
            GovernanceActionType::NewConstitution => &voting_thresholds.update_constitution,
            GovernanceActionType::HardForkInitiation => &voting_thresholds.hard_fork_initiation,
            GovernanceActionType::ParameterChange => {
                // Would need to analyze which parameter group based on changes
                &voting_thresholds.pparam_technical_group
            }
            GovernanceActionType::TreasuryWithdrawals => &voting_thresholds.treasury_withdrawal,
            GovernanceActionType::InfoAction => &voting_thresholds.pparam_technical_group, // Lowest threshold
        }
    }

    fn get_drep_voting_power(drep_id: &DRepId, governance_state: &GovernanceState) -> Coin {
        match drep_id {
            DRepId::Abstain | DRepId::NoConfidence => {
                // Calculate total stake delegated to this special DRep
                governance_state.delegations
                    .iter()
                    .filter_map(|(stake_cred, target)| {
                        match target {
                            DelegationTarget::DRep(delegated_drep) => {
                                if delegated_drep == drep_id {
                                    Some(Self::get_stake_for_credential(stake_cred, governance_state))
                                } else {
                                    None
                                }
                            }
                            DelegationTarget::PoolAndDRep { drep_id: delegated_drep, .. } => {
                                if delegated_drep == drep_id {
                                    Some(Self::get_stake_for_credential(stake_cred, governance_state))
                                } else {
                                    None
                                }
                            }
                            DelegationTarget::PoolOnly(_) => None, // No voting power
                        }
                    })
                    .sum()
            }
            _ => {
                governance_state.dreps
                    .get(drep_id)
                    .map(|info| info.voting_power)
                    .unwrap_or(0)
            }
        }
    }

    fn get_pool_stake(_pool_id: &Ed25519KeyHash, _governance_state: &GovernanceState) -> Coin {
        // Would calculate total stake delegated to this pool
        // This requires access to delegation certificates and stake distribution
        0 // Simplified
    }

    fn get_stake_for_credential(_stake_cred: &Ed25519KeyHash, _governance_state: &GovernanceState) -> Coin {
        // Would look up actual stake amount for this credential
        1_000_000 // Simplified - 1 ADA per credential
    }
}

impl Default for VotingThresholds {
    fn default() -> Self {
        // Default thresholds from CIP-1694
        Self {
            motion_no_confidence: ThresholdGroup {
                committee_threshold: None, // Committee doesn't vote on no confidence
                drep_threshold: Rational::new(51, 100).unwrap(), // 51%
                spo_threshold: Rational::new(51, 100).unwrap(), // 51%
            },
            committee_normal: ThresholdGroup {
                committee_threshold: Some(Rational::new(51, 100).unwrap()),
                drep_threshold: Rational::new(51, 100).unwrap(),
                spo_threshold: Rational::new(51, 100).unwrap(),
            },
            committee_no_confidence: ThresholdGroup {
                committee_threshold: None,
                drep_threshold: Rational::new(60, 100).unwrap(), // 60%
                spo_threshold: Rational::new(60, 100).unwrap(), // 60%
            },
            update_constitution: ThresholdGroup {
                committee_threshold: Some(Rational::new(75, 100).unwrap()), // 75%
                drep_threshold: Rational::new(75, 100).unwrap(), // 75%
                spo_threshold: Rational::new(75, 100).unwrap(), // 75%
            },
            hard_fork_initiation: ThresholdGroup {
                committee_threshold: Some(Rational::new(60, 100).unwrap()),
                drep_threshold: Rational::new(60, 100).unwrap(),
                spo_threshold: Rational::new(60, 100).unwrap(),
            },
            pparam_economic_group: ThresholdGroup {
                committee_threshold: Some(Rational::new(60, 100).unwrap()),
                drep_threshold: Rational::new(60, 100).unwrap(),
                spo_threshold: Rational::new(60, 100).unwrap(),
            },
            pparam_network_group: ThresholdGroup {
                committee_threshold: Some(Rational::new(60, 100).unwrap()),
                drep_threshold: Rational::new(60, 100).unwrap(),
                spo_threshold: Rational::new(60, 100).unwrap(),
            },
            pparam_technical_group: ThresholdGroup {
                committee_threshold: Some(Rational::new(60, 100).unwrap()),
                drep_threshold: Rational::new(60, 100).unwrap(),
                spo_threshold: Rational::new(60, 100).unwrap(),
            },
            pparam_governance_group: ThresholdGroup {
                committee_threshold: Some(Rational::new(75, 100).unwrap()),
                drep_threshold: Rational::new(75, 100).unwrap(),
                spo_threshold: Rational::new(75, 100).unwrap(),
            },
            treasury_withdrawal: ThresholdGroup {
                committee_threshold: Some(Rational::new(60, 100).unwrap()),
                drep_threshold: Rational::new(60, 100).unwrap(),
                spo_threshold: Rational::new(60, 100).unwrap(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rational_threshold() {
        let threshold = Rational::new(60, 100).unwrap(); // 60%

        assert!(threshold.meets_threshold(60, 100)); // Exactly 60%
        assert!(threshold.meets_threshold(65, 100)); // 65% > 60%
        assert!(!threshold.meets_threshold(59, 100)); // 59% < 60%
        assert!(!threshold.meets_threshold(0, 0)); // No votes
    }

    #[test]
    fn test_drep_id_special() {
        assert!(DRepId::abstain().is_special());
        assert!(DRepId::no_confidence().is_special());

        let key_drep = DRepId::KeyHash(Blake2b256Hash::hash(&[0u8; 28]));
        assert!(!key_drep.is_special());
    }

    #[test]
    fn test_vote_properties() {
        let yes_vote = Vote::Yes;
        let no_vote = Vote::No;
        let abstain_vote = Vote::Abstain;

        assert!(yes_vote.is_yes());
        assert!(!no_vote.is_yes());
        assert!(!abstain_vote.is_yes());

        assert!(!yes_vote.is_abstain());
        assert!(abstain_vote.is_abstain());
    }

    #[test]
    fn test_governance_action_creation() {
        let action_id = GovernanceActionId {
            transaction_id: Blake2b256Hash::hash(b"test_tx"),
            governance_action_index: 0,
        };

        let parameter_change = GovernanceAction {
            action_type: GovernanceActionType::ParameterChange,
            action_id,
            parameter_changes: Some(ProtocolParameterUpdate {
                min_fee_a: Some(44),
                min_fee_b: Some(155381),
                ..Default::default()
            }),
            hard_fork_info: None,
            treasury_withdrawals: None,
            committee_update: None,
            constitution_update: None,
            info_data: None,
            guardrails_policy: None,
        };

        assert_eq!(parameter_change.action_type, GovernanceActionType::ParameterChange);
        assert!(parameter_change.parameter_changes.is_some());
    }

    #[test]
    fn test_conway_transaction_builder() {
        let babbage_tx = BabbageTransaction {
            inputs: vec![],
            outputs: vec![],
            fee: 200_000,
            ttl: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            auxiliary_data: None,
            validity_interval: ValidityInterval {
                invalid_before: None,
                invalid_hereafter: None,
            },
            mint: None,
            script_data_hash: None,
            collateral: vec![],
            required_signers: vec![],
            network_id: None,
            collateral_return: None,
            total_collateral: None,
            reference_inputs: vec![],
            witness_set: BabbageWitnessSet {
                vkey_witnesses: vec![],
                native_scripts: vec![],
                bootstrap_witnesses: vec![],
                plutus_v1_scripts: vec![],
                plutus_v2_scripts: vec![],
                plutus_data: vec![],
                redeemers: vec![],
            },
        };

        let conway_tx = ConwayTransaction::from_babbage(babbage_tx)
            .with_treasury_donation(1_000_000);

        assert_eq!(conway_tx.treasury_donation, Some(1_000_000));
        assert!(conway_tx.voting_procedures.is_empty());
        assert!(conway_tx.proposal_procedures.is_empty());
    }

    #[test]
    fn test_voting_thresholds_defaults() {
        let thresholds = VotingThresholds::default();

        // Constitution changes require 75% thresholds
        assert_eq!(thresholds.update_constitution.drep_threshold.numerator, 75);
        assert_eq!(thresholds.update_constitution.drep_threshold.denominator, 100);

        // No confidence is 51%
        assert_eq!(thresholds.motion_no_confidence.drep_threshold.numerator, 51);
        assert_eq!(thresholds.motion_no_confidence.drep_threshold.denominator, 100);
    }
}

impl Default for ProtocolParameterUpdate {
    fn default() -> Self {
        Self {
            min_fee_a: None,
            min_fee_b: None,
            max_block_body_size: None,
            max_transaction_size: None,
            max_block_header_size: None,
            key_deposit: None,
            pool_deposit: None,
            min_pool_cost: None,
            price_memory: None,
            price_steps: None,
            max_tx_execution_units: None,
            max_block_execution_units: None,
            max_value_size: None,
            collateral_percentage: None,
            max_collateral_inputs: None,
            cost_model_plutus_v1: None,
            cost_model_plutus_v2: None,
            cost_model_plutus_v3: None,
            governance_voting_thresholds: None,
            governance_action_lifetime: None,
            governance_action_deposit: None,
            drep_deposit: None,
            drep_activity: None,
            min_committee_size: None,
            committee_max_term_length: None,
        }
    }
}
