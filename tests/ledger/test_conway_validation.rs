//! Conway Era Validation Tests
//!
//! Tests for Conway era ledger rules validation.
//! Conway introduces comprehensive on-chain governance (CIP-1694):
//! - Governance actions and voting mechanisms
//! - Delegated Representatives (DReps) system
//! - Constitutional Committee with hot/cold keys
//! - Treasury management and parameter updates

use cardano_ledger::{LedgerError, Result};
use cardano_crypto::{Blake2b256Hash, Ed25519KeyHash, VrfProof};
use std::collections::HashMap;

/// Re-export types from previous eras
pub use crate::test_babbage_validation::{
    BabbageTransaction, BabbageTransactionOutput, BabbageWitnessSet,
    BabbageRedeemer, BabbageLedgerState, OutputDatum, ScriptReference
};
pub use crate::test_alonzo_validation::{PlutusScript, PlutusData, ExUnits, RedeemerTag};
pub use crate::test_mary_validation::{MaryValue, PolicyId, AssetName, Mint};
pub use crate::test_allegra_validation::{ValidityInterval, NativeScript, VKeyWitness};
pub use crate::test_shelley_validation::{ShelleyAddress, ShelleyTxIn, Credential, NetworkId};

/// Governance action types
#[derive(Debug, Clone, PartialEq)]
pub enum GovernanceAction {
    /// Motion of no-confidence in the constitutional committee
    NoConfidence {
        governance_action_id: Option<GovernanceActionId>,
    },
    /// Update constitutional committee members
    UpdateCommittee {
        governance_action_id: Option<GovernanceActionId>,
        members_to_remove: Vec<CommitteeColdKey>,
        members_to_add: HashMap<CommitteeColdKey, u64>, // key -> expiration epoch
        threshold: Rational, // Quorum threshold
    },
    /// New constitution
    NewConstitution {
        governance_action_id: Option<GovernanceActionId>,
        constitution: Constitution,
    },
    /// Hard fork initiation
    HardForkInitiation {
        governance_action_id: Option<GovernanceActionId>,
        protocol_version: ProtocolVersion,
    },
    /// Protocol parameter changes
    ParameterChange {
        governance_action_id: Option<GovernanceActionId>,
        parameter_update: ProtocolParameterUpdate,
        policy_hash: Option<Blake2b256Hash>, // Guardrails script hash
    },
    /// Treasury withdrawal
    TreasuryWithdrawal {
        withdrawals: HashMap<ShelleyAddress, u64>, // reward address -> amount
        policy_hash: Option<Blake2b256Hash>, // Guardrails script hash
    },
    /// Information action (no on-chain effect)
    InfoAction,
}

/// Governance action identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GovernanceActionId {
    pub transaction_id: Blake2b256Hash,
    pub governance_action_index: u32,
}

/// Vote on governance actions
#[derive(Debug, Clone)]
pub struct Vote {
    pub governance_action_id: GovernanceActionId,
    pub voter: Voter,
    pub vote_choice: VoteChoice,
    pub anchor: Option<Anchor>, // Optional metadata anchor
}

/// Voter types
#[derive(Debug, Clone, PartialEq)]
pub enum Voter {
    ConstitutionalCommittee(CommitteeHotKey),
    DelegatedRepresentative(DRepId),
    StakePool(Ed25519KeyHash), // Pool ID
}

/// Vote choices
#[derive(Debug, Clone, PartialEq)]
pub enum VoteChoice {
    No,
    Yes,
    Abstain,
}

/// Delegated Representative ID
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DRepId {
    KeyHash(Ed25519KeyHash),
    ScriptHash(Blake2b256Hash),
    AlwaysAbstain,
    AlwaysNoConfidence,
}

/// Constitutional committee keys
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommitteeColdKey(pub Ed25519KeyHash);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommitteeHotKey(pub Ed25519KeyHash);

/// Constitution document
#[derive(Debug, Clone)]
pub struct Constitution {
    pub anchor: Option<Anchor>, // Metadata reference
    pub script_hash: Option<Blake2b256Hash>, // Optional guardrails script
}

/// Protocol version for hard forks
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolVersion {
    pub major: u32,
    pub minor: u32,
}

/// Rational number for thresholds
#[derive(Debug, Clone, PartialEq)]
pub struct Rational {
    pub numerator: u64,
    pub denominator: u64,
}

/// Protocol parameter updates
#[derive(Debug, Clone)]
pub struct ProtocolParameterUpdate {
    pub min_fee_a: Option<u64>,
    pub min_fee_b: Option<u64>,
    pub max_block_body_size: Option<u32>,
    pub max_tx_size: Option<u32>,
    pub max_block_header_size: Option<u32>,
    pub key_deposit: Option<u64>,
    pub pool_deposit: Option<u64>,
    pub e_max: Option<u64>, // Maximum epoch
    pub n_opt: Option<u32>, // Optimal number of pools
    pub pool_pledge_influence: Option<Rational>,
    pub expansion_rate: Option<Rational>,
    pub treasury_growth_rate: Option<Rational>,
    pub min_pool_cost: Option<u64>,
    pub ada_per_utxo_byte: Option<u64>,
    pub cost_models_for_script_languages: Option<HashMap<u32, Vec<i64>>>,
    pub execution_costs: Option<ExUnits>,
    pub max_tx_ex_units: Option<ExUnits>,
    pub max_block_ex_units: Option<ExUnits>,
    pub max_value_size: Option<u32>,
    pub collateral_percentage: Option<u32>,
    pub max_collateral_inputs: Option<u32>,
    /// Conway era governance parameters
    pub pool_voting_thresholds: Option<PoolVotingThresholds>,
    pub drep_voting_thresholds: Option<DRepVotingThresholds>,
    pub min_committee_size: Option<u32>,
    pub committee_max_term_length: Option<u64>, // In epochs
    pub governance_action_lifetime: Option<u64>, // In epochs
    pub governance_action_deposit: Option<u64>,
    pub drep_deposit: Option<u64>,
    pub drep_activity: Option<u64>, // DRep activity period in epochs
}

/// Voting thresholds for stake pools
#[derive(Debug, Clone)]
pub struct PoolVotingThresholds {
    pub motion_no_confidence: Rational,
    pub committee_normal: Rational,
    pub committee_no_confidence: Rational,
    pub hard_fork_initiation: Rational,
    pub pp_security_group: Rational,
}

/// Voting thresholds for DReps
#[derive(Debug, Clone)]
pub struct DRepVotingThresholds {
    pub motion_no_confidence: Rational,
    pub committee_normal: Rational,
    pub committee_no_confidence: Rational,
    pub update_to_constitution: Rational,
    pub hard_fork_initiation: Rational,
    pub pp_network_group: Rational,
    pub pp_economic_group: Rational,
    pub pp_technical_group: Rational,
    pub pp_governance_group: Rational,
    pub treasury_withdrawal: Rational,
}

/// Metadata anchor
#[derive(Debug, Clone)]
pub struct Anchor {
    pub url: String,
    pub data_hash: Blake2b256Hash,
}

/// Conway certificates with governance support
#[derive(Debug, Clone)]
pub enum ConwayCertificate {
    // Existing Shelley certificates
    StakeRegistration(Credential),
    StakeDeregistration(Credential),
    StakeDelegation {
        stake_credential: Credential,
        pool_id: Ed25519KeyHash,
    },
    PoolRegistration {
        pool_id: Ed25519KeyHash,
        vrf_key: Blake2b256Hash,
        pledge: u64,
        cost: u64,
        margin: Rational,
        reward_account: ShelleyAddress,
        owners: Vec<Ed25519KeyHash>,
    },
    PoolRetirement {
        pool_id: Ed25519KeyHash,
        epoch: u64,
    },
    // New Conway governance certificates
    DRepRegistration {
        drep_id: DRepId,
        deposit: u64,
        anchor: Option<Anchor>,
    },
    DRepDeregistration {
        drep_id: DRepId,
        refund: u64,
    },
    DRepUpdate {
        drep_id: DRepId,
        anchor: Option<Anchor>,
    },
    VoteDelegation {
        stake_credential: Credential,
        drep_id: DRepId,
    },
    StakeVoteDelegation {
        stake_credential: Credential,
        pool_id: Ed25519KeyHash,
        drep_id: DRepId,
    },
    StakeRegDelegation {
        stake_credential: Credential,
        pool_id: Ed25519KeyHash,
        deposit: u64,
    },
    VoteRegDelegation {
        stake_credential: Credential,
        drep_id: DRepId,
        deposit: u64,
    },
    StakeVoteRegDelegation {
        stake_credential: Credential,
        pool_id: Ed25519KeyHash,
        drep_id: DRepId,
        deposit: u64,
    },
    CommitteeHotAuth {
        committee_cold_key: CommitteeColdKey,
        committee_hot_key: CommitteeHotKey,
    },
    CommitteeColdResign {
        committee_cold_key: CommitteeColdKey,
        anchor: Option<Anchor>,
    },
}

/// Conway transaction with governance support
#[derive(Debug, Clone)]
pub struct ConwayTransaction {
    pub inputs: Vec<ShelleyTxIn>,
    pub reference_inputs: Vec<ShelleyTxIn>,
    pub outputs: Vec<BabbageTransactionOutput>,
    pub collateral: Vec<ShelleyTxIn>,
    pub collateral_return: Option<BabbageTransactionOutput>,
    pub total_collateral: Option<u64>,
    pub fee: u64,
    pub validity_interval: Option<ValidityInterval>,
    pub certificates: Vec<ConwayCertificate>,
    pub withdrawals: HashMap<ShelleyAddress, u64>,
    pub mint: Option<Mint>,
    pub required_signers: Vec<Ed25519KeyHash>,
    pub network_id: Option<NetworkId>,
    /// New Conway fields
    pub voting_procedures: Vec<Vote>,
    pub governance_actions: Vec<GovernanceAction>,
    pub donation: Option<u64>, // Donation to treasury
}

/// Conway witness set with governance support
#[derive(Debug, Clone)]
pub struct ConwayWitnessSet {
    pub vkey_witnesses: Vec<VKeyWitness>,
    pub native_scripts: Vec<NativeScript>,
    pub plutus_v1_scripts: Vec<PlutusScript>,
    pub plutus_v2_scripts: Vec<PlutusScript>,
    pub plutus_data: Vec<PlutusData>,
    pub redeemers: Vec<BabbageRedeemer>,
}

/// Conway UTXO entry
#[derive(Debug, Clone)]
pub struct ConwayUtxo {
    pub tx_out: BabbageTransactionOutput,
    pub spent: bool,
}

/// Conway ledger state with governance tracking
#[derive(Debug, Clone)]
pub struct ConwayLedgerState {
    pub base: BabbageLedgerState,
    pub conway_utxo_set: HashMap<ShelleyTxIn, ConwayUtxo>,
    pub current_slot: u64,
    pub current_epoch: u64,

    /// Governance state
    pub committee: ConstitutionalCommittee,
    pub constitution: Constitution,
    pub dreps: HashMap<DRepId, DRepInfo>,
    pub governance_actions: HashMap<GovernanceActionId, GovernanceActionInfo>,
    pub protocol_parameters: ConwayProtocolParameters,

    /// Voting tracking
    pub votes: HashMap<GovernanceActionId, HashMap<Voter, VoteChoice>>,
    pub stake_delegation: HashMap<Credential, Ed25519KeyHash>, // stake -> pool
    pub vote_delegation: HashMap<Credential, DRepId>, // stake -> drep
    pub treasury: u64,
}

/// Constitutional committee state
#[derive(Debug, Clone)]
pub struct ConstitutionalCommittee {
    pub members: HashMap<CommitteeColdKey, CommitteeMember>,
    pub threshold: Rational,
}

#[derive(Debug, Clone)]
pub struct CommitteeMember {
    pub expiration_epoch: u64,
    pub hot_key: Option<CommitteeHotKey>,
    pub status: CommitteeStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommitteeStatus {
    Active,
    Expired,
    Resigned,
}

/// DRep information
#[derive(Debug, Clone)]
pub struct DRepInfo {
    pub deposit: u64,
    pub anchor: Option<Anchor>,
    pub last_activity_epoch: u64,
    pub voting_power: u64, // Delegated stake
}

/// Governance action state
#[derive(Debug, Clone)]
pub struct GovernanceActionInfo {
    pub action: GovernanceAction,
    pub deposit: u64,
    pub return_address: ShelleyAddress,
    pub expiration_epoch: u64,
    pub votes: HashMap<Voter, VoteChoice>,
}

/// Conway protocol parameters
#[derive(Debug, Clone)]
pub struct ConwayProtocolParameters {
    pub pool_voting_thresholds: PoolVotingThresholds,
    pub drep_voting_thresholds: DRepVotingThresholds,
    pub min_committee_size: u32,
    pub committee_max_term_length: u64,
    pub governance_action_lifetime: u64,
    pub governance_action_deposit: u64,
    pub drep_deposit: u64,
    pub drep_activity_period: u64,
}

impl Rational {
    pub fn new(numerator: u64, denominator: u64) -> Result<Self> {
        if denominator == 0 {
            return Err(LedgerError::InvalidRational("Denominator cannot be zero".to_string()));
        }
        Ok(Self { numerator, denominator })
    }

    pub fn to_f64(&self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }

    pub fn is_greater_than(&self, other: &Rational) -> bool {
        self.numerator * other.denominator > other.numerator * self.denominator
    }
}

impl ConwayTransaction {
    /// Validate transaction against Conway era rules
    pub fn validate(&self, witnesses: &ConwayWitnessSet, ledger_state: &ConwayLedgerState) -> Result<()> {
        // Basic validation (inherited from Babbage)
        self.validate_basic_rules(witnesses, ledger_state)?;

        // Validate governance actions
        self.validate_governance_actions(ledger_state)?;

        // Validate voting procedures
        self.validate_voting_procedures(ledger_state)?;

        // Validate Conway certificates
        self.validate_conway_certificates(ledger_state)?;

        // Validate donation
        if let Some(donation) = self.donation {
            if donation == 0 {
                return Err(LedgerError::InvalidTransaction("Donation must be positive".to_string()));
            }
        }

        Ok(())
    }

    fn validate_basic_rules(&self, _witnesses: &ConwayWitnessSet, ledger_state: &ConwayLedgerState) -> Result<()> {
        // Validity interval check
        if let Some(validity_interval) = &self.validity_interval {
            if !validity_interval.is_valid_at_slot(ledger_state.current_slot) {
                return Err(LedgerError::InvalidTransaction("Transaction outside validity interval".to_string()));
            }
        }

        // Network ID check
        if let Some(tx_network_id) = &self.network_id {
            if *tx_network_id != ledger_state.base.network_id {
                return Err(LedgerError::InvalidTransaction("Network ID mismatch".to_string()));
            }
        }

        // Check inputs exist and are unspent
        for input in &self.inputs {
            if !ledger_state.conway_utxo_set.contains_key(input) {
                return Err(LedgerError::InvalidInput(format!("Input {:?} not found", input.tx_id)));
            }

            if ledger_state.conway_utxo_set[input].spent {
                return Err(LedgerError::InvalidInput("Input already spent".to_string()));
            }
        }

        // Check reference inputs exist
        for ref_input in &self.reference_inputs {
            if !ledger_state.conway_utxo_set.contains_key(ref_input) {
                return Err(LedgerError::InvalidInput("Reference input not found".to_string()));
            }
        }

        Ok(())
    }

    fn validate_governance_actions(&self, ledger_state: &ConwayLedgerState) -> Result<()> {
        for action in &self.governance_actions {
            match action {
                GovernanceAction::UpdateCommittee { members_to_add, threshold, .. } => {
                    // Validate threshold
                    if threshold.to_f64() < 0.5 || threshold.to_f64() > 1.0 {
                        return Err(LedgerError::InvalidGovernanceAction("Committee threshold must be between 0.5 and 1.0".to_string()));
                    }

                    // Check member expiration epochs are in the future
                    for &expiration_epoch in members_to_add.values() {
                        if expiration_epoch <= ledger_state.current_epoch {
                            return Err(LedgerError::InvalidGovernanceAction("Committee member expiration must be in future".to_string()));
                        }
                    }
                }

                GovernanceAction::HardForkInitiation { protocol_version, .. } => {
                    // Validate protocol version is an increment
                    if protocol_version.major == 0 || protocol_version.minor > 255 {
                        return Err(LedgerError::InvalidGovernanceAction("Invalid protocol version".to_string()));
                    }
                }

                GovernanceAction::TreasuryWithdrawal { withdrawals, .. } => {
                    let total_withdrawal: u64 = withdrawals.values().sum();
                    if total_withdrawal > ledger_state.treasury {
                        return Err(LedgerError::InvalidGovernanceAction("Treasury withdrawal exceeds available funds".to_string()));
                    }
                }

                _ => {} // Other actions have no special validation
            }
        }

        Ok(())
    }

    fn validate_voting_procedures(&self, ledger_state: &ConwayLedgerState) -> Result<()> {
        for vote in &self.voting_procedures {
            // Check governance action exists
            if !ledger_state.governance_actions.contains_key(&vote.governance_action_id) {
                return Err(LedgerError::InvalidVote("Governance action not found".to_string()));
            }

            let gov_action_info = &ledger_state.governance_actions[&vote.governance_action_id];

            // Check action hasn't expired
            if ledger_state.current_epoch >= gov_action_info.expiration_epoch {
                return Err(LedgerError::InvalidVote("Governance action has expired".to_string()));
            }

            // Validate voter eligibility
            match &vote.voter {
                Voter::ConstitutionalCommittee(hot_key) => {
                    self.validate_committee_vote(hot_key, ledger_state)?;
                }

                Voter::DelegatedRepresentative(drep_id) => {
                    self.validate_drep_vote(drep_id, ledger_state)?;
                }

                Voter::StakePool(pool_id) => {
                    self.validate_pool_vote(pool_id, ledger_state)?;
                }
            }
        }

        Ok(())
    }

    fn validate_committee_vote(&self, hot_key: &CommitteeHotKey, ledger_state: &ConwayLedgerState) -> Result<()> {
        // Find committee member with this hot key
        let member = ledger_state.committee.members.values()
            .find(|member| member.hot_key.as_ref() == Some(hot_key))
            .ok_or_else(|| LedgerError::InvalidVote("Committee hot key not authorized".to_string()))?;

        if member.status != CommitteeStatus::Active {
            return Err(LedgerError::InvalidVote("Committee member not active".to_string()));
        }

        if member.expiration_epoch <= ledger_state.current_epoch {
            return Err(LedgerError::InvalidVote("Committee member has expired".to_string()));
        }

        Ok(())
    }

    fn validate_drep_vote(&self, drep_id: &DRepId, ledger_state: &ConwayLedgerState) -> Result<()> {
        match drep_id {
            DRepId::AlwaysAbstain | DRepId::AlwaysNoConfidence => {
                // Special DReps are always valid
                Ok(())
            }

            DRepId::KeyHash(_) | DRepId::ScriptHash(_) => {
                let drep_info = ledger_state.dreps.get(drep_id)
                    .ok_or_else(|| LedgerError::InvalidVote("DRep not registered".to_string()))?;

                // Check DRep is still active (hasn't expired due to inactivity)
                let activity_deadline = drep_info.last_activity_epoch + ledger_state.protocol_parameters.drep_activity_period;
                if ledger_state.current_epoch > activity_deadline {
                    return Err(LedgerError::InvalidVote("DRep has expired due to inactivity".to_string()));
                }

                Ok(())
            }
        }
    }

    fn validate_pool_vote(&self, pool_id: &Ed25519KeyHash, _ledger_state: &ConwayLedgerState) -> Result<()> {
        // Check pool is registered and active (simplified)
        // In reality, would check pool registration status
        if pool_id.as_ref().is_empty() {
            return Err(LedgerError::InvalidVote("Invalid pool ID".to_string()));
        }

        Ok(())
    }

    fn validate_conway_certificates(&self, ledger_state: &ConwayLedgerState) -> Result<()> {
        for cert in &self.certificates {
            match cert {
                ConwayCertificate::DRepRegistration { drep_id, deposit, .. } => {
                    if ledger_state.dreps.contains_key(drep_id) {
                        return Err(LedgerError::InvalidCertificate("DRep already registered".to_string()));
                    }

                    if *deposit < ledger_state.protocol_parameters.drep_deposit {
                        return Err(LedgerError::InvalidCertificate("Insufficient DRep deposit".to_string()));
                    }
                }

                ConwayCertificate::DRepDeregistration { drep_id, .. } => {
                    if !ledger_state.dreps.contains_key(drep_id) {
                        return Err(LedgerError::InvalidCertificate("DRep not registered".to_string()));
                    }
                }

                ConwayCertificate::VoteDelegation { stake_credential, drep_id } => {
                    // Check stake credential is registered
                    if !ledger_state.stake_delegation.contains_key(stake_credential) {
                        return Err(LedgerError::InvalidCertificate("Stake credential not registered".to_string()));
                    }

                    // Check DRep exists (unless special DRep)
                    match drep_id {
                        DRepId::AlwaysAbstain | DRepId::AlwaysNoConfidence => {}
                        _ => {
                            if !ledger_state.dreps.contains_key(drep_id) {
                                return Err(LedgerError::InvalidCertificate("DRep not registered".to_string()));
                            }
                        }
                    }
                }

                ConwayCertificate::CommitteeHotAuth { committee_cold_key, .. } => {
                    if !ledger_state.committee.members.contains_key(committee_cold_key) {
                        return Err(LedgerError::InvalidCertificate("Committee cold key not in committee".to_string()));
                    }
                }

                _ => {} // Other certificates use existing Shelley validation
            }
        }

        Ok(())
    }

    pub fn hash(&self) -> Blake2b256Hash {
        Blake2b256Hash::new(&format!("{:?}", self).as_bytes())
    }
}

impl ConwayLedgerState {
    pub fn new() -> Self {
        Self {
            base: BabbageLedgerState::new(),
            conway_utxo_set: HashMap::new(),
            current_slot: 0,
            current_epoch: 0,
            committee: ConstitutionalCommittee {
                members: HashMap::new(),
                threshold: Rational::new(2, 3).unwrap(), // 2/3 majority
            },
            constitution: Constitution {
                anchor: None,
                script_hash: None,
            },
            dreps: HashMap::new(),
            governance_actions: HashMap::new(),
            protocol_parameters: ConwayProtocolParameters {
                pool_voting_thresholds: PoolVotingThresholds {
                    motion_no_confidence: Rational::new(51, 100).unwrap(),
                    committee_normal: Rational::new(51, 100).unwrap(),
                    committee_no_confidence: Rational::new(51, 100).unwrap(),
                    hard_fork_initiation: Rational::new(51, 100).unwrap(),
                    pp_security_group: Rational::new(51, 100).unwrap(),
                },
                drep_voting_thresholds: DRepVotingThresholds {
                    motion_no_confidence: Rational::new(67, 100).unwrap(),
                    committee_normal: Rational::new(67, 100).unwrap(),
                    committee_no_confidence: Rational::new(60, 100).unwrap(),
                    update_to_constitution: Rational::new(75, 100).unwrap(),
                    hard_fork_initiation: Rational::new(60, 100).unwrap(),
                    pp_network_group: Rational::new(67, 100).unwrap(),
                    pp_economic_group: Rational::new(67, 100).unwrap(),
                    pp_technical_group: Rational::new(67, 100).unwrap(),
                    pp_governance_group: Rational::new(75, 100).unwrap(),
                    treasury_withdrawal: Rational::new(67, 100).unwrap(),
                },
                min_committee_size: 3,
                committee_max_term_length: 146, // ~2 years in epochs
                governance_action_lifetime: 6,  // ~1 month in epochs
                governance_action_deposit: 100_000_000_000, // 100,000 ADA
                drep_deposit: 500_000_000, // 500 ADA
                drep_activity_period: 20, // ~3 months in epochs
            },
            votes: HashMap::new(),
            stake_delegation: HashMap::new(),
            vote_delegation: HashMap::new(),
            treasury: 1_000_000_000_000_000, // 1 billion ADA
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_shelley_validation::StakeReference;

    fn create_test_address() -> ShelleyAddress {
        ShelleyAddress {
            payment_credential: Credential::Key(Ed25519KeyHash::new(b"test_payment_key")),
            stake_credential: Some(StakeReference::Credential(
                Credential::Key(Ed25519KeyHash::new(b"test_stake_key"))
            )),
            network_id: crate::test_shelley_validation::NetworkId::Testnet,
        }
    }

    #[test]
    fn test_drep_registration_certificate() {
        let ledger = ConwayLedgerState::new();

        let drep_id = DRepId::KeyHash(Ed25519KeyHash::new(b"drep_key"));
        let cert = ConwayCertificate::DRepRegistration {
            drep_id: drep_id.clone(),
            deposit: 500_000_000, // 500 ADA
            anchor: Some(Anchor {
                url: "https://drep.example.com/metadata".to_string(),
                data_hash: Blake2b256Hash::new(b"metadata_hash"),
            }),
        };

        let tx = ConwayTransaction {
            inputs: vec![],
            reference_inputs: vec![],
            outputs: vec![],
            collateral: vec![],
            collateral_return: None,
            total_collateral: None,
            fee: 200_000,
            validity_interval: None,
            certificates: vec![cert],
            withdrawals: HashMap::new(),
            mint: None,
            required_signers: vec![],
            network_id: None,
            voting_procedures: vec![],
            governance_actions: vec![],
            donation: None,
        };

        let witnesses = ConwayWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_v2_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_ok());
    }

    #[test]
    fn test_insufficient_drep_deposit() {
        let ledger = ConwayLedgerState::new();

        let drep_id = DRepId::KeyHash(Ed25519KeyHash::new(b"drep_key"));
        let cert = ConwayCertificate::DRepRegistration {
            drep_id: drep_id.clone(),
            deposit: 100_000_000, // Only 100 ADA (insufficient)
            anchor: None,
        };

        let tx = ConwayTransaction {
            inputs: vec![],
            reference_inputs: vec![],
            outputs: vec![],
            collateral: vec![],
            collateral_return: None,
            total_collateral: None,
            fee: 200_000,
            validity_interval: None,
            certificates: vec![cert],
            withdrawals: HashMap::new(),
            mint: None,
            required_signers: vec![],
            network_id: None,
            voting_procedures: vec![],
            governance_actions: vec![],
            donation: None,
        };

        let witnesses = ConwayWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_v2_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidCertificate(_)));
    }

    #[test]
    fn test_committee_update_governance_action() {
        let ledger = ConwayLedgerState::new();

        let mut members_to_add = HashMap::new();
        members_to_add.insert(
            CommitteeColdKey(Ed25519KeyHash::new(b"new_committee_member")),
            ledger.current_epoch + 100, // Expires in 100 epochs
        );

        let action = GovernanceAction::UpdateCommittee {
            governance_action_id: None,
            members_to_remove: vec![],
            members_to_add,
            threshold: Rational::new(2, 3).unwrap(), // Valid 2/3 threshold
        };

        let tx = ConwayTransaction {
            inputs: vec![],
            reference_inputs: vec![],
            outputs: vec![],
            collateral: vec![],
            collateral_return: None,
            total_collateral: None,
            fee: 200_000,
            validity_interval: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            mint: None,
            required_signers: vec![],
            network_id: None,
            voting_procedures: vec![],
            governance_actions: vec![action],
            donation: None,
        };

        let witnesses = ConwayWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_v2_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_committee_threshold() {
        let ledger = ConwayLedgerState::new();

        let action = GovernanceAction::UpdateCommittee {
            governance_action_id: None,
            members_to_remove: vec![],
            members_to_add: HashMap::new(),
            threshold: Rational::new(1, 3).unwrap(), // Invalid threshold < 0.5
        };

        let tx = ConwayTransaction {
            inputs: vec![],
            reference_inputs: vec![],
            outputs: vec![],
            collateral: vec![],
            collateral_return: None,
            total_collateral: None,
            fee: 200_000,
            validity_interval: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            mint: None,
            required_signers: vec![],
            network_id: None,
            voting_procedures: vec![],
            governance_actions: vec![action],
            donation: None,
        };

        let witnesses = ConwayWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_v2_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidGovernanceAction(_)));
    }

    #[test]
    fn test_treasury_withdrawal_exceeds_funds() {
        let mut ledger = ConwayLedgerState::new();
        ledger.treasury = 1_000_000_000; // Only 1,000 ADA available

        let mut withdrawals = HashMap::new();
        withdrawals.insert(create_test_address(), 2_000_000_000); // Try to withdraw 2,000 ADA

        let action = GovernanceAction::TreasuryWithdrawal {
            withdrawals,
            policy_hash: None,
        };

        let tx = ConwayTransaction {
            inputs: vec![],
            reference_inputs: vec![],
            outputs: vec![],
            collateral: vec![],
            collateral_return: None,
            total_collateral: None,
            fee: 200_000,
            validity_interval: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            mint: None,
            required_signers: vec![],
            network_id: None,
            voting_procedures: vec![],
            governance_actions: vec![action],
            donation: None,
        };

        let witnesses = ConwayWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_v2_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidGovernanceAction(_)));
    }

    #[test]
    fn test_vote_delegation_certificate() {
        let mut ledger = ConwayLedgerState::new();

        // Register stake credential and DRep
        let stake_cred = Credential::Key(Ed25519KeyHash::new(b"stake_key"));
        let drep_id = DRepId::KeyHash(Ed25519KeyHash::new(b"drep_key"));

        ledger.stake_delegation.insert(stake_cred.clone(), Ed25519KeyHash::new(b"pool_id"));
        ledger.dreps.insert(drep_id.clone(), DRepInfo {
            deposit: 500_000_000,
            anchor: None,
            last_activity_epoch: ledger.current_epoch,
            voting_power: 0,
        });

        let cert = ConwayCertificate::VoteDelegation {
            stake_credential: stake_cred,
            drep_id: drep_id.clone(),
        };

        let tx = ConwayTransaction {
            inputs: vec![],
            reference_inputs: vec![],
            outputs: vec![],
            collateral: vec![],
            collateral_return: None,
            total_collateral: None,
            fee: 200_000,
            validity_interval: None,
            certificates: vec![cert],
            withdrawals: HashMap::new(),
            mint: None,
            required_signers: vec![],
            network_id: None,
            voting_procedures: vec![],
            governance_actions: vec![],
            donation: None,
        };

        let witnesses = ConwayWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_v2_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_ok());
    }

    #[test]
    fn test_special_drep_vote_delegation() {
        let mut ledger = ConwayLedgerState::new();

        let stake_cred = Credential::Key(Ed25519KeyHash::new(b"stake_key"));
        ledger.stake_delegation.insert(stake_cred.clone(), Ed25519KeyHash::new(b"pool_id"));

        // Delegate to special "always abstain" DRep
        let cert = ConwayCertificate::VoteDelegation {
            stake_credential: stake_cred,
            drep_id: DRepId::AlwaysAbstain,
        };

        let tx = ConwayTransaction {
            inputs: vec![],
            reference_inputs: vec![],
            outputs: vec![],
            collateral: vec![],
            collateral_return: None,
            total_collateral: None,
            fee: 200_000,
            validity_interval: None,
            certificates: vec![cert],
            withdrawals: HashMap::new(),
            mint: None,
            required_signers: vec![],
            network_id: None,
            voting_procedures: vec![],
            governance_actions: vec![],
            donation: None,
        };

        let witnesses = ConwayWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_v2_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![],
        };

        let result = tx.validate(&witnesses, &ledger);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rational_operations() {
        let r1 = Rational::new(2, 3).unwrap();
        let r2 = Rational::new(1, 2).unwrap();

        assert!(r1.is_greater_than(&r2)); // 2/3 > 1/2
        assert!(!r2.is_greater_than(&r1)); // 1/2 < 2/3

        assert!((r1.to_f64() - 0.6666666666666666).abs() < f64::EPSILON);
        assert!((r2.to_f64() - 0.5).abs() < f64::EPSILON);

        // Test invalid rational
        let invalid = Rational::new(1, 0);
        assert!(invalid.is_err());
    }

    #[test]
    fn test_donation_validation() {
        let ledger = ConwayLedgerState::new();

        // Valid donation
        let tx_valid = ConwayTransaction {
            inputs: vec![],
            reference_inputs: vec![],
            outputs: vec![],
            collateral: vec![],
            collateral_return: None,
            total_collateral: None,
            fee: 200_000,
            validity_interval: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            mint: None,
            required_signers: vec![],
            network_id: None,
            voting_procedures: vec![],
            governance_actions: vec![],
            donation: Some(1_000_000), // 1 ADA donation
        };

        let witnesses = ConwayWitnessSet {
            vkey_witnesses: vec![],
            native_scripts: vec![],
            plutus_v1_scripts: vec![],
            plutus_v2_scripts: vec![],
            plutus_data: vec![],
            redeemers: vec![],
        };

        let result = tx_valid.validate(&witnesses, &ledger);
        assert!(result.is_ok());

        // Invalid zero donation
        let tx_invalid = ConwayTransaction {
            donation: Some(0), // Invalid zero donation
            ..tx_valid
        };

        let result = tx_invalid.validate(&witnesses, &ledger);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LedgerError::InvalidTransaction(_)));
    }
}
