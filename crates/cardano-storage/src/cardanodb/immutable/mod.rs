//! ImmutableDB - Chunk-based storage for ancient, immutable blocks
//!
//! This module implements the ImmutableDB component of CardanoDB, which stores
//! blocks in chunk files with primary and secondary indices for fast lookup.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │                  ImmutableDB                         │
//! │  ┌──────────────┐  ┌──────────────┐  ┌───────────┐ │
//! │  │ PrimaryIndex │  │SecondaryIndex│  │   Tip     │ │
//! │  │  Slot→Loc    │  │   Hash→Slot  │  │ (Latest)  │ │
//! │  └──────────────┘  └──────────────┘  └───────────┘ │
//! │           │                │               │         │
//! │           └────────────────┴───────────────┘         │
//! │                        ▼                             │
//! │           ┌──────────────────────────┐               │
//! │           │     Chunk Cache          │               │
//! │           │  HashMap<ChunkNo, Handle>│               │
//! │           └──────────────────────────┘               │
//! │                        ▼                             │
//! │  ┌──────────────────────────────────────────────┐   │
//! │  │           ChunkFile (mmap)                   │   │
//! │  │  ┌─────────────────────────────────────┐    │   │
//! │  │  │ Header (29 bytes)                   │    │   │
//! │  │  ├─────────────────────────────────────┤    │   │
//! │  │  │ Block 1 (offset=29, size=X)         │◄───┼───┼── Zero-copy read
//! │  │  ├─────────────────────────────────────┤    │   │
//! │  │  │ Block 2 (offset=29+X, size=Y)       │    │   │
//! │  │  ├─────────────────────────────────────┤    │   │
//! │  │  │ Block 3 ...                         │    │   │
//! │  │  └─────────────────────────────────────┘    │   │
//! │  └──────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! # Storage Layout
//!
//! ```text
//! immutable_db/
//! ├── chunk_00000000.dat       # Chunk 0 (slots 0-999)
//! ├── chunk_00000001.dat       # Chunk 1 (slots 1000-1999)
//! ├── primary_index.bin        # Slot → Location map (JSON)
//! ├── secondary_index.bin      # Hash → Slot map (JSON)
//! └── tip.json                 # Latest block metadata
//! ```
//!
//! # Usage Example
//!
//! ```no_run
//! use cardano_storage::cardanodb::immutable::ImmutableDB;
//! use cardano_storage::cardanodb::config::ImmutableDBConfig;
//! use cardano_storage::cardanodb::types::{Blake2b256Hash, BlockNo, SlotNo};
//! use std::path::PathBuf;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Open database
//! let config = ImmutableDBConfig {
//!     path: PathBuf::from("/var/lib/cardano/immutable"),
//!     chunk_size: 1000,
//!     enable_compression: false,
//!     max_cached_chunks: 100,
//! };
//! let db = ImmutableDB::open(config)?;
//!
//! // Store a block
//! let slot = SlotNo(12345);
//! let block_no = BlockNo(100);
//! let block_data = b"block data...";
//! let hash = Blake2b256Hash::hash(block_data);
//!
//! db.store_block(slot, block_no, hash, block_data).await?;
//!
//! // Retrieve by slot
//! let block = db.get_block_by_slot(slot).await?;
//! assert_eq!(block.as_deref(), Some(block_data.as_slice()));
//!
//! // Retrieve by hash
//! let block = db.get_block_by_hash(&hash).await?;
//! assert_eq!(block.as_deref(), Some(block_data.as_slice()));
//!
//! // Finalize and persist
//! db.finalize_current_chunk().await?;
//! db.save_indices().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Performance Characteristics
//!
//! - **Reads**: Zero-copy via memory-mapped I/O, O(log N) slot lookup, O(1) hash lookup
//! - **Writes**: Sequential append, buffered by OS, automatic chunk rotation
//! - **Memory**: Indices kept in RAM, chunks memory-mapped on-demand
//! - **Concurrency**: Multiple concurrent readers supported, single writer
//!
//! # Error Handling
//!
//! All public methods return `Result<T>` with detailed error context. Common errors:
//! - `Block data cannot be empty` - Attempted to store empty block
//! - `Block data too large` - Exceeded 16 MB limit
//! - `Chunk file does not exist` - Attempted to read non-existent chunk
//! - `Failed to serialize indices` - Persistence failure
//!
//! # Thread Safety
//!
//! `ImmutableDB` is `Send + Sync` and can be safely shared across threads using `Arc`.
//! Internal synchronization is handled via `RwLock` for concurrent reads.

pub mod chunk;
pub mod index;

use crate::cardanodb::{
    config::ImmutableDBConfig,
    types::{Blake2b256Hash, BlockNo, ChunkNo, ImmutableTip, SlotNo},
};
use anyhow::{Context, Result};
use chunk::ChunkFile;
use hashbrown::HashMap;
use index::{PrimaryIndex, SecondaryIndex};

// Re-export chunk abstractions for external use
pub use chunk::{ChunkReader, ChunkWriter};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// ImmutableDB: Chunk-based storage for ancient blocks
pub struct ImmutableDB {
    /// Base directory for chunks
    base_path: PathBuf,

    /// Configuration
    config: ImmutableDBConfig,

    /// Open chunk files (mmap for fast reads)
    chunks: RwLock<HashMap<ChunkNo, ChunkHandle>>,

    /// LRU tracking for cache eviction (most recently used at back)
    chunk_lru: RwLock<VecDeque<ChunkNo>>,

    /// Primary index (slot -> location)
    primary_index: Arc<RwLock<PrimaryIndex>>,

    /// Secondary index (hash -> slot)
    secondary_index: Arc<RwLock<SecondaryIndex>>,

    /// Current tip of immutable chain
    tip: RwLock<Option<ImmutableTip>>,

    /// Current writable chunk for appending blocks
    current_chunk: RwLock<Option<CurrentChunk>>,
}

impl Drop for ImmutableDB {
    fn drop(&mut self) {
        // Note: Cannot use async in Drop, so we just log
        // Users should call finalize_current_chunk() before dropping
        debug!("ImmutableDB dropped - ensure finalize_current_chunk() was called before drop");
    }
}

/// Handle to an open chunk file
struct ChunkHandle {
    /// Memory-mapped chunk file
    chunk_file: Arc<ChunkFile>,
    /// Reference count for tracking active readers (for safe eviction)
    #[allow(dead_code)]
    ref_count: usize,
}

/// Current chunk being written to
struct CurrentChunk {
    chunk_no: ChunkNo,
    chunk_file: ChunkFile,
    block_count: u32,
}

impl ImmutableDB {
    /// Open/create ImmutableDB at the given path
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created or accessed.
    pub fn open(config: ImmutableDBConfig) -> Result<Self> {
        // Create chunk directory
        std::fs::create_dir_all(&config.path).context("Failed to create ImmutableDB directory")?;

        info!(
            path = %config.path.display(),
            chunk_size = config.chunk_size,
            "Opening ImmutableDB"
        );

        Ok(Self {
            base_path: config.path.clone(),
            config,
            chunks: RwLock::new(HashMap::new()),
            chunk_lru: RwLock::new(VecDeque::new()),
            primary_index: Arc::new(RwLock::new(PrimaryIndex::new())),
            secondary_index: Arc::new(RwLock::new(SecondaryIndex::new())),
            tip: RwLock::new(None),
            current_chunk: RwLock::new(None),
        })
    }

    /// Get chunk file path
    fn chunk_path(&self, chunk_no: ChunkNo) -> PathBuf {
        self.base_path
            .join(format!("chunk_{:08}.dat", chunk_no.to_u64()))
    }

    /// Get primary index file path
    fn primary_index_path(&self) -> PathBuf {
        self.base_path.join("primary_index.bin")
    }

    /// Get secondary index file path
    fn secondary_index_path(&self) -> PathBuf {
        self.base_path.join("secondary_index.bin")
    }

    /// Get tip file path
    fn tip_path(&self) -> PathBuf {
        self.base_path.join("tip.json")
    }

    /// Open or create a chunk file
    ///
    /// This method implements chunk caching: if the chunk is already open,
    /// it returns the cached version. Otherwise, it opens the file and caches it.
    async fn get_chunk(&self, chunk_no: ChunkNo) -> Result<Arc<ChunkFile>> {
        // Check if already open
        {
            let chunks = self.chunks.read().await;
            if let Some(handle) = chunks.get(&chunk_no) {
                debug!(chunk_no = chunk_no.0, "Cache hit for chunk");
                // Update LRU: move to back (most recently used)
                let mut lru = self.chunk_lru.write().await;
                lru.retain(|&c| c != chunk_no);
                lru.push_back(chunk_no);
                return Ok(handle.chunk_file.clone());
            }
        }

        debug!(chunk_no = chunk_no.0, "Opening chunk file");

        // Open the chunk
        let path = self.chunk_path(chunk_no);
        let chunk_file = if path.exists() {
            ChunkFile::open(&path)
                .with_context(|| format!("Failed to open chunk {}", chunk_no.0))?
        } else {
            return Err(anyhow::anyhow!(
                "Chunk file does not exist: {}",
                path.display()
            ));
        };

        let chunk_file = Arc::new(chunk_file);

        // Store in cache and update LRU
        {
            let mut chunks = self.chunks.write().await;
            let mut lru = self.chunk_lru.write().await;

            // Check cache limit and evict if necessary
            if self.config.max_cached_chunks > 0 && chunks.len() >= self.config.max_cached_chunks {
                // Evict least recently used chunk
                if let Some(evict_chunk_no) = lru.pop_front() {
                    if chunks.remove(&evict_chunk_no).is_some() {
                        debug!(
                            evicted_chunk = evict_chunk_no.0,
                            cache_size = chunks.len(),
                            "Evicted chunk from cache (LRU)"
                        );
                    }
                }
            }

            chunks.insert(
                chunk_no,
                ChunkHandle {
                    chunk_file: chunk_file.clone(),
                    ref_count: 1,
                },
            );
            lru.push_back(chunk_no);
        }

        Ok(chunk_file)
    }

    /// Get the current tip of the immutable chain
    ///
    /// Returns the metadata of the most recently stored block, or `None` if no blocks
    /// have been stored yet.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use cardano_storage::cardanodb::immutable::ImmutableDB;
    /// # use cardano_storage::cardanodb::config::ImmutableDBConfig;
    /// # async fn example(db: &ImmutableDB) {
    /// if let Some(tip) = db.get_tip().await {
    ///     println!("Latest block: slot={}, block={}", tip.slot_no.0, tip.block_no.0);
    /// }
    /// # }
    /// ```
    pub async fn get_tip(&self) -> Option<ImmutableTip> {
        self.tip.read().await.clone()
    }

    /// Get a block by slot number
    ///
    /// Performs a primary index lookup followed by a zero-copy read from the chunk file.
    ///
    /// # Arguments
    ///
    /// * `slot` - The slot number to look up
    ///
    /// # Returns
    ///
    /// - `Ok(Some(data))` if the block exists
    /// - `Ok(None)` if no block exists at this slot
    /// - `Err(...)` if the chunk file is corrupted or inaccessible
    ///
    /// # Performance
    ///
    /// - Index lookup: O(log N) via BTreeMap
    /// - Chunk access: O(1) if cached, one file open if not
    /// - Block read: Zero-copy via memory-mapped I/O
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use cardano_storage::cardanodb::immutable::ImmutableDB;
    /// # use cardano_storage::cardanodb::types::SlotNo;
    /// # async fn example(db: &ImmutableDB) -> anyhow::Result<()> {
    /// let slot = SlotNo(12345);
    /// if let Some(block_data) = db.get_block_by_slot(slot).await? {
    ///     println!("Found block at slot {}: {} bytes", slot.0, block_data.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_block_by_slot(&self, slot: SlotNo) -> Result<Option<Vec<u8>>> {
        // Look up in primary index
        let primary = self.primary_index.read().await;
        let location = match primary.get(&slot) {
            Some(loc) => *loc,
            None => return Ok(None),
        };
        drop(primary);

        // Calculate chunk number (simple scheme: slot / chunk_size)
        let chunk_no = ChunkNo(slot.0 / self.config.chunk_size as u64);

        // Get chunk and read block
        let chunk = self.get_chunk(chunk_no).await?;
        let block_data = chunk.read_block(&location)?;

        Ok(Some(block_data.to_vec()))
    }

    /// Get a block by hash
    ///
    /// Performs a secondary index lookup to find the slot, then retrieves the block
    /// via the primary index and chunk file.
    ///
    /// # Arguments
    ///
    /// * `hash` - The Blake2b-256 hash of the block
    ///
    /// # Returns
    ///
    /// - `Ok(Some(data))` if the block exists
    /// - `Ok(None)` if no block with this hash exists
    /// - `Err(...)` if the chunk file is corrupted or inaccessible
    ///
    /// # Performance
    ///
    /// - Hash lookup: O(1) via HashMap
    /// - Slot lookup: O(log N) via BTreeMap
    /// - Chunk access: O(1) if cached
    /// - Block read: Zero-copy via memory-mapped I/O
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use cardano_storage::cardanodb::immutable::ImmutableDB;
    /// # use cardano_storage::cardanodb::types::Blake2b256Hash;
    /// # async fn example(db: &ImmutableDB, hash: Blake2b256Hash) -> anyhow::Result<()> {
    /// if let Some(block_data) = db.get_block_by_hash(&hash).await? {
    ///     println!("Found block by hash: {} bytes", block_data.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_block_by_hash(&self, hash: &Blake2b256Hash) -> Result<Option<Vec<u8>>> {
        // Look up in secondary index
        let secondary = self.secondary_index.read().await;
        let slot = match secondary.get(hash) {
            Some(&slot) => slot,
            None => return Ok(None),
        };
        drop(secondary);

        // Use slot-based lookup
        self.get_block_by_slot(slot).await
    }

    /// Store a block in the immutable database
    ///
    /// This writes the block to the current chunk, updates indices, and maintains the tip.
    ///
    /// # Arguments
    ///
    /// * `slot` - The slot number of the block
    /// * `block_no` - The block number
    /// * `hash` - The Blake2b-256 hash of the block
    /// * `block_data` - The raw block data
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Block data is empty
    /// - Block data exceeds maximum size (16 MB)
    /// - Chunk file cannot be created or written to
    /// - Indices cannot be updated
    ///
    /// # Performance
    ///
    /// This method acquires multiple write locks and performs I/O operations.
    /// It should not be called from hot paths in parallel.
    pub async fn store_block(
        &self,
        slot: SlotNo,
        block_no: BlockNo,
        hash: Blake2b256Hash,
        block_data: &[u8],
    ) -> Result<()> {
        // Validate inputs
        if block_data.is_empty() {
            return Err(anyhow::anyhow!("Block data cannot be empty"));
        }
        const MAX_BLOCK_SIZE: usize = 16 * 1024 * 1024; // 16 MB
        if block_data.len() > MAX_BLOCK_SIZE {
            return Err(anyhow::anyhow!(
                "Block data too large: {} bytes (max {})",
                block_data.len(),
                MAX_BLOCK_SIZE
            ));
        }

        debug!(
            slot = slot.0,
            block_no = block_no.0,
            size = block_data.len(),
            "Storing block"
        );

        // Calculate which chunk this block belongs to
        let chunk_no = ChunkNo(slot.0 / self.config.chunk_size as u64);

        // Get or create the current chunk
        let location = {
            let mut current = self.current_chunk.write().await;

            // Check if we need a new chunk
            let needs_new_chunk = match current.as_ref() {
                None => true,
                Some(c) => c.chunk_no != chunk_no || c.block_count >= self.config.chunk_size,
            };

            if needs_new_chunk {
                if let Some(old_chunk) = current.as_ref() {
                    info!(
                        chunk_no = old_chunk.chunk_no.0,
                        block_count = old_chunk.block_count,
                        "Finalizing full chunk"
                    );
                }

                // Finalize old chunk if any
                if let Some(old_chunk) = current.take() {
                    self.finalize_chunk_internal(old_chunk).await?;
                }

                // Create new chunk
                let path = self.chunk_path(chunk_no);
                let chunk_file = ChunkFile::create(&path, chunk_no)
                    .with_context(|| format!("Failed to create chunk {}", chunk_no.0))?;

                info!(chunk_no = chunk_no.0, "Created new chunk");

                *current = Some(CurrentChunk {
                    chunk_no,
                    chunk_file,
                    block_count: 0,
                });
            }

            // Write block to current chunk
            let current_chunk = current
                .as_mut()
                .expect("Current chunk must exist after creation check");
            let location = current_chunk
                .chunk_file
                .write_block(block_data)
                .context("Failed to write block to chunk")?;
            current_chunk.block_count += 1;

            location
        };

        // Update indices
        {
            let mut primary = self.primary_index.write().await;
            primary.insert(slot, location);
        }

        {
            let mut secondary = self.secondary_index.write().await;
            secondary.insert(hash, slot);
        }

        // Update tip
        {
            let mut tip = self.tip.write().await;
            *tip = Some(ImmutableTip {
                slot_no: slot,
                hash,
                block_no,
            });
        }

        debug!(slot = slot.0, "Block stored successfully");
        Ok(())
    }

    /// Finalize the current chunk (flush and prepare for reading)
    ///
    /// # Errors
    ///
    /// Returns an error if the chunk cannot be finalized or cached.
    pub async fn finalize_current_chunk(&self) -> Result<()> {
        let mut current = self.current_chunk.write().await;

        if let Some(chunk) = current.take() {
            info!(
                chunk_no = chunk.chunk_no.0,
                block_count = chunk.block_count,
                "Finalizing current chunk"
            );
            self.finalize_chunk_internal(chunk).await?;
        } else {
            debug!("No current chunk to finalize");
        }

        Ok(())
    }

    /// Internal method to finalize a chunk
    async fn finalize_chunk_internal(&self, chunk: CurrentChunk) -> Result<()> {
        let chunk_no = chunk.chunk_no;

        // Finalize the chunk file (flush and close for writing)
        let finalized = chunk
            .chunk_file
            .finalize()
            .with_context(|| format!("Failed to finalize chunk {}", chunk_no.0))?;

        // Add to chunk cache for reading
        let chunk_file = Arc::new(finalized);
        let mut chunks = self.chunks.write().await;
        let mut lru = self.chunk_lru.write().await;

        // Evict if cache is full
        if self.config.max_cached_chunks > 0 && chunks.len() >= self.config.max_cached_chunks {
            if let Some(evict_chunk_no) = lru.pop_front() {
                if evict_chunk_no != chunk_no && chunks.remove(&evict_chunk_no).is_some() {
                    debug!(evicted_chunk = evict_chunk_no.0, "Evicted chunk from cache");
                }
            }
        }

        chunks.insert(
            chunk_no,
            ChunkHandle {
                chunk_file: chunk_file.clone(),
                ref_count: 1,
            },
        );
        lru.push_back(chunk_no);

        debug!(chunk_no = chunk_no.0, "Chunk finalized and cached");
        Ok(())
    }

    /// Save indices and tip to disk with atomic updates
    ///
    /// This ensures crash consistency by writing to temporary files first,
    /// then atomically renaming them. Safe to call periodically for checkpointing.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Indices cannot be serialized
    /// - Files cannot be written (disk full, permissions)
    /// - Atomic rename fails
    ///
    /// # Atomicity
    ///
    /// Uses the POSIX atomic rename guarantee: the rename operation either succeeds
    /// completely or fails completely. Partial updates are impossible.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use cardano_storage::cardanodb::immutable::ImmutableDB;
    /// # async fn example(db: &ImmutableDB) -> anyhow::Result<()> {
    /// // Store some blocks...
    ///
    /// // Checkpoint to disk
    /// db.save_indices().await?;
    /// println!("Indices saved successfully");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn save_indices(&self) -> Result<()> {
        // Save primary index
        {
            let primary = self.primary_index.read().await;
            let tmp_path = self.primary_index_path().with_extension("tmp");
            primary.save(&tmp_path)?;
            std::fs::rename(&tmp_path, self.primary_index_path())?;
        }

        // Save secondary index
        {
            let secondary = self.secondary_index.read().await;
            let tmp_path = self.secondary_index_path().with_extension("tmp");
            secondary.save(&tmp_path)?;
            std::fs::rename(&tmp_path, self.secondary_index_path())?;
        }

        // Save tip
        {
            let tip = self.tip.read().await;
            if let Some(tip_data) = tip.as_ref() {
                let tmp_path = self.tip_path().with_extension("tmp");
                let json = serde_json::to_string_pretty(tip_data)?;
                std::fs::write(&tmp_path, json)?;
                std::fs::rename(&tmp_path, self.tip_path())?;
            }
        }

        Ok(())
    }

    /// Load indices and tip from disk (called during open)
    ///
    /// Restores the database state from a previous checkpoint. Should be called
    /// after `open()` to recover from a restart or crash.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Index files are corrupted
    /// - Files cannot be read
    /// - Deserialization fails
    ///
    /// # Behavior
    ///
    /// - If index files don't exist, indices remain empty (new database)
    /// - If files exist but are corrupted, returns an error
    /// - Partial recovery is not supported (all-or-nothing)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use cardano_storage::cardanodb::immutable::ImmutableDB;
    /// # use cardano_storage::cardanodb::config::ImmutableDBConfig;
    /// # use std::path::PathBuf;
    /// # async fn example() -> anyhow::Result<()> {
    /// let config = ImmutableDBConfig {
    ///     path: PathBuf::from("/var/lib/cardano/immutable"),
    ///     chunk_size: 1000,
    ///     enable_compression: false,
    ///     max_cached_chunks: 100,
    /// };
    ///
    /// let db = ImmutableDB::open(config)?;
    /// db.load_indices().await?;  // Restore from disk
    ///
    /// if let Some(tip) = db.get_tip().await {
    ///     println!("Recovered database at slot {}", tip.slot_no.0);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn load_indices(&self) -> Result<()> {
        // Load primary index if exists
        let primary_path = self.primary_index_path();
        if primary_path.exists() {
            let mut primary = self.primary_index.write().await;
            *primary = PrimaryIndex::load(&primary_path)?;
        }

        // Load secondary index if exists
        let secondary_path = self.secondary_index_path();
        if secondary_path.exists() {
            let mut secondary = self.secondary_index.write().await;
            *secondary = SecondaryIndex::load(&secondary_path)?;
        }

        // Load tip if exists
        let tip_path = self.tip_path();
        if tip_path.exists() {
            let json = std::fs::read_to_string(&tip_path)?;
            let tip_data: ImmutableTip = serde_json::from_str(&json)?;
            let mut tip = self.tip.write().await;
            *tip = Some(tip_data);
        }

        Ok(())
    }

    /// Get the number of blocks in the primary index
    ///
    /// Useful for monitoring and debugging.
    pub async fn block_count(&self) -> usize {
        self.primary_index.read().await.len()
    }

    /// Get the number of cached chunks
    ///
    /// Useful for monitoring memory usage.
    pub async fn cached_chunk_count(&self) -> usize {
        self.chunks.read().await.len()
    }

    /// Clear the chunk cache
    ///
    /// This releases memory-mapped files from the cache. They will be reopened
    /// on next access. Useful for controlling memory usage.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use cardano_storage::cardanodb::immutable::ImmutableDB;
    /// # async fn example(db: &ImmutableDB) {
    /// // After processing a large batch
    /// db.clear_chunk_cache().await;
    /// println!("Chunk cache cleared");
    /// # }
    /// ```
    pub async fn clear_chunk_cache(&self) {
        let mut chunks = self.chunks.write().await;
        let count = chunks.len();
        chunks.clear();
        debug!(cleared_chunks = count, "Chunk cache cleared");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn immutable_db_can_be_opened() {
        let dir = tempdir().unwrap();
        let config = ImmutableDBConfig {
            path: dir.path().to_path_buf(),
            chunk_size: 1000,
            max_cached_chunks: 10,
            enable_compression: false,
        };

        let db = ImmutableDB::open(config).unwrap();
        assert!(db.get_tip().await.is_none());
    }

    #[tokio::test]
    async fn immutable_db_store_and_retrieve_block() {
        let dir = tempdir().unwrap();
        let config = ImmutableDBConfig {
            path: dir.path().to_path_buf(),
            chunk_size: 1000,
            max_cached_chunks: 10,
            enable_compression: false,
        };

        let db = ImmutableDB::open(config).unwrap();

        // Create test block
        let slot = SlotNo(100);
        let block_no = BlockNo(50);
        let block_data = b"test block data";
        let hash = Blake2b256Hash::hash(block_data);

        // Store block
        db.store_block(slot, block_no, hash, block_data)
            .await
            .unwrap();

        // Finalize chunk
        db.finalize_current_chunk().await.unwrap();

        // Retrieve by slot
        let retrieved = db.get_block_by_slot(slot).await.unwrap();
        assert_eq!(retrieved.as_deref(), Some(block_data.as_slice()));

        // Retrieve by hash
        let retrieved = db.get_block_by_hash(&hash).await.unwrap();
        assert_eq!(retrieved.as_deref(), Some(block_data.as_slice()));

        // Check tip
        let tip = db.get_tip().await.unwrap();
        assert_eq!(tip.slot_no, slot);
        assert_eq!(tip.block_no, block_no);
        assert_eq!(tip.hash, hash);
    }

    #[tokio::test]
    async fn immutable_db_store_multiple_blocks() {
        let dir = tempdir().unwrap();
        let config = ImmutableDBConfig {
            path: dir.path().to_path_buf(),
            chunk_size: 1000,
            max_cached_chunks: 10,
            enable_compression: false,
        };

        let db = ImmutableDB::open(config).unwrap();

        // Store multiple blocks
        for i in 0..10 {
            let slot = SlotNo(i * 10);
            let block_no = BlockNo(i);
            let block_data = format!("block_{}", i).into_bytes();
            let hash = Blake2b256Hash::hash(&block_data);

            db.store_block(slot, block_no, hash, &block_data)
                .await
                .unwrap();
        }

        // Finalize chunk
        db.finalize_current_chunk().await.unwrap();

        // Verify all blocks can be retrieved
        for i in 0..10 {
            let slot = SlotNo(i * 10);
            let expected = format!("block_{}", i).into_bytes();
            let retrieved = db.get_block_by_slot(slot).await.unwrap().unwrap();
            assert_eq!(retrieved, expected);
        }

        // Check tip is the last block
        let tip = db.get_tip().await.unwrap();
        assert_eq!(tip.slot_no, SlotNo(90));
        assert_eq!(tip.block_no, BlockNo(9));
    }

    #[tokio::test]
    async fn immutable_db_index_persistence() {
        let dir = tempdir().unwrap();
        let config = ImmutableDBConfig {
            path: dir.path().to_path_buf(),
            chunk_size: 1000,
            max_cached_chunks: 10,
            enable_compression: false,
        };

        // Store some blocks
        {
            let db = ImmutableDB::open(config.clone()).unwrap();

            for i in 0..5 {
                let slot = SlotNo(i * 10);
                let block_no = BlockNo(i);
                let block_data = format!("block_{}", i).into_bytes();
                let hash = Blake2b256Hash::hash(&block_data);

                db.store_block(slot, block_no, hash, &block_data)
                    .await
                    .unwrap();
            }

            db.finalize_current_chunk().await.unwrap();
            db.save_indices().await.unwrap();
        }

        // Reopen and verify indices were persisted
        {
            let db = ImmutableDB::open(config).unwrap();
            db.load_indices().await.unwrap();

            // Verify tip was restored
            let tip = db.get_tip().await.unwrap();
            assert_eq!(tip.slot_no, SlotNo(40));
            assert_eq!(tip.block_no, BlockNo(4));

            // Verify blocks can be retrieved via indices
            for i in 0..5 {
                let slot = SlotNo(i * 10);
                let expected = format!("block_{}", i).into_bytes();
                let retrieved = db.get_block_by_slot(slot).await.unwrap().unwrap();
                assert_eq!(retrieved, expected);
            }
        }
    }

    #[tokio::test]
    async fn immutable_db_empty_block_rejected() {
        let dir = tempdir().unwrap();
        let config = ImmutableDBConfig {
            path: dir.path().to_path_buf(),
            chunk_size: 1000,
            max_cached_chunks: 10,
            enable_compression: false,
        };

        let db = ImmutableDB::open(config).unwrap();

        // Try to store empty block
        let slot = SlotNo(100);
        let block_no = BlockNo(50);
        let block_data = b"";
        let hash = Blake2b256Hash::hash(block_data);

        let result = db.store_block(slot, block_no, hash, block_data).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[tokio::test]
    async fn immutable_db_oversized_block_rejected() {
        let dir = tempdir().unwrap();
        let config = ImmutableDBConfig {
            path: dir.path().to_path_buf(),
            chunk_size: 1000,
            max_cached_chunks: 10,
            enable_compression: false,
        };

        let db = ImmutableDB::open(config).unwrap();

        // Try to store block larger than 16 MB
        let slot = SlotNo(100);
        let block_no = BlockNo(50);
        let block_data = vec![0u8; 17 * 1024 * 1024]; // 17 MB
        let hash = Blake2b256Hash::hash(&block_data);

        let result = db.store_block(slot, block_no, hash, &block_data).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too large"));
    }

    #[tokio::test]
    async fn immutable_db_get_nonexistent_block_by_slot() {
        let dir = tempdir().unwrap();
        let config = ImmutableDBConfig {
            path: dir.path().to_path_buf(),
            chunk_size: 1000,
            max_cached_chunks: 10,
            enable_compression: false,
        };

        let db = ImmutableDB::open(config).unwrap();

        // Try to retrieve non-existent block
        let result = db.get_block_by_slot(SlotNo(999999)).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn immutable_db_get_nonexistent_block_by_hash() {
        let dir = tempdir().unwrap();
        let config = ImmutableDBConfig {
            path: dir.path().to_path_buf(),
            chunk_size: 1000,
            max_cached_chunks: 10,
            enable_compression: false,
        };

        let db = ImmutableDB::open(config).unwrap();

        // Try to retrieve non-existent block
        let fake_hash = Blake2b256Hash::new([0u8; 32]);
        let result = db.get_block_by_hash(&fake_hash).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn immutable_db_concurrent_reads() {
        use std::sync::Arc;

        let dir = tempdir().unwrap();
        let config = ImmutableDBConfig {
            path: dir.path().to_path_buf(),
            chunk_size: 1000,
            max_cached_chunks: 10,
            enable_compression: false,
        };

        let db = Arc::new(ImmutableDB::open(config).unwrap());

        // Store some blocks
        for i in 0..10 {
            let slot = SlotNo(i * 10);
            let block_no = BlockNo(i);
            let block_data = format!("block_{}", i).into_bytes();
            let hash = Blake2b256Hash::hash(&block_data);

            db.store_block(slot, block_no, hash, &block_data)
                .await
                .unwrap();
        }

        db.finalize_current_chunk().await.unwrap();

        // Spawn multiple concurrent readers
        let mut handles = vec![];
        for i in 0..10 {
            let db_clone = db.clone();
            let handle = tokio::spawn(async move {
                let slot = SlotNo(i * 10);
                let expected = format!("block_{}", i).into_bytes();

                // Read multiple times
                for _ in 0..100 {
                    let retrieved = db_clone.get_block_by_slot(slot).await.unwrap().unwrap();
                    assert_eq!(retrieved, expected);
                }
            });
            handles.push(handle);
        }

        // Wait for all readers
        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[tokio::test]
    async fn immutable_db_multiple_chunks() {
        let dir = tempdir().unwrap();
        let config = ImmutableDBConfig {
            path: dir.path().to_path_buf(),
            chunk_size: 5, // Small chunks to force multiple
            max_cached_chunks: 10,
            enable_compression: false,
        };

        let db = ImmutableDB::open(config).unwrap();

        // Store blocks across multiple chunks
        for i in 0..20 {
            let slot = SlotNo(i);
            let block_no = BlockNo(i);
            let block_data = format!("block_{}", i).into_bytes();
            let hash = Blake2b256Hash::hash(&block_data);

            db.store_block(slot, block_no, hash, &block_data)
                .await
                .unwrap();
        }

        db.finalize_current_chunk().await.unwrap();

        // Verify all blocks can be retrieved from different chunks
        for i in 0..20 {
            let slot = SlotNo(i);
            let expected = format!("block_{}", i).into_bytes();
            let retrieved = db.get_block_by_slot(slot).await.unwrap().unwrap();
            assert_eq!(retrieved, expected);
        }

        // Check that multiple chunk files were created
        let chunk_files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("dat"))
            .collect();

        assert!(chunk_files.len() >= 4, "Expected multiple chunk files");
    }

    #[tokio::test]
    async fn immutable_db_lru_cache_eviction() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let config = ImmutableDBConfig {
            path: dir.path().to_path_buf(),
            chunk_size: 1,        // One block per chunk
            max_cached_chunks: 3, // Only keep 3 chunks in cache
            enable_compression: false,
        };

        let db = ImmutableDB::open(config).unwrap();

        // Store 5 blocks in 5 different chunks
        for i in 0..5 {
            let slot = SlotNo(i);
            let block_no = BlockNo(i);
            let hash = Blake2b256Hash::hash(&[i as u8; 100]);
            let block_data = vec![i as u8; 100];

            db.store_block(slot, block_no, hash, &block_data)
                .await
                .unwrap();
            db.finalize_current_chunk().await.unwrap();
        }

        // Cache should have at most 3 chunks (with some tolerance for current chunk)
        let cached = db.cached_chunk_count().await;
        assert!(
            cached <= 4,
            "Cache should have at most 4 chunks (3 + current), got {}",
            cached
        );

        // Access all blocks - some will trigger cache eviction
        for i in 0..5 {
            let slot = SlotNo(i);
            let retrieved = db.get_block_by_slot(slot).await.unwrap().unwrap();
            assert_eq!(retrieved, vec![i as u8; 100]);
        }

        // Verify all 5 chunk files exist on disk (eviction doesn't delete files)
        let chunk_files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|s| s.starts_with("chunk_"))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(chunk_files.len(), 5, "All 5 chunks should exist on disk");

        // Clear cache and verify
        db.clear_chunk_cache().await;
        assert_eq!(db.cached_chunk_count().await, 0, "Cache should be empty");

        // Data should still be accessible (reopened from disk)
        let retrieved = db.get_block_by_slot(SlotNo(0)).await.unwrap().unwrap();
        assert_eq!(retrieved, vec![0u8; 100]);
    }
}
