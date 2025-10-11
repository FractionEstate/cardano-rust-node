# N2 Roadmap - Task 12: Transaction Gossip Protocol

**Task 12:** Implement Transaction Gossip and Block Dissemination

- **Status**: Not Started
- **Files**:
  - Create: `crates/cardano-network/src/tx_submission.rs` (~400 lines)
  - Create: `crates/cardano-network/src/block_fetch.rs` (~350 lines)
  - Modify: `crates/cardano-mempool/src/lib.rs` (add network integration)
  - Create: `tests/network/gossip_tests.rs`
- **Description**: Implement transaction submission and block fetch protocols for gossiping pending transactions and fetching blocks from peers.

## Task Checklist

### Transaction Submission Protocol

- [ ] Implement TxSubmission mini-protocol
- [ ] Add RequestTxIds message handling
- [ ] Add RequestTxs message handling
- [ ] Implement tx announcement to peers
- [ ] Add duplicate transaction filtering
- [ ] Create flood prevention mechanism

### Block Fetch Protocol

- [ ] Implement BlockFetch mini-protocol
- [ ] Add RequestRange message handling
- [ ] Implement block streaming
- [ ] Add block validation during fetch
- [ ] Create parallel block fetching
- [ ] Add fetch priority queue

### Peer Transaction Exchange

- [ ] Design tx inventory tracking (per peer)
- [ ] Implement tx relay selection
- [ ] Add bandwidth management
- [ ] Create tx batching for efficiency
- [ ] Implement back-pressure handling
- [ ] Add peer scoring for tx relay

### Integration with Mempool

- [ ] Connect gossip to mempool
- [ ] Add tx validation before broadcast
- [ ] Implement tx rebroadcast logic
- [ ] Create tx expiry handling
- [ ] Add mempool sync on startup
- [ ] Implement tx conflict resolution

### Testing

- [ ] Test tx submission between peers
- [ ] Test block fetch from multiple peers
- [ ] Test duplicate filtering
- [ ] Test flood prevention
- [ ] Test bandwidth limits
- [ ] Benchmark gossip performance
- [ ] Test network partitions
- [ ] Test malicious peer handling

## Implementation Overview

```rust
// crates/cardano-network/src/tx_submission.rs

pub struct TxSubmissionProtocol {
    mempool: Arc<Mempool>,
    peer_inventories: Arc<RwLock<HashMap<PeerId, TxInventory>>>,
    config: TxSubmissionConfig,
}

#[derive(Clone)]
pub struct TxSubmissionConfig {
    pub max_txs_per_request: usize,
    pub max_tx_ids_per_announcement: usize,
    pub announcement_interval_ms: u64,
    pub max_pending_tx_bytes: usize,
}

impl TxSubmissionProtocol {
    pub async fn announce_transactions(&self, peer_id: PeerId, tx_ids: Vec<TxId>) -> Result<()> {
        // Send TxIds announcement to peer
        // Track what we've announced to avoid duplicates
    }

    pub async fn handle_request_txs(&self, peer_id: PeerId, tx_ids: Vec<TxId>) -> Result<Vec<Transaction>> {
        // Fetch transactions from mempool
        // Return to requesting peer
    }

    pub async fn handle_tx_ids(&self, peer_id: PeerId, tx_ids: Vec<TxId>) -> Result<()> {
        // Check which txs we don't have
        // Request missing txs from peer
    }
}
```

## Success Criteria

- [ ] Transactions propagate across network
- [ ] Duplicate filtering prevents bandwidth waste
- [ ] Block fetch retrieves missing blocks
- [ ] Flood prevention limits malicious spam
- [ ] All tests pass (8+ tests)
- [ ] Gossip latency <2 seconds for tx propagation
- [ ] Bandwidth usage is reasonable (<10MB/min)
- [ ] Documentation complete

## Estimated Effort

- **Total: 20-25 hours**
