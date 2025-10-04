//! Chunk file format and I/O operations
//!
//! This module defines the binary format for chunk files and provides
//! methods for reading and writing blocks to chunks using memory-mapped I/O.

use crate::cardanodb::types::{BlockLocation, ChunkNo};
use anyhow::{Context, Result};
use memmap2::Mmap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Magic bytes for chunk files: "CARDANO_CHUNK"
pub const CHUNK_MAGIC: &[u8; 13] = b"CARDANO_CHUNK";

/// Current chunk file format version
pub const CHUNK_VERSION: u32 = 1;

/// A chunk file header
#[repr(C)]
pub struct ChunkHeader {
    /// Magic bytes
    pub magic: [u8; 13],
    /// Format version
    pub version: u32,
    /// Chunk number
    pub chunk_no: ChunkNo,
    /// Number of blocks in this chunk
    pub block_count: u32,
}

impl ChunkHeader {
    pub fn new(chunk_no: ChunkNo) -> Self {
        Self {
            magic: *CHUNK_MAGIC,
            version: CHUNK_VERSION,
            chunk_no,
            block_count: 0,
        }
    }

    /// Validate header magic and version
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.magic == *CHUNK_MAGIC,
            "Invalid chunk magic: expected {:?}, got {:?}",
            CHUNK_MAGIC,
            self.magic
        );
        anyhow::ensure!(
            self.version == CHUNK_VERSION,
            "Unsupported chunk version: expected {}, got {}",
            CHUNK_VERSION,
            self.version
        );
        Ok(())
    }
}

/// A chunk file with memory-mapped I/O
pub struct ChunkFile {
    /// Path to the chunk file
    path: PathBuf,
    /// Chunk number
    chunk_no: ChunkNo,
    /// File handle (for writing)
    file: Option<File>,
    /// Memory-mapped view (for reading)
    mmap: Option<Arc<Mmap>>,
    /// Current write offset
    write_offset: u64,
}

impl ChunkFile {
    /// Header size in bytes (magic + version + chunk_no + block_count)
    pub const HEADER_SIZE: u64 = 13 + 4 + 8 + 4; // 29 bytes

    /// Create a new chunk file for writing
    pub fn create<P: AsRef<Path>>(path: P, chunk_no: ChunkNo) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .context("Failed to create chunk file")?;

        // Write header
        let header = ChunkHeader::new(chunk_no);
        file.write_all(&header.magic)?;
        file.write_all(&header.version.to_le_bytes())?;
        file.write_all(&header.chunk_no.to_u64().to_le_bytes())?;
        file.write_all(&header.block_count.to_le_bytes())?;
        file.flush()?;

        Ok(Self {
            path,
            chunk_no,
            file: Some(file),
            mmap: None,
            write_offset: Self::HEADER_SIZE,
        })
    }

    /// Open an existing chunk file for reading
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        let file = File::open(&path).context("Failed to open chunk file")?;
        let mmap = unsafe { Mmap::map(&file).context("Failed to mmap chunk file")? };

        // Validate header
        anyhow::ensure!(
            mmap.len() >= Self::HEADER_SIZE as usize,
            "Chunk file too small: {} bytes",
            mmap.len()
        );

        let mut magic = [0u8; 13];
        magic.copy_from_slice(&mmap[0..13]);
        anyhow::ensure!(magic == *CHUNK_MAGIC, "Invalid chunk magic");

        let version = u32::from_le_bytes(mmap[13..17].try_into()?);
        anyhow::ensure!(version == CHUNK_VERSION, "Unsupported chunk version");

        let chunk_no_val = u64::from_le_bytes(mmap[17..25].try_into()?);
        let chunk_no = ChunkNo(chunk_no_val);

        Ok(Self {
            path,
            chunk_no,
            file: None,
            mmap: Some(Arc::new(mmap)),
            write_offset: 0,
        })
    }

    /// Write a block to the chunk
    pub fn write_block(&mut self, block_data: &[u8]) -> Result<BlockLocation> {
        let file = self
            .file
            .as_mut()
            .context("Chunk file not opened for writing")?;

        let offset = self.write_offset;
        let size = block_data.len() as u32;

        file.write_all(block_data)
            .context("Failed to write block data")?;

        self.write_offset += size as u64;

        Ok(BlockLocation::new(offset, size))
    }

    /// Read a block from the chunk using memory-mapped I/O (zero-copy)
    pub fn read_block(&self, location: &BlockLocation) -> Result<&[u8]> {
        let mmap = self
            .mmap
            .as_ref()
            .context("Chunk file not opened for reading")?;

        let start = location.offset as usize;
        let end = location.end_offset() as usize;

        anyhow::ensure!(
            end <= mmap.len(),
            "Block location out of bounds: {}..{} (file size: {})",
            start,
            end,
            mmap.len()
        );

        Ok(&mmap[start..end])
    }

    /// Flush all pending writes to disk
    pub fn flush(&mut self) -> Result<()> {
        if let Some(ref mut file) = self.file {
            file.flush().context("Failed to flush chunk file")?;
        }
        Ok(())
    }

    /// Get the chunk number
    pub fn chunk_no(&self) -> ChunkNo {
        self.chunk_no
    }

    /// Get the current file size
    pub fn size(&self) -> u64 {
        if let Some(ref mmap) = self.mmap {
            mmap.len() as u64
        } else {
            self.write_offset
        }
    }

    /// Finalize the chunk (convert from write mode to read mode)
    pub fn finalize(mut self) -> Result<Self> {
        // Flush any pending writes
        self.flush()?;

        // Close the file handle
        drop(self.file.take());

        // Reopen as memory-mapped
        let file = File::open(&self.path).context("Failed to reopen chunk file")?;
        let mmap = unsafe { Mmap::map(&file).context("Failed to mmap finalized chunk")? };

        Ok(Self {
            path: self.path,
            chunk_no: self.chunk_no,
            file: None,
            mmap: Some(Arc::new(mmap)),
            write_offset: 0,
        })
    }
}

// TODO: Implement chunk reader/writer in next phase

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn chunk_header_validation() {
        let header = ChunkHeader::new(ChunkNo(42));
        assert!(header.validate().is_ok());
    }

    #[test]
    fn invalid_magic_fails_validation() {
        let mut header = ChunkHeader::new(ChunkNo(0));
        header.magic = *b"WRONG_MAGIC!!";
        assert!(header.validate().is_err());
    }

    #[test]
    fn chunk_file_create_and_write() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_0000.dat");

        let mut chunk = ChunkFile::create(&path, ChunkNo(0)).unwrap();

        // Write some test blocks
        let block1 = b"Block 1 data";
        let block2 = b"Block 2 with more data";

        let loc1 = chunk.write_block(block1).unwrap();
        let loc2 = chunk.write_block(block2).unwrap();

        assert_eq!(loc1.offset, ChunkFile::HEADER_SIZE);
        assert_eq!(loc1.size, block1.len() as u32);
        assert_eq!(loc2.offset, ChunkFile::HEADER_SIZE + block1.len() as u64);
        assert_eq!(loc2.size, block2.len() as u32);

        chunk.flush().unwrap();
    }

    #[test]
    fn chunk_file_write_and_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_0001.dat");

        // Write phase
        let mut chunk = ChunkFile::create(&path, ChunkNo(1)).unwrap();
        let block_data = b"Test block data for reading";
        let location = chunk.write_block(block_data).unwrap();
        let chunk = chunk.finalize().unwrap();

        // Read phase
        let read_data = chunk.read_block(&location).unwrap();
        assert_eq!(read_data, block_data);
    }

    #[test]
    fn chunk_file_multiple_blocks() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_0002.dat");

        let blocks = vec![
            b"First block".to_vec(),
            b"Second block with different size".to_vec(),
            b"Third".to_vec(),
            b"Fourth block is the longest one here".to_vec(),
        ];

        // Write all blocks
        let mut chunk = ChunkFile::create(&path, ChunkNo(2)).unwrap();
        let mut locations = Vec::new();
        for block in &blocks {
            let loc = chunk.write_block(block).unwrap();
            locations.push(loc);
        }
        let chunk = chunk.finalize().unwrap();

        // Read and verify all blocks
        for (i, loc) in locations.iter().enumerate() {
            let read_data = chunk.read_block(loc).unwrap();
            assert_eq!(read_data, blocks[i].as_slice(), "Block {} mismatch", i);
        }
    }

    #[test]
    fn chunk_file_open_existing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_0003.dat");

        // Create and write
        let mut chunk = ChunkFile::create(&path, ChunkNo(3)).unwrap();
        let block_data = b"Persisted block data";
        let location = chunk.write_block(block_data).unwrap();
        chunk.flush().unwrap();
        drop(chunk);

        // Reopen and read
        let chunk = ChunkFile::open(&path).unwrap();
        assert_eq!(chunk.chunk_no(), ChunkNo(3));
        let read_data = chunk.read_block(&location).unwrap();
        assert_eq!(read_data, block_data);
    }

    #[test]
    fn chunk_file_bounds_checking() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_0004.dat");

        let mut chunk = ChunkFile::create(&path, ChunkNo(4)).unwrap();
        chunk.write_block(b"Small block").unwrap();
        let chunk = chunk.finalize().unwrap();

        // Try to read beyond file bounds
        let invalid_location = BlockLocation::new(1000, 100);
        assert!(chunk.read_block(&invalid_location).is_err());
    }
}
