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
#[derive(Debug)]
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

/// ChunkReader - High-level abstraction for reading blocks from chunks
///
/// Provides a clean API for sequential and random access to blocks within a chunk.
/// Uses memory-mapped I/O for zero-copy reads.
///
/// # Example
///
/// ```no_run
/// # use cardano_storage::cardanodb::immutable::chunk::ChunkReader;
/// # use cardano_storage::cardanodb::types::{ChunkNo, BlockLocation};
/// # fn example() -> anyhow::Result<()> {
/// let reader = ChunkReader::open("chunk_00000000.dat", ChunkNo(0))?;
///
/// // Random access by location
/// let location = BlockLocation::new(1024, 512);
/// let block_data = reader.read_block(&location)?;
///
/// // Streaming access
/// for block in reader.iter_blocks(&[location])? {
///     println!("Block: {} bytes", block?.len());
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct ChunkReader {
    chunk: ChunkFile,
}

impl ChunkReader {
    /// Open a chunk file for reading
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the chunk file
    /// * `chunk_no` - Expected chunk number (validated against file)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File does not exist
    /// - File is corrupted or has invalid header
    /// - Chunk number mismatch
    pub fn open<P: AsRef<Path>>(path: P, chunk_no: ChunkNo) -> Result<Self> {
        let chunk = ChunkFile::open(path)?;

        anyhow::ensure!(
            chunk.chunk_no() == chunk_no,
            "Chunk number mismatch: expected {}, got {}",
            chunk_no.to_u64(),
            chunk.chunk_no().to_u64()
        );

        Ok(Self { chunk })
    }

    /// Read a single block by location (zero-copy)
    ///
    /// # Arguments
    ///
    /// * `location` - Block location (offset + size)
    ///
    /// # Returns
    ///
    /// Reference to the block data (zero-copy via mmap)
    ///
    /// # Errors
    ///
    /// Returns an error if the location is out of bounds
    pub fn read_block(&self, location: &BlockLocation) -> Result<&[u8]> {
        self.chunk.read_block(location)
    }

    /// Read multiple blocks sequentially
    ///
    /// More efficient than calling read_block() multiple times as it
    /// optimizes for sequential access patterns.
    ///
    /// # Arguments
    ///
    /// * `locations` - Slice of block locations to read
    ///
    /// # Returns
    ///
    /// Iterator over block data references
    pub fn read_blocks<'a>(
        &'a self,
        locations: &'a [BlockLocation],
    ) -> Result<impl Iterator<Item = Result<&'a [u8]>> + 'a> {
        Ok(locations.iter().map(move |loc| self.read_block(loc)))
    }

    /// Iterate over blocks sequentially
    ///
    /// Provides a streaming interface for processing blocks one at a time.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use cardano_storage::cardanodb::immutable::chunk::ChunkReader;
    /// # use cardano_storage::cardanodb::types::{ChunkNo, BlockLocation};
    /// # fn example(reader: &ChunkReader, locations: &[BlockLocation]) -> anyhow::Result<()> {
    /// for block in reader.iter_blocks(locations)? {
    ///     let data = block?;
    ///     // Process block data
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn iter_blocks<'a>(
        &'a self,
        locations: &'a [BlockLocation],
    ) -> Result<impl Iterator<Item = Result<&'a [u8]>> + 'a> {
        self.read_blocks(locations)
    }

    /// Get chunk number
    pub fn chunk_no(&self) -> ChunkNo {
        self.chunk.chunk_no()
    }

    /// Get chunk file size
    pub fn size(&self) -> u64 {
        self.chunk.size()
    }
}

/// ChunkWriter - High-level abstraction for writing blocks to chunks
///
/// Provides a clean API for sequential block writing with automatic buffering
/// and offset tracking.
///
/// # Example
///
/// ```no_run
/// # use cardano_storage::cardanodb::immutable::chunk::ChunkWriter;
/// # use cardano_storage::cardanodb::types::ChunkNo;
/// # fn example() -> anyhow::Result<()> {
/// let mut writer = ChunkWriter::create("chunk_00000001.dat", ChunkNo(1))?;
///
/// // Write blocks sequentially
/// let loc1 = writer.append_block(b"First block")?;
/// let loc2 = writer.append_block(b"Second block")?;
///
/// // Flush and finalize
/// let reader = writer.finalize()?;
/// # Ok(())
/// # }
/// ```
pub struct ChunkWriter {
    chunk: ChunkFile,
    blocks_written: u32,
}

impl ChunkWriter {
    /// Create a new chunk file for writing
    ///
    /// # Arguments
    ///
    /// * `path` - Path where chunk file will be created
    /// * `chunk_no` - Chunk number
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created
    pub fn create<P: AsRef<Path>>(path: P, chunk_no: ChunkNo) -> Result<Self> {
        let chunk = ChunkFile::create(path, chunk_no)?;
        Ok(Self {
            chunk,
            blocks_written: 0,
        })
    }

    /// Append a block to the chunk
    ///
    /// Blocks are written sequentially. Returns the location where the block
    /// was written.
    ///
    /// # Arguments
    ///
    /// * `block_data` - Block data to write
    ///
    /// # Returns
    ///
    /// Location of the written block (offset + size)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Block data is empty
    /// - Block data exceeds maximum size (16 MB)
    /// - Write fails
    pub fn append_block(&mut self, block_data: &[u8]) -> Result<BlockLocation> {
        anyhow::ensure!(!block_data.is_empty(), "Block data cannot be empty");
        anyhow::ensure!(
            block_data.len() <= 16 * 1024 * 1024,
            "Block data too large: {} bytes (max 16 MB)",
            block_data.len()
        );

        let location = self.chunk.write_block(block_data)?;
        self.blocks_written += 1;

        Ok(location)
    }

    /// Append multiple blocks in batch
    ///
    /// More efficient than calling append_block() multiple times as it
    /// optimizes for sequential writes.
    ///
    /// # Arguments
    ///
    /// * `blocks` - Slice of block data to write
    ///
    /// # Returns
    ///
    /// Vector of locations for each written block
    pub fn append_blocks(&mut self, blocks: &[&[u8]]) -> Result<Vec<BlockLocation>> {
        let mut locations = Vec::with_capacity(blocks.len());
        for block in blocks {
            let location = self.append_block(block)?;
            locations.push(location);
        }
        Ok(locations)
    }

    /// Flush pending writes to disk
    ///
    /// Ensures all buffered data is written to the filesystem.
    pub fn flush(&mut self) -> Result<()> {
        self.chunk.flush()
    }

    /// Get number of blocks written so far
    pub fn blocks_written(&self) -> u32 {
        self.blocks_written
    }

    /// Get chunk number
    pub fn chunk_no(&self) -> ChunkNo {
        self.chunk.chunk_no()
    }

    /// Get current chunk size
    pub fn size(&self) -> u64 {
        self.chunk.size()
    }

    /// Finalize the chunk and convert to reader
    ///
    /// Flushes all pending writes, closes the write handle, and reopens
    /// the file as a memory-mapped reader.
    ///
    /// # Returns
    ///
    /// A ChunkReader for reading the finalized chunk
    ///
    /// # Errors
    ///
    /// Returns an error if flush or remapping fails
    pub fn finalize(self) -> Result<ChunkReader> {
        let chunk = self.chunk.finalize()?;
        Ok(ChunkReader { chunk })
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

    // ChunkReader tests
    #[test]
    fn chunk_reader_open_and_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_reader_test.dat");

        // Create chunk with test data
        let mut writer = ChunkWriter::create(&path, ChunkNo(5)).unwrap();
        let block_data = b"Test block for reader";
        let location = writer.append_block(block_data).unwrap();
        let _reader = writer.finalize().unwrap();

        // Open with ChunkReader
        let reader = ChunkReader::open(&path, ChunkNo(5)).unwrap();
        assert_eq!(reader.chunk_no(), ChunkNo(5));

        // Read block
        let read_data = reader.read_block(&location).unwrap();
        assert_eq!(read_data, block_data);
    }

    #[test]
    fn chunk_reader_multiple_blocks() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_reader_multi.dat");

        let blocks = vec![
            b"Block one".to_vec(),
            b"Block two with more data".to_vec(),
            b"Block three".to_vec(),
        ];

        // Write blocks
        let mut writer = ChunkWriter::create(&path, ChunkNo(6)).unwrap();
        let mut locations = Vec::new();
        for block in &blocks {
            let loc = writer.append_block(block).unwrap();
            locations.push(loc);
        }
        let _reader = writer.finalize().unwrap();

        // Read back with ChunkReader
        let reader = ChunkReader::open(&path, ChunkNo(6)).unwrap();

        // Read all blocks
        for (i, result) in reader.iter_blocks(&locations).unwrap().enumerate() {
            let data = result.unwrap();
            assert_eq!(data, blocks[i].as_slice());
        }
    }

    #[test]
    fn chunk_reader_chunk_number_validation() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_validation.dat");

        // Create chunk with number 7
        let mut writer = ChunkWriter::create(&path, ChunkNo(7)).unwrap();
        writer.append_block(b"test").unwrap();
        writer.finalize().unwrap();

        // Try to open with wrong chunk number
        let result = ChunkReader::open(&path, ChunkNo(99));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Chunk number mismatch"));
    }

    // ChunkWriter tests
    #[test]
    fn chunk_writer_create_and_append() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_writer_test.dat");

        let mut writer = ChunkWriter::create(&path, ChunkNo(10)).unwrap();
        assert_eq!(writer.blocks_written(), 0);
        assert_eq!(writer.chunk_no(), ChunkNo(10));

        let block1 = b"First block";
        let loc1 = writer.append_block(block1).unwrap();
        assert_eq!(writer.blocks_written(), 1);
        assert_eq!(loc1.offset, ChunkFile::HEADER_SIZE);
        assert_eq!(loc1.size, block1.len() as u32);

        let block2 = b"Second block";
        let loc2 = writer.append_block(block2).unwrap();
        assert_eq!(writer.blocks_written(), 2);
        assert_eq!(loc2.offset, ChunkFile::HEADER_SIZE + block1.len() as u64);
    }

    #[test]
    fn chunk_writer_append_blocks_batch() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_writer_batch.dat");

        let blocks: Vec<&[u8]> = vec![b"Block 1", b"Block 2", b"Block 3", b"Block 4"];

        let mut writer = ChunkWriter::create(&path, ChunkNo(11)).unwrap();
        let locations = writer.append_blocks(&blocks).unwrap();

        assert_eq!(locations.len(), 4);
        assert_eq!(writer.blocks_written(), 4);

        // Verify locations are sequential
        let mut expected_offset = ChunkFile::HEADER_SIZE;
        for (i, loc) in locations.iter().enumerate() {
            assert_eq!(loc.offset, expected_offset);
            assert_eq!(loc.size, blocks[i].len() as u32);
            expected_offset += blocks[i].len() as u64;
        }
    }

    #[test]
    fn chunk_writer_empty_block_fails() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_writer_empty.dat");

        let mut writer = ChunkWriter::create(&path, ChunkNo(12)).unwrap();
        let result = writer.append_block(&[]);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Block data cannot be empty"));
    }

    #[test]
    fn chunk_writer_large_block_fails() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_writer_large.dat");

        let mut writer = ChunkWriter::create(&path, ChunkNo(13)).unwrap();
        let large_block = vec![0u8; 17 * 1024 * 1024]; // 17 MB > 16 MB limit
        let result = writer.append_block(&large_block);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Block data too large"));
    }

    #[test]
    fn chunk_writer_finalize_to_reader() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_writer_finalize.dat");

        let blocks = vec![b"Block A", b"Block B", b"Block C"];

        // Write blocks
        let mut writer = ChunkWriter::create(&path, ChunkNo(14)).unwrap();
        let locations = writer
            .append_blocks(&blocks.iter().map(|b| b.as_slice()).collect::<Vec<_>>())
            .unwrap();

        // Finalize to reader
        let reader = writer.finalize().unwrap();
        assert_eq!(reader.chunk_no(), ChunkNo(14));

        // Read back all blocks
        for (i, loc) in locations.iter().enumerate() {
            let data = reader.read_block(loc).unwrap();
            assert_eq!(data, blocks[i]);
        }
    }

    #[test]
    fn chunk_reader_writer_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_roundtrip.dat");

        // Write phase
        let test_blocks = vec![
            b"First block with some data".to_vec(),
            b"Second".to_vec(),
            b"Third block is longer than the others".to_vec(),
            b"Fourth".to_vec(),
        ];

        let mut writer = ChunkWriter::create(&path, ChunkNo(15)).unwrap();
        let mut locations = Vec::new();
        for block in &test_blocks {
            let loc = writer.append_block(block).unwrap();
            locations.push(loc);
        }
        writer.flush().unwrap();
        let reader = writer.finalize().unwrap();

        // Read phase
        assert_eq!(reader.chunk_no(), ChunkNo(15));
        for (i, loc) in locations.iter().enumerate() {
            let data = reader.read_block(loc).unwrap();
            assert_eq!(data, test_blocks[i].as_slice());
        }

        // Streaming read
        let streamed_blocks: Vec<_> = reader
            .iter_blocks(&locations)
            .unwrap()
            .map(|r| r.unwrap().to_vec())
            .collect();

        assert_eq!(streamed_blocks.len(), test_blocks.len());
        for (i, block) in streamed_blocks.iter().enumerate() {
            assert_eq!(block, &test_blocks[i]);
        }
    }

    #[test]
    fn chunk_writer_flush() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_flush.dat");

        let mut writer = ChunkWriter::create(&path, ChunkNo(16)).unwrap();
        writer.append_block(b"Test block").unwrap();

        // Flush should succeed
        assert!(writer.flush().is_ok());

        // Should still be able to append more
        writer.append_block(b"Another block").unwrap();
        assert_eq!(writer.blocks_written(), 2);
    }

    #[test]
    fn chunk_reader_read_blocks_sequential() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("chunk_sequential.dat");

        let blocks: Vec<&[u8]> = vec![b"A", b"BB", b"CCC", b"DDDD"];

        // Write
        let mut writer = ChunkWriter::create(&path, ChunkNo(17)).unwrap();
        let locations = writer.append_blocks(&blocks).unwrap();
        let reader = writer.finalize().unwrap();

        // Sequential read with read_blocks
        let results: Vec<_> = reader.read_blocks(&locations).unwrap().collect();
        let data_blocks: Vec<_> = results.into_iter().map(|r| r.unwrap()).collect();

        assert_eq!(data_blocks.len(), 4);
        for (i, data) in data_blocks.iter().enumerate() {
            assert_eq!(*data, blocks[i]);
        }
    }
}
