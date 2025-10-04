//! Primary and secondary indices for fast block lookup
//!
//! - Primary index: SlotNo -> BlockLocation (for slot-based lookup)
//! - Secondary index: Blake2b256Hash -> SlotNo (for hash-based lookup)

use crate::cardanodb::types::{Blake2b256Hash, BlockLocation, SlotNo};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// Magic bytes for primary index files: "CARDANO_PRIMARY"
pub const PRIMARY_INDEX_MAGIC: &[u8; 15] = b"CARDANO_PRIMARY";

/// Magic bytes for secondary index files: "CARDANO_SECONDARY"
pub const SECONDARY_INDEX_MAGIC: &[u8; 17] = b"CARDANO_SECONDARY";

/// Index file version
pub const INDEX_VERSION: u32 = 1;

/// Primary index: slot -> block location mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimaryIndex {
    /// Map: SlotNo -> (offset, size)
    /// BTreeMap provides sorted access (important for range queries)
    entries: BTreeMap<SlotNo, BlockLocation>,
}

impl PrimaryIndex {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Insert a new entry
    pub fn insert(&mut self, slot_no: SlotNo, location: BlockLocation) {
        self.entries.insert(slot_no, location);
    }

    /// Get block location by slot number
    pub fn get(&self, slot_no: &SlotNo) -> Option<&BlockLocation> {
        self.entries.get(slot_no)
    }

    /// Number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all entries
    pub fn iter(&self) -> impl Iterator<Item = (&SlotNo, &BlockLocation)> {
        self.entries.iter()
    }

    /// Save index to disk using JSON encoding
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_vec(self).context("Failed to serialize primary index")?;
        std::fs::write(path, json).context("Failed to write primary index")?;
        Ok(())
    }

    /// Load index from disk
    pub fn load(path: &Path) -> Result<Self> {
        let data = std::fs::read(path).context("Failed to read primary index")?;
        let index: Self =
            serde_json::from_slice(&data).context("Failed to deserialize primary index")?;
        Ok(index)
    }
}

impl Default for PrimaryIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Secondary index: hash -> slot mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecondaryIndex {
    /// Map: Blake2b256Hash -> SlotNo
    /// HashMap provides O(1) hash-based lookup
    #[serde(
        serialize_with = "serialize_hash_map",
        deserialize_with = "deserialize_hash_map"
    )]
    entries: HashMap<Blake2b256Hash, SlotNo>,
}

// Helper functions for serializing HashMap with non-string keys
fn serialize_hash_map<S>(
    map: &HashMap<Blake2b256Hash, SlotNo>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize;
    let vec: Vec<_> = map.iter().collect();
    vec.serialize(serializer)
}

fn deserialize_hash_map<'de, D>(
    deserializer: D,
) -> Result<HashMap<Blake2b256Hash, SlotNo>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let vec: Vec<(Blake2b256Hash, SlotNo)> = Vec::deserialize(deserializer)?;
    Ok(vec.into_iter().collect())
}

impl SecondaryIndex {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Insert a new entry
    pub fn insert(&mut self, hash: Blake2b256Hash, slot_no: SlotNo) {
        self.entries.insert(hash, slot_no);
    }

    /// Get slot number by block hash
    pub fn get(&self, hash: &Blake2b256Hash) -> Option<&SlotNo> {
        self.entries.get(hash)
    }

    /// Number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all entries
    pub fn iter(&self) -> impl Iterator<Item = (&Blake2b256Hash, &SlotNo)> {
        self.entries.iter()
    }

    /// Save index to disk using JSON encoding
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_vec(self).context("Failed to serialize secondary index")?;
        std::fs::write(path, json).context("Failed to write secondary index")?;
        Ok(())
    }

    /// Load index from disk
    pub fn load(path: &Path) -> Result<Self> {
        let data = std::fs::read(path).context("Failed to read secondary index")?;
        let index: Self =
            serde_json::from_slice(&data).context("Failed to deserialize secondary index")?;
        Ok(index)
    }
}

impl Default for SecondaryIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_index_insert_and_get() {
        let mut index = PrimaryIndex::new();
        let slot = SlotNo(100);
        let location = BlockLocation::new(1000, 500);

        index.insert(slot, location);
        assert_eq!(index.get(&slot), Some(&location));
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn secondary_index_insert_and_get() {
        let mut index = SecondaryIndex::new();
        let hash = Blake2b256Hash::new([42u8; 32]);
        let slot = SlotNo(200);

        index.insert(hash, slot);
        assert_eq!(index.get(&hash), Some(&slot));
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn index_serialization_roundtrip() {
        let mut primary = PrimaryIndex::new();
        primary.insert(SlotNo(1), BlockLocation::new(0, 100));
        primary.insert(SlotNo(2), BlockLocation::new(100, 200));

        let bytes = serde_json::to_vec(&primary).unwrap();
        let decoded: PrimaryIndex = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded.get(&SlotNo(1)), primary.get(&SlotNo(1)));
    }
}
