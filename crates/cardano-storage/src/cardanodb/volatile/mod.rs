//! VolatileDB - Fast storage for recent blocks
//!
//! This module implements the VolatileDB component using a ring buffer for
//! efficient storage of the most recent k blocks.

pub mod ring_buffer;

use crate::cardanodb::{
    config::VolatileDBConfig,
    types::{Blake2b256Hash, BlockNo, SlotNo},
};
use anyhow::Result;
use hashbrown::HashMap;
use std::collections::BTreeMap;
use std::time::Instant;
use tokio::sync::RwLock;

pub use ring_buffer::RingBuffer;

/// VolatileDB: Fast storage for recent blocks
pub struct VolatileDB {
    /// Ring buffer for recent blocks (last k blocks)
    blocks: RwLock<RingBuffer<VolatileBlock>>,

    /// Hash -> Index mapping for fast lookup
    hash_index: RwLock<HashMap<Blake2b256Hash, usize>>,

    /// Slot -> Index mapping for fast lookup
    slot_index: RwLock<BTreeMap<SlotNo, usize>>,

    /// Maximum blocks to keep (security parameter k)
    k: u64,
}

/// A block stored in VolatileDB
#[derive(Debug, Clone)]
pub struct VolatileBlock {
    /// Block hash
    pub hash: Blake2b256Hash,

    /// Slot number
    pub slot_no: SlotNo,

    /// Block number
    pub block_no: BlockNo,

    /// Raw block data (CBOR-encoded)
    pub data: Vec<u8>,

    /// Timestamp when added
    pub added_at: Instant,
}

impl VolatileDB {
    /// Create a new VolatileDB with capacity k
    pub fn new(config: VolatileDBConfig) -> Self {
        Self {
            blocks: RwLock::new(RingBuffer::with_capacity(config.k as usize)),
            hash_index: RwLock::new(HashMap::new()),
            slot_index: RwLock::new(BTreeMap::new()),
            k: config.k,
        }
    }

    /// Add a block to VolatileDB
    pub async fn add_block(
        &self,
        hash: Blake2b256Hash,
        slot_no: SlotNo,
        block_no: BlockNo,
        data: Vec<u8>,
    ) -> Result<()> {
        let block = VolatileBlock {
            hash,
            slot_no,
            block_no,
            data,
            added_at: Instant::now(),
        };

        let mut blocks = self.blocks.write().await;

        // If buffer is full, remove oldest block
        if let Some(removed) = blocks.push(block.clone()) {
            // Update indices - remove old entry
            let mut hash_index = self.hash_index.write().await;
            let mut slot_index = self.slot_index.write().await;

            hash_index.remove(&removed.hash);
            slot_index.remove(&removed.slot_no);
        }

        // Add to indices
        let index = blocks.len().saturating_sub(1);
        self.hash_index.write().await.insert(hash, index);
        self.slot_index.write().await.insert(slot_no, index);

        Ok(())
    }

    /// Get a block by hash
    pub async fn get_block(&self, hash: &Blake2b256Hash) -> Option<Vec<u8>> {
        let hash_index = self.hash_index.read().await;
        let blocks = self.blocks.read().await;

        if let Some(&index) = hash_index.get(hash) {
            if let Some(block) = blocks.get(index) {
                return Some(block.data.clone());
            }
        }

        None
    }

    /// Get a block by slot number
    pub async fn get_block_by_slot(&self, slot_no: &SlotNo) -> Option<Vec<u8>> {
        let slot_index = self.slot_index.read().await;
        let blocks = self.blocks.read().await;

        if let Some(&index) = slot_index.get(slot_no) {
            if let Some(block) = blocks.get(index) {
                return Some(block.data.clone());
            }
        }

        None
    }

    /// Get blocks ready for garbage collection (move to ImmutableDB)
    ///
    /// Returns blocks older than k that should be moved to immutable storage
    pub async fn get_gc_candidates(&self) -> Vec<VolatileBlock> {
        let blocks = self.blocks.read().await;

        // Return blocks that exceed our k capacity
        // These are the oldest blocks in the ring buffer
        if blocks.len() > self.k as usize {
            blocks
                .iter()
                .take(blocks.len() - self.k as usize)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all blocks (for migration processing)
    pub async fn get_all_blocks(&self) -> Vec<VolatileBlock> {
        let blocks = self.blocks.read().await;
        blocks.iter().cloned().collect()
    }

    /// Remove a block (after moving to ImmutableDB)
    pub async fn remove_block(&self, hash: &Blake2b256Hash) -> Result<()> {
        let mut hash_index = self.hash_index.write().await;
        let mut slot_index = self.slot_index.write().await;
        let mut blocks = self.blocks.write().await;

        if let Some(&index) = hash_index.get(hash) {
            if let Some(block) = blocks.remove(index) {
                hash_index.remove(&block.hash);
                slot_index.remove(&block.slot_no);
            }
        }

        Ok(())
    }

    /// Get the current number of blocks in VolatileDB
    pub async fn block_count(&self) -> usize {
        self.blocks.read().await.len()
    }

    /// Get the tip of the volatile chain (most recent block)
    pub async fn get_tip(&self) -> Option<crate::cardanodb::types::ChainTip> {
        let blocks = self.blocks.read().await;

        if blocks.is_empty() {
            return None;
        }

        // Get the most recent block (last in ring buffer: len - 1)
        blocks
            .get(blocks.len() - 1)
            .map(|block| crate::cardanodb::types::ChainTip {
                block_hash: block.hash,
                block_no: block.block_no,
                slot_no: block.slot_no,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn volatile_db_stores_and_retrieves_blocks() {
        let config = VolatileDBConfig { k: 10 };
        let db = VolatileDB::new(config);

        let hash = Blake2b256Hash::new([1u8; 32]);
        let slot = SlotNo(100);
        let block_no = BlockNo(50);
        let data = vec![1, 2, 3, 4, 5];

        db.add_block(hash, slot, block_no, data.clone())
            .await
            .unwrap();

        let retrieved = db.get_block(&hash).await.unwrap();
        assert_eq!(retrieved, data);

        let retrieved_by_slot = db.get_block_by_slot(&slot).await.unwrap();
        assert_eq!(retrieved_by_slot, data);
    }

    #[tokio::test]
    async fn volatile_db_evicts_old_blocks() {
        let config = VolatileDBConfig { k: 3 };
        let db = VolatileDB::new(config);

        // Add 5 blocks (capacity is 3)
        for i in 0..5 {
            let hash = Blake2b256Hash::new([i as u8; 32]);
            let slot = SlotNo(i as u64);
            let block_no = BlockNo(i as u64);
            let data = vec![i as u8];

            db.add_block(hash, slot, block_no, data).await.unwrap();
        }

        // Should only have 3 blocks (the most recent ones)
        assert_eq!(db.block_count().await, 3);

        // First two blocks should be gone
        let hash0 = Blake2b256Hash::new([0u8; 32]);
        assert!(db.get_block(&hash0).await.is_none());

        // Last three blocks should still be there
        let hash4 = Blake2b256Hash::new([4u8; 32]);
        assert!(db.get_block(&hash4).await.is_some());
    }
}
