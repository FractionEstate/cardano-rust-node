//! Chain database operation tests for Cardano Node Rust
//!
//! This module tests the chain database functionality including:
//! - Block insertion and retrieval operations
//! - Chain navigation by hash and height
//! - Fork handling and selection
//! - Rollback operations
//! - Database consistency checks

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Mock types for testing
type BlockHash = [u8; 32];
type SlotNo = u64;
type BlockNo = u64;

// Mock structures for testing

#[derive(Debug, Clone)]
pub struct MockBlock {
    hash: BlockHash,
    previous_hash: BlockHash,
    slot_no: SlotNo,
    block_no: BlockNo,
    // Simplified block data
    data: Vec<u8>,
}

impl MockBlock {
    fn new(previous_hash: BlockHash, slot_no: SlotNo, block_no: BlockNo, data: Vec<u8>) -> Self {
        let mut hash = [0u8; 32];
        hash[0..8].copy_from_slice(&block_no.to_be_bytes());
        hash[8..16].copy_from_slice(&slot_no.to_be_bytes());

        Self {
            hash,
            previous_hash,
            slot_no,
            block_no,
            data,
        }
    }

    fn genesis() -> Self {
        Self::new([0u8; 32], 0, 0, vec![])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChainTip {
    slot_no: SlotNo,
    hash: BlockHash,
    block_no: BlockNo,
}

impl ChainTip {
    fn new(slot_no: SlotNo, hash: BlockHash, block_no: BlockNo) -> Self {
        Self { slot_no, hash, block_no }
    }

    fn genesis() -> Self {
        Self::new(0, [0u8; 32], 0)
    }

    fn hash(&self) -> BlockHash {
        self.hash
    }

    fn block_no(&self) -> BlockNo {
        self.block_no
    }
}

#[derive(Debug, Clone)]
pub enum ChainPoint {
    Genesis,
    At { slot_no: SlotNo, hash: BlockHash },
}

#[derive(Debug, thiserror::Error)]
pub enum ChainDBError {
    #[error("Block not found: {0:?}")]
    BlockNotFound(BlockHash),
    #[error("No common ancestor found")]
    NoCommonAncestor,
    #[error("Invalid block: {0}")]
    InvalidBlock(String),
}

/// Mock Chain Database implementation for testing
#[derive(Debug, Clone)]
pub struct MockChainDB {
    /// Blocks indexed by hash
    blocks: Arc<RwLock<HashMap<BlockHash, MockBlock>>>,
    /// Chain tip
    tip: Arc<RwLock<ChainTip>>,
    /// Block height index
    height_index: Arc<RwLock<HashMap<BlockNo, BlockHash>>>,
    /// Current chain (ordered list of block hashes from genesis to tip)
    current_chain: Arc<RwLock<Vec<BlockHash>>>,
    /// Fork alternatives (blocks not on main chain)
    forks: Arc<RwLock<HashMap<BlockHash, MockBlock>>>,
}

impl MockChainDB {
    pub fn new() -> Self {
        Self {
            blocks: Arc::new(RwLock::new(HashMap::new())),
            tip: Arc::new(RwLock::new(ChainTip::genesis())),
            height_index: Arc::new(RwLock::new(HashMap::new())),
            current_chain: Arc::new(RwLock::new(Vec::new())),
            forks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn insert_block(&self, block: MockBlock) -> Result<(), ChainDBError> {
        let block_hash = block.hash;
        let block_no = block.block_no;

        // Insert block into main storage
        self.blocks.write().await.insert(block_hash, block.clone());

        // Update height index
        self.height_index.write().await.insert(block_no, block_hash);

        // Update current chain
        self.current_chain.write().await.push(block_hash);

        // Update tip
        let new_tip = ChainTip::new(
            block.slot_no,
            block_hash,
            block_no,
        );
        *self.tip.write().await = new_tip;

        Ok(())
    }

    async fn get_block_by_hash(&self, hash: &BlockHash) -> Option<MockBlock> {
        self.blocks.read().await.get(hash).cloned()
    }

    async fn get_block_by_height(&self, height: BlockNo) -> Option<MockBlock> {
        let hash = self.height_index.read().await.get(&height)?;
        self.blocks.read().await.get(hash).cloned()
    }

    async fn get_tip(&self) -> ChainTip {
        self.tip.read().await.clone()
    }

    async fn rollback_to(&self, point: ChainPoint) -> Result<(), ChainDBError> {
        match point {
            ChainPoint::Genesis => {
                // Rollback to genesis
                self.blocks.write().await.clear();
                self.height_index.write().await.clear();
                self.current_chain.write().await.clear();
                *self.tip.write().await = ChainTip::genesis();
            }
            ChainPoint::At { slot_no, hash } => {
                // Find the block in current chain
                let chain = self.current_chain.read().await;
                if let Some(pos) = chain.iter().position(|&h| h == hash) {
                    // Remove blocks after this position
                    drop(chain);

                    let mut chain = self.current_chain.write().await;
                    let mut blocks = self.blocks.write().await;
                    let mut height_index = self.height_index.write().await;

                    // Remove blocks from the end
                    while chain.len() > pos + 1 {
                        if let Some(removed_hash) = chain.pop() {
                            if let Some(removed_block) = blocks.remove(&removed_hash) {
                                height_index.remove(&removed_block.block_no);
                            }
                        }
                    }

                    // Update tip
                    if let Some(block) = blocks.get(&hash) {
                        *self.tip.write().await = ChainTip::new(
                            block.slot_no,
                            hash,
                            block.block_no,
                        );
                    }
                } else {
                    return Err(ChainDBError::BlockNotFound(hash));
                }
            }
        }
        Ok(())
    }

    async fn switch_to_fork(&self, fork_blocks: &[MockBlock]) -> Result<(), ChainDBError> {
        // Find common ancestor
        let current_chain = self.current_chain.read().await;
        let mut common_ancestor_pos = None;

        for (i, &chain_hash) in current_chain.iter().enumerate() {
            for fork_block in fork_blocks {
                if fork_block.previous_hash == chain_hash {
                    common_ancestor_pos = Some(i);
                    break;
                }
            }
            if common_ancestor_pos.is_some() {
                break;
            }
        }

        drop(current_chain);

        if let Some(ancestor_pos) = common_ancestor_pos {
            // Rollback to common ancestor
            let mut chain = self.current_chain.write().await;
            let mut blocks = self.blocks.write().await;
            let mut height_index = self.height_index.write().await;

            // Remove blocks after common ancestor
            while chain.len() > ancestor_pos + 1 {
                if let Some(removed_hash) = chain.pop() {
                    if let Some(removed_block) = blocks.remove(&removed_hash) {
                        height_index.remove(&removed_block.block_no);
                    }
                }
            }

            // Add fork blocks
            for block in fork_blocks {
                let block_hash = block.hash;
                let block_no = block.block_no;

                blocks.insert(block_hash, block.clone());
                height_index.insert(block_no, block_hash);
                chain.push(block_hash);

                // Update tip with last block
                *self.tip.write().await = ChainTip::new(
                    block.slot_no,
                    block_hash,
                    block_no,
                );
            }
        } else {
            return Err(ChainDBError::NoCommonAncestor);
        }

        Ok(())
    }
}

// Test helper functions
fn create_genesis_block() -> MockBlock {
    MockBlock::genesis()
}

fn create_test_block(previous_hash: BlockHash, slot_no: SlotNo, block_no: BlockNo) -> MockBlock {
    MockBlock::new(previous_hash, slot_no, block_no, vec![])
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_chaindb_block_insertion() {
        let chaindb = MockChainDB::new();

        // Create genesis block
        let genesis = create_genesis_block();
        let genesis_hash = genesis.hash;

        // Insert genesis block
        assert!(chaindb.insert_block(genesis.clone()).await.is_ok());

        // Verify block can be retrieved by hash
        let retrieved = chaindb.get_block_by_hash(&genesis_hash).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().hash, genesis_hash);

        // Verify block can be retrieved by height
        let retrieved_by_height = chaindb.get_block_by_height(0).await;
        assert!(retrieved_by_height.is_some());
        assert_eq!(retrieved_by_height.unwrap().hash, genesis_hash);

        // Verify tip is updated
        let tip = chaindb.get_tip().await;
        assert_eq!(tip.hash(), genesis_hash);
        assert_eq!(tip.block_no(), 0);
    }

    #[tokio::test]
    async fn test_chaindb_chain_extension() {
        let chaindb = MockChainDB::new();

        // Create and insert genesis block
        let genesis = create_genesis_block();
        let genesis_hash = genesis.hash;
        chaindb.insert_block(genesis).await.unwrap();

        // Create and insert next block
        let block1 = create_test_block(genesis_hash, 1, 1);
        let block1_hash = block1.hash;
        chaindb.insert_block(block1).await.unwrap();

        // Create and insert third block
        let block2 = create_test_block(block1_hash, 2, 2);
        let block2_hash = block2.hash;
        chaindb.insert_block(block2).await.unwrap();

        // Verify chain length and tip
        let tip = chaindb.get_tip().await;
        assert_eq!(tip.hash(), block2_hash);
        assert_eq!(tip.block_no(), 2);

        // Verify all blocks can be retrieved by height
        for i in 0..=2 {
            let block = chaindb.get_block_by_height(i).await;
            assert!(block.is_some());
            assert_eq!(block.unwrap().block_no, i);
        }
    }

    #[tokio::test]
    async fn test_chaindb_block_retrieval_nonexistent() {
        let chaindb = MockChainDB::new();

        // Try to retrieve non-existent block by hash
        let fake_hash = [99u8; 32];
        let result = chaindb.get_block_by_hash(&fake_hash).await;
        assert!(result.is_none());

        // Try to retrieve non-existent block by height
        let result = chaindb.get_block_by_height(999).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_chaindb_rollback_to_genesis() {
        let chaindb = MockChainDB::new();

        // Build a small chain
        let genesis = create_genesis_block();
        let genesis_hash = genesis.header().hash();
        chaindb.insert_block(genesis).await.unwrap();

        let block1 = create_test_block(genesis_hash, SlotNo(1), BlockNo(1));
        chaindb.insert_block(block1).await.unwrap();

        let block2 = create_test_block(block1.header().hash(), SlotNo(2), BlockNo(2));
        chaindb.insert_block(block2).await.unwrap();

        // Verify chain has 3 blocks
        let tip = chaindb.get_tip().await;
        assert_eq!(tip.block_no(), BlockNo(2));

        // Rollback to genesis
        chaindb.rollback_to(ChainPoint::Genesis).await.unwrap();

        // Verify chain is empty
        let tip = chaindb.get_tip().await;
        assert_eq!(tip, ChainTip::genesis());

        // Verify blocks are removed
        for i in 0..=2 {
            let block = chaindb.get_block_by_height(BlockNo(i)).await;
            assert!(block.is_none());
        }
    }

    #[tokio::test]
    async fn test_chaindb_rollback_to_point() {
        let chaindb = MockChainDB::new();

        // Build a chain of 4 blocks
        let genesis = create_genesis_block();
        let genesis_hash = genesis.header().hash();
        chaindb.insert_block(genesis).await.unwrap();

        let block1 = create_test_block(genesis_hash, SlotNo(1), BlockNo(1));
        let block1_hash = block1.header().hash();
        chaindb.insert_block(block1.clone()).await.unwrap();

        let block2 = create_test_block(block1_hash, SlotNo(2), BlockNo(2));
        chaindb.insert_block(block2).await.unwrap();

        let block3 = create_test_block(block2.header().hash(), SlotNo(3), BlockNo(3));
        chaindb.insert_block(block3).await.unwrap();

        // Rollback to block1
        let rollback_point = ChainPoint::At {
            slot_no: block1.header().slot_no(),
            hash: block1_hash,
        };
        chaindb.rollback_to(rollback_point).await.unwrap();

        // Verify tip is now block1
        let tip = chaindb.get_tip().await;
        assert_eq!(tip.hash(), block1_hash);
        assert_eq!(tip.block_no(), BlockNo(1));

        // Verify blocks after rollback point are removed
        assert!(chaindb.get_block_by_height(BlockNo(0)).await.is_some()); // genesis
        assert!(chaindb.get_block_by_height(BlockNo(1)).await.is_some()); // block1
        assert!(chaindb.get_block_by_height(BlockNo(2)).await.is_none());  // block2 removed
        assert!(chaindb.get_block_by_height(BlockNo(3)).await.is_none());  // block3 removed
    }

    #[tokio::test]
    async fn test_chaindb_fork_handling() {
        let chaindb = MockChainDB::new();

        // Build main chain
        let genesis = create_genesis_block();
        let genesis_hash = genesis.header().hash();
        chaindb.insert_block(genesis).await.unwrap();

        let block1 = create_test_block(genesis_hash, SlotNo(1), BlockNo(1));
        let block1_hash = block1.header().hash();
        chaindb.insert_block(block1).await.unwrap();

        let main_block2 = create_test_block(block1_hash, SlotNo(2), BlockNo(2));
        chaindb.insert_block(main_block2).await.unwrap();

        // Create fork starting from block1
        let fork_block2 = create_test_block(block1_hash, SlotNo(3), BlockNo(2)); // Different slot, same height
        let fork_block3 = create_test_block(fork_block2.header().hash(), SlotNo(4), BlockNo(3));
        let fork_blocks = vec![fork_block2.clone(), fork_block3.clone()];

        // Switch to fork
        chaindb.switch_to_fork(&fork_blocks).await.unwrap();

        // Verify tip is now fork tip
        let tip = chaindb.get_tip().await;
        assert_eq!(tip.hash(), fork_block3.header().hash());
        assert_eq!(tip.block_no(), BlockNo(3));

        // Verify fork blocks are in chain
        let retrieved_block2 = chaindb.get_block_by_height(BlockNo(2)).await.unwrap();
        assert_eq!(retrieved_block2.header().hash(), fork_block2.header().hash());

        let retrieved_block3 = chaindb.get_block_by_height(BlockNo(3)).await.unwrap();
        assert_eq!(retrieved_block3.header().hash(), fork_block3.header().hash());
    }

    #[tokio::test]
    async fn test_chaindb_fork_no_common_ancestor() {
        let chaindb = MockChainDB::new();

        // Build main chain
        let genesis = create_genesis_block();
        let genesis_hash = genesis.header().hash();
        chaindb.insert_block(genesis).await.unwrap();

        let block1 = create_test_block(genesis_hash, SlotNo(1), BlockNo(1));
        chaindb.insert_block(block1).await.unwrap();

        // Create fork with no common ancestor
        let fake_previous = BlockHash::zero(); // Fake previous hash
        let orphan_block = create_test_block(fake_previous, SlotNo(5), BlockNo(5));
        let fork_blocks = vec![orphan_block];

        // Attempt to switch to fork should fail
        let result = chaindb.switch_to_fork(&fork_blocks).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ChainDBError::NoCommonAncestor));
    }

    #[tokio::test]
    async fn test_chaindb_concurrent_operations() {
        use std::sync::Arc;
        use tokio::task::JoinSet;

        let chaindb = Arc::new(MockChainDB::new());

        // Insert genesis block
        let genesis = create_genesis_block();
        let genesis_hash = genesis.header().hash();
        chaindb.insert_block(genesis).await.unwrap();

        let mut join_set = JoinSet::new();

        // Spawn concurrent block insertion tasks
        for i in 1..=10 {
            let chaindb_clone = Arc::clone(&chaindb);
            let previous_hash = if i == 1 { genesis_hash } else {
                // For simplicity, create a chain where each block references the previous
                BlockHash::from_bytes(&[i as u8 - 1; 32]) // Mock hash
            };

            join_set.spawn(async move {
                let block = create_test_block(previous_hash, SlotNo(i), BlockNo(i));
                chaindb_clone.insert_block(block).await
            });
        }

        // Wait for all insertions to complete
        let mut success_count = 0;
        while let Some(result) = join_set.join_next().await {
            if result.unwrap().is_ok() {
                success_count += 1;
            }
        }

        // At least some insertions should succeed
        assert!(success_count > 0);

        // Verify final state is consistent
        let tip = chaindb.get_tip().await;
        assert!(tip.block_no().0 > 0);
    }

    #[tokio::test]
    async fn test_chaindb_chain_navigation() {
        let chaindb = MockChainDB::new();

        // Build a chain of 5 blocks
        let mut blocks = Vec::new();
        let genesis = create_genesis_block();
        blocks.push(genesis.clone());
        chaindb.insert_block(genesis).await.unwrap();

        for i in 1..=4 {
            let previous_hash = blocks[i-1].header().hash();
            let block = create_test_block(previous_hash, SlotNo(i as u64), BlockNo(i as u64));
            blocks.push(block.clone());
            chaindb.insert_block(block).await.unwrap();
        }

        // Test forward navigation
        for i in 0..5 {
            let block = chaindb.get_block_by_height(BlockNo(i)).await;
            assert!(block.is_some());
            let block = block.unwrap();
            assert_eq!(block.header().block_no(), BlockNo(i));
            assert_eq!(block.header().hash(), blocks[i as usize].header().hash());
        }

        // Test backward navigation through previous hashes
        let mut current_block = blocks[4].clone(); // Start from tip
        for i in (0..4).rev() {
            let previous_hash = current_block.header().previous_hash();
            let previous_block = chaindb.get_block_by_hash(&previous_hash).await;
            assert!(previous_block.is_some());
            let previous_block = previous_block.unwrap();
            assert_eq!(previous_block.header().hash(), blocks[i].header().hash());
            current_block = previous_block;
        }
    }

    #[tokio::test]
    async fn test_chaindb_consistency_after_rollback() {
        let chaindb = MockChainDB::new();

        // Build initial chain
        let genesis = create_genesis_block();
        let genesis_hash = genesis.header().hash();
        chaindb.insert_block(genesis).await.unwrap();

        let block1 = create_test_block(genesis_hash, SlotNo(1), BlockNo(1));
        let block1_hash = block1.header().hash();
        chaindb.insert_block(block1.clone()).await.unwrap();

        let block2 = create_test_block(block1_hash, SlotNo(2), BlockNo(2));
        let block2_hash = block2.header().hash();
        chaindb.insert_block(block2.clone()).await.unwrap();

        // Rollback to block1
        let rollback_point = ChainPoint::At {
            slot_no: block1.header().slot_no(),
            hash: block1_hash,
        };
        chaindb.rollback_to(rollback_point).await.unwrap();

        // Verify consistency: tip should match the rollback point
        let tip = chaindb.get_tip().await;
        assert_eq!(tip.hash(), block1_hash);
        assert_eq!(tip.block_no(), BlockNo(1));

        // Verify height index consistency
        assert!(chaindb.get_block_by_height(BlockNo(0)).await.is_some()); // genesis
        assert!(chaindb.get_block_by_height(BlockNo(1)).await.is_some()); // block1
        assert!(chaindb.get_block_by_height(BlockNo(2)).await.is_none());  // block2 removed

        // Add new blocks after rollback
        let new_block2 = create_test_block(block1_hash, SlotNo(3), BlockNo(2));
        chaindb.insert_block(new_block2.clone()).await.unwrap();

        // Verify new chain state
        let tip = chaindb.get_tip().await;
        assert_eq!(tip.hash(), new_block2.header().hash());
        assert_eq!(tip.block_no(), BlockNo(2));

        // Verify we can retrieve the new block2
        let retrieved = chaindb.get_block_by_height(BlockNo(2)).await.unwrap();
        assert_eq!(retrieved.header().hash(), new_block2.header().hash());
        assert_ne!(retrieved.header().hash(), block2_hash); // Should be different from old block2
    }
}
