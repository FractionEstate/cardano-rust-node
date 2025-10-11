# N2 Roadmap - Task 11: Mempool Foundation

**Task 11:** Implement Mempool Data Structures and Transaction Queue

- **Status**: Not Started
- **Files**:
  - Create: `crates/cardano-mempool/Cargo.toml`
  - Create: `crates/cardano-mempool/src/lib.rs`
  - Create: `crates/cardano-mempool/src/mempool.rs` (~400 lines)
  - Create: `crates/cardano-mempool/src/tx_queue.rs` (~300 lines)
  - Create: `tests/mempool/mempool_tests.rs`
- **Description**: Create mempool crate with transaction storage, priority queue, validation hooks, and memory management for pending transactions awaiting block inclusion.

## Task Checklist

### Mempool Core Structure

- [ ] Create cardano-mempool crate
- [ ] Define Transaction struct (simplified)
- [ ] Implement TxId type (Blake2b256 hash)
- [ ] Create Mempool struct with HashMap storage
- [ ] Add size tracking (bytes and tx count)
- [ ] Implement memory limits configuration

### Transaction Queue

- [ ] Design priority queue (fee per byte)
- [ ] Implement FIFO within same priority
- [ ] Add transaction eviction policy (lowest fee first)
- [ ] Create transaction replacement (RBF - Replace By Fee)
- [ ] Add age-based eviction (TTL for transactions)
- [ ] Implement queue iteration

### Transaction Validation Hooks

- [ ] Define ValidationHook trait
- [ ] Add pre-validation checks (size, fee minimum)
- [ ] Create conflict detection (double-spend)
- [ ] Implement dependency tracking (UTxO availability)
- [ ] Add validation result caching
- [ ] Hook integration points

### Memory Management

- [ ] Configure max mempool size (bytes)
- [ ] Configure max transaction count
- [ ] Implement eviction triggers (80% threshold)
- [ ] Add memory pressure handling
- [ ] Create metrics (size, count, evictions)
- [ ] Implement periodic cleanup

### Testing

- [ ] Test add/remove transactions
- [ ] Test priority ordering
- [ ] Test eviction policies
- [ ] Test conflict detection
- [ ] Test memory limits enforcement
- [ ] Test replacement scenarios
- [ ] Benchmark mempool performance
- [ ] Test concurrent access

## Implementation Details

### Mempool Structure

```rust
// crates/cardano-mempool/src/mempool.rs

use std::collections::{HashMap, BTreeSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use blake2::{Blake2b256, Digest};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TxId(pub [u8; 32]);

impl TxId {
    pub fn compute(tx_bytes: &[u8]) -> Self {
        let mut hasher = Blake2b256::new();
        hasher.update(tx_bytes);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        Self(hash)
    }
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: TxId,
    pub raw_bytes: Vec<u8>,
    pub size_bytes: usize,
    pub fee: u64,
    pub inputs: Vec<UtxoId>,
    pub outputs: Vec<TxOutput>,
    pub received_at: SystemTime,
}

impl Transaction {
    pub fn fee_per_byte(&self) -> f64 {
        self.fee as f64 / self.size_bytes as f64
    }
}

#[derive(Debug, Clone)]
pub struct MempoolConfig {
    /// Maximum mempool size in bytes (default: 50MB)
    pub max_size_bytes: usize,

    /// Maximum number of transactions (default: 10,000)
    pub max_tx_count: usize,

    /// Minimum fee per byte (lovelace/byte)
    pub min_fee_per_byte: f64,

    /// Transaction time-to-live (seconds)
    pub tx_ttl_seconds: u64,

    /// Eviction threshold (% of max_size_bytes)
    pub eviction_threshold: f32,
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: 50 * 1024 * 1024, // 50MB
            max_tx_count: 10_000,
            min_fee_per_byte: 0.001, // 0.001 ADA per byte
            tx_ttl_seconds: 3600, // 1 hour
            eviction_threshold: 0.8, // 80%
        }
    }
}

pub struct Mempool {
    config: MempoolConfig,

    /// Transaction storage by ID
    transactions: Arc<RwLock<HashMap<TxId, Transaction>>>,

    /// Priority queue ordered by fee per byte
    priority_queue: Arc<RwLock<BTreeSet<PriorityEntry>>>,

    /// Current mempool size in bytes
    current_size: Arc<RwLock<usize>>,

    /// Metrics
    metrics: Arc<RwLock<MempoolMetrics>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PriorityEntry {
    /// Fee per byte (scaled to u64 for ordering)
    fee_per_byte_scaled: u64,

    /// Received timestamp (for FIFO within same fee)
    received_at: SystemTime,

    /// Transaction ID
    tx_id: TxId,
}

#[derive(Debug, Clone, Default)]
pub struct MempoolMetrics {
    pub tx_count: usize,
    pub total_size_bytes: usize,
    pub total_fee: u64,
    pub additions: u64,
    pub removals: u64,
    pub evictions: u64,
    pub rejections: u64,
}

impl Mempool {
    pub async fn new(config: MempoolConfig) -> Self {
        Self {
            config,
            transactions: Arc::new(RwLock::new(HashMap::new())),
            priority_queue: Arc::new(RwLock::new(BTreeSet::new())),
            current_size: Arc::new(RwLock::new(0)),
            metrics: Arc::new(RwLock::new(MempoolMetrics::default())),
        }
    }

    /// Add transaction to mempool
    pub async fn add_transaction(&self, tx: Transaction) -> Result<(), MempoolError> {
        // Pre-validation
        if tx.fee_per_byte() < self.config.min_fee_per_byte {
            let mut metrics = self.metrics.write().await;
            metrics.rejections += 1;
            return Err(MempoolError::FeeTooLow);
        }

        // Check size limit
        let current_size = *self.current_size.read().await;
        let new_size = current_size + tx.size_bytes;

        if new_size > self.config.max_size_bytes {
            // Try eviction first
            self.evict_if_needed(tx.size_bytes).await?;
        }

        // Check for conflicts (double-spend)
        if self.has_conflict(&tx).await? {
            return Err(MempoolError::Conflict);
        }

        // Add to storage
        let priority = PriorityEntry {
            fee_per_byte_scaled: (tx.fee_per_byte() * 1_000_000.0) as u64,
            received_at: tx.received_at,
            tx_id: tx.id,
        };

        let mut transactions = self.transactions.write().await;
        let mut queue = self.priority_queue.write().await;
        let mut size = self.current_size.write().await;
        let mut metrics = self.metrics.write().await;

        transactions.insert(tx.id, tx.clone());
        queue.insert(priority);
        *size += tx.size_bytes;

        metrics.additions += 1;
        metrics.tx_count = transactions.len();
        metrics.total_size_bytes = *size;
        metrics.total_fee += tx.fee;

        Ok(())
    }

    /// Remove transaction from mempool
    pub async fn remove_transaction(&self, tx_id: TxId) -> Result<Transaction, MempoolError> {
        let mut transactions = self.transactions.write().await;
        let mut queue = self.priority_queue.write().await;
        let mut size = self.current_size.write().await;
        let mut metrics = self.metrics.write().await;

        let tx = transactions.remove(&tx_id)
            .ok_or(MempoolError::NotFound)?;

        // Remove from priority queue
        let priority = PriorityEntry {
            fee_per_byte_scaled: (tx.fee_per_byte() * 1_000_000.0) as u64,
            received_at: tx.received_at,
            tx_id: tx.id,
        };
        queue.remove(&priority);

        *size -= tx.size_bytes;
        metrics.removals += 1;
        metrics.tx_count = transactions.len();
        metrics.total_size_bytes = *size;
        metrics.total_fee -= tx.fee;

        Ok(tx)
    }

    /// Get highest priority transactions for block building
    pub async fn get_top_transactions(&self, max_size: usize) -> Vec<Transaction> {
        let transactions = self.transactions.read().await;
        let queue = self.priority_queue.read().await;

        let mut result = Vec::new();
        let mut total_size = 0;

        // Iterate from highest to lowest priority
        for entry in queue.iter().rev() {
            if let Some(tx) = transactions.get(&entry.tx_id) {
                if total_size + tx.size_bytes > max_size {
                    break;
                }
                result.push(tx.clone());
                total_size += tx.size_bytes;
            }
        }

        result
    }

    /// Check if transaction conflicts with mempool
    async fn has_conflict(&self, tx: &Transaction) -> Result<bool, MempoolError> {
        let transactions = self.transactions.read().await;

        // Check if any input is already spent by mempool tx
        for input in &tx.inputs {
            for existing_tx in transactions.values() {
                if existing_tx.inputs.contains(input) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Evict low-priority transactions to make room
    async fn evict_if_needed(&self, needed_bytes: usize) -> Result<(), MempoolError> {
        let current_size = *self.current_size.read().await;
        let threshold = (self.config.max_size_bytes as f32 * self.config.eviction_threshold) as usize;

        if current_size + needed_bytes <= threshold {
            return Ok(());
        }

        // Evict lowest priority transactions
        let queue = self.priority_queue.read().await;
        let mut to_evict = Vec::new();
        let mut freed = 0;

        for entry in queue.iter() {
            let transactions = self.transactions.read().await;
            if let Some(tx) = transactions.get(&entry.tx_id) {
                to_evict.push(entry.tx_id);
                freed += tx.size_bytes;

                if freed >= needed_bytes {
                    break;
                }
            }
        }

        drop(queue);

        // Actually remove them
        for tx_id in to_evict {
            self.remove_transaction(tx_id).await?;
            let mut metrics = self.metrics.write().await;
            metrics.evictions += 1;
        }

        Ok(())
    }

    /// Remove expired transactions
    pub async fn cleanup_expired(&self) -> Result<usize, MempoolError> {
        let now = SystemTime::now();
        let ttl = Duration::from_secs(self.config.tx_ttl_seconds);

        let transactions = self.transactions.read().await;
        let mut expired = Vec::new();

        for (tx_id, tx) in transactions.iter() {
            if let Ok(age) = now.duration_since(tx.received_at) {
                if age > ttl {
                    expired.push(*tx_id);
                }
            }
        }

        drop(transactions);

        let count = expired.len();
        for tx_id in expired {
            self.remove_transaction(tx_id).await?;
        }

        Ok(count)
    }

    pub async fn get_metrics(&self) -> MempoolMetrics {
        self.metrics.read().await.clone()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MempoolError {
    #[error("Transaction fee too low")]
    FeeTooLow,

    #[error("Transaction conflicts with mempool")]
    Conflict,

    #[error("Mempool full, cannot evict enough transactions")]
    Full,

    #[error("Transaction not found")]
    NotFound,
}
```

### Simplified Transaction Types

```rust
// For now, use simplified transaction types
// Will be replaced with full Cardano transaction types in L2

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UtxoId {
    pub tx_id: TxId,
    pub output_index: u32,
}

#[derive(Debug, Clone)]
pub struct TxOutput {
    pub address: Vec<u8>,
    pub amount: u64,
}
```

## Success Criteria

- [ ] Mempool can add/remove transactions
- [ ] Priority queue orders by fee per byte correctly
- [ ] Eviction removes lowest-priority transactions
- [ ] Conflict detection prevents double-spends
- [ ] Memory limits are enforced
- [ ] Metrics track mempool state accurately
- [ ] All tests pass (8+ tests)
- [ ] Benchmark shows <10ms per transaction operation
- [ ] Concurrent access is thread-safe
- [ ] Documentation complete

## Dependencies

- External: blake2, thiserror, tokio

## Testing

```bash
# Run mempool tests
cargo test --package cardano-mempool

# Benchmark performance
cargo bench --package cardano-mempool

# Test with concurrent access
cargo test --package cardano-mempool concurrent -- --test-threads=1
```

## Performance Targets

- Add transaction: <5ms (p99)
- Remove transaction: <3ms (p99)
- Get top transactions: <20ms for 1000 txs
- Memory overhead: <10% of stored transaction data
- Concurrent throughput: >1000 tx/sec

## Estimated Effort

- Mempool core: 4-5 hours
- Priority queue: 3-4 hours
- Conflict detection: 2-3 hours
- Memory management: 2-3 hours
- Testing and benchmarks: 4-5 hours
- **Total: 15-20 hours**

## Future Enhancements

- [ ] Full Cardano transaction validation
- [ ] Smart eviction (preserve tx chains)
- [ ] Mempool synchronization between peers
- [ ] Persistent mempool (survive restarts)
- [ ] Script execution cost estimation
- [ ] Multi-dimensional priority (fee + script cost)
