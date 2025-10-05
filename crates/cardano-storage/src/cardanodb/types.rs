//! Common types used throughout CardanoDB
//!
//! This module defines the core types used by all three databases (ImmutableDB,
//! VolatileDB, LedgerDB), leveraging types from cardano-base-rust where possible.

use serde::{Deserialize, Serialize};
use std::fmt;

// Re-export types from cardano-base-rust
pub use cardano_slotting::{BlockNo, EpochNo, SlotNo};

/// Chunk number for ImmutableDB
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ChunkNo(pub u64);

impl ChunkNo {
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    pub const fn to_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ChunkNo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Blake2b-256 hash (32 bytes)
///
/// We use a newtype wrapper for better type safety
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Blake2b256Hash([u8; 32]);

impl Blake2b256Hash {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        if slice.len() == 32 {
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(slice);
            Some(Self(bytes))
        } else {
            None
        }
    }

    /// Compute Blake2b-256 hash of data
    pub fn hash(data: &[u8]) -> Self {
        use blake2::digest::consts::U32;
        use blake2::{Blake2b, Digest};

        let mut hasher = Blake2b::<U32>::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&result);
        Self(bytes)
    }
}

impl fmt::Debug for Blake2b256Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Blake2b256Hash({})", hex::encode(self.0))
    }
}

impl fmt::Display for Blake2b256Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

/// Location of a block within a chunk file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockLocation {
    /// Byte offset in chunk file
    pub offset: u64,
    /// Block size in bytes
    pub size: u32,
}

impl BlockLocation {
    pub const fn new(offset: u64, size: u32) -> Self {
        Self { offset, size }
    }

    pub const fn end_offset(&self) -> u64 {
        self.offset + self.size as u64
    }
}

/// Tip of the immutable chain
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImmutableTip {
    /// Slot of the tip block
    pub slot_no: SlotNo,
    /// Hash of the tip block
    pub hash: Blake2b256Hash,
    /// Block number of the tip
    pub block_no: BlockNo,
}

/// Current tip of the chain (most recent block)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainTip {
    /// Hash of the tip block
    pub block_hash: Blake2b256Hash,
    /// Block number of the tip
    pub block_no: BlockNo,
    /// Slot number of the tip block
    pub slot_no: SlotNo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_no_ordering() {
        assert!(ChunkNo(0) < ChunkNo(1));
        assert!(ChunkNo(100) > ChunkNo(50));
    }

    #[test]
    fn blake2b_hash_from_slice() {
        let bytes = [42u8; 32];
        let hash = Blake2b256Hash::from_slice(&bytes).unwrap();
        assert_eq!(hash.as_bytes(), &bytes);
    }

    #[test]
    fn blake2b_hash_computation() {
        let data = b"hello world";
        let hash = Blake2b256Hash::hash(data);
        // Verify it's a valid 32-byte hash
        assert_eq!(hash.as_bytes().len(), 32);
    }

    #[test]
    fn block_location_end_offset() {
        let loc = BlockLocation::new(1000, 500);
        assert_eq!(loc.end_offset(), 1500);
    }
}
