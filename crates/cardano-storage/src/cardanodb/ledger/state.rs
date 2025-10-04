//! Ledger state representation
//!
//! This module defines the in-memory ledger state structure containing
//! the UTxO set, stake distribution, and other protocol state.

use crate::cardanodb::types::{BlockNo, EpochNo, SlotNo};
use serde::{Deserialize, Serialize};

/// In-memory ledger state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerState {
    /// Current slot
    pub slot: SlotNo,

    /// Current block number
    pub block_no: BlockNo,

    /// Current epoch
    pub epoch: EpochNo,
    // TODO: Add UTxO set, stake distribution, etc. in next phase
    // pub utxo: HashMap<TxInput, TxOutput>,
    // pub stake: HashMap<StakeCredential, Coin>,
    // pub delegations: HashMap<StakeCredential, PoolId>,
    // pub pools: HashMap<PoolId, PoolParameters>,
    // pub protocol_params: ProtocolParameters,
}

impl LedgerState {
    /// Create the genesis ledger state
    pub fn genesis() -> Self {
        Self {
            slot: SlotNo(0),
            block_no: BlockNo(0),
            epoch: EpochNo(0),
        }
    }

    /// Check if this is the genesis state
    pub fn is_genesis(&self) -> bool {
        self.block_no == BlockNo(0)
    }
}

impl Default for LedgerState {
    fn default() -> Self {
        Self::genesis()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_state_is_at_block_zero() {
        let state = LedgerState::genesis();
        assert_eq!(state.block_no, BlockNo(0));
        assert_eq!(state.slot, SlotNo(0));
        assert_eq!(state.epoch, EpochNo(0));
        assert!(state.is_genesis());
    }

    #[test]
    fn ledger_state_serialization_roundtrip() {
        let state = LedgerState {
            slot: SlotNo(12345),
            block_no: BlockNo(6789),
            epoch: EpochNo(123),
        };

        let bytes = serde_json::to_vec(&state).unwrap();
        let decoded: LedgerState = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded.slot, state.slot);
        assert_eq!(decoded.block_no, state.block_no);
        assert_eq!(decoded.epoch, state.epoch);
    }
}
