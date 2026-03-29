//! Memory-mapped status cache for Git repository status information.
//!
//! This module provides a high-performance caching mechanism for git status
//! data using a two-file architecture:
//!
//! - `status_index.bin`: Maps repository paths to data file offsets (loaded into memory)
//! - `status_data.bin`: Binary-encoded status entries (memory-mapped for lazy loading)
//!
//! # Architecture
//!
//! The cache uses a write-once, append-only data file with an in-memory index.
//! This design provides O(1) lookups with minimal memory usage:
//!
//! 1. Index is loaded into a HashMap on first access
//! 2. Status data is read from memory-mapped file only when needed
//! 3. Old entries are never deleted (cache grows indefinitely)
//!
//! # Cache TTL
//!
//! Status entries expire after 5 minutes (`CACHE_TTL_SECONDS`). Expired entries
//! are automatically recomputed on next access.
//!
//! # Files
//!
//! All cache files are stored in `~/.fmr/`:
//! - `~/.fmr/status_index.bin` - Path → offset mappings
//! - `~/.fmr/status_data.bin` - Binary status data

use bincode::{deserialize, serialize};
use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::fmr_dir;

/// Time-to-live for cached status entries in seconds.
/// After this duration, entries are considered stale and will be recomputed.
const CACHE_TTL_SECONDS: u64 = 300; // 5 minutes

/// Filename for the index file (path → offset mappings).
const INDEX_FILE: &str = "status_index.bin";

/// Filename for the data file (binary status entries).
const DATA_FILE: &str = "status_data.bin";

/// A cached status entry for a single repository.
///
/// Stores the git status information along with a timestamp for TTL validation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StatusEntry {
    pub clean: bool,
    pub behind: bool,
    pub branch: Option<String>,
    pub timestamp: u64,
}

/// Type alias for index entries.
///
/// Stores (offset in data file, length of serialized entry) for each repository path.
type IndexEntry = (u64, u32);

/// Returns the path to the index file.
///
/// The index is stored at `~/.fmr/status_index.bin`.
fn index_path() -> PathBuf {
    let mut path = fmr_dir();
    path.push(INDEX_FILE);
    path
}

/// Returns the path to the data file.
///
/// Status data is stored at `~/.fmr/status_data.bin`.
fn data_path() -> PathBuf {
    let mut path = fmr_dir();
    path.push(DATA_FILE);
    path
}

/// Returns the current Unix timestamp in seconds.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Loads the index from disk into memory.
///
/// Deserializes the index file into a HashMap for O(1) lookups.
/// If the file doesn't exist or can't be read, returns an empty map.
///
/// # Performance
/// This operation is fast because the index only stores path → offset mappings,
/// not the actual status data.
fn load_index() -> HashMap<String, IndexEntry> {
    let index_path = index_path();

    if !index_path.exists() {
        return HashMap::new();
    }

    let file = match File::open(&index_path) {
        Ok(f) => f,
        Err(_) => return HashMap::new(),
    };

    // Memory map the index file for fast reads
    let mmap = match unsafe { Mmap::map(&file) } {
        Ok(m) => m,
        Err(_) => return HashMap::new(),
    };

    deserialize(&mmap).unwrap_or_default()
}

/// Saves the index to disk.
///
/// Serializes the HashMap to binary and writes to the index file.
/// Creates the file if it doesn't exist, truncating any existing content.
///
/// # Panics
/// Panics if file operations fail.
fn save_index(index: &HashMap<String, IndexEntry>) {
    let index_path = index_path();
    let encoded = serialize(index).unwrap_or_default();

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&index_path)
        .unwrap();

    file.write_all(&encoded).unwrap();
    file.flush().unwrap();
}

/// Reads a specific status entry from the data file.
///
/// Uses memory-mapped I/O for efficient access to the entry at the given offset.
/// Only reads the specific bytes needed for this entry.
///
/// # Arguments
/// * `offset` - Byte offset in the data file where the entry begins
/// * `length` - Length of the serialized entry in bytes
///
/// # Returns
/// * `Some(StatusEntry)` - Successfully read and deserialized entry
/// * `None` - Entry not found, file missing, or deserialization failed
///
/// # Safety
/// Uses `unsafe` for memory mapping. The file is opened read-only, so this is safe.
fn read_data_entry(offset: u64, length: u32) -> Option<StatusEntry> {
    let data_path = data_path();

    if !data_path.exists() {
        return None;
    }

    let file = File::open(&data_path).ok()?;

    // Memory map the data file for efficient access
    let mmap = unsafe { Mmap::map(&file).ok()? };

    // Calculate the byte range for this specific entry
    let start = offset as usize;
    let end = start + length as usize;

    // Bounds check
    if end > mmap.len() {
        return None;
    }

    // Deserialize the entry bytes
    let entry_bytes = &mmap[start..end];
    deserialize(entry_bytes).ok()
}

/// Appends a new status entry to the data file.
///
/// Serializes the entry and appends it to the data file. Returns the offset
/// and length needed to update the index.
///
/// # Arguments
/// * `entry` - The status entry to store
///
/// # Returns
/// * `Some((offset, length))` - Position and size of the written entry
/// * `None` - Serialization or file write failed
///
/// # Note
/// This uses append mode, so old entries are never overwritten.
fn append_data_entry(entry: &StatusEntry) -> Option<(u64, u32)> {
    let data_path = data_path();
    let encoded = serialize(entry).ok()?;
    let length = encoded.len() as u32;

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(&data_path)
        .ok()?;

    // Get current file position (where we'll write the new entry)
    let offset = file.seek(SeekFrom::End(0)).ok()?;

    // Write the serialized entry
    file.write_all(&encoded).ok()?;
    file.flush().ok()?;

    Some((offset, length))
}

/// Retrieves a cached status entry for a repository path.
///
/// Checks if a valid (non-expired) status entry exists in the cache.
/// Returns None if the entry doesn't exist or has expired.
///
/// # Arguments
/// * `path` - Absolute path to the repository
///
/// # Returns
/// * `Some(StatusEntry)` - Valid cached entry found
/// * `None` - Entry not found or expired (cache miss)
///
/// # Performance
/// - Loads only the index into memory (fast)
/// - Reads status data via memory mapping (lazy)
/// - O(1) lookup via HashMap
pub fn get_cached_status(path: &str) -> Option<StatusEntry> {
    // Load only the index (small, fast operation)
    let index = load_index();

    // Look up the entry location in the index
    let (offset, length) = index.get(path)?;

    // Read only this specific entry from the data file (lazy load)
    let entry = read_data_entry(*offset, *length)?;

    // Check if the entry has expired
    let now = current_timestamp();
    let age = now.saturating_sub(entry.timestamp);

    if age < CACHE_TTL_SECONDS {
        Some(entry)
    } else {
        None
    }
}

/// Stores a status entry in the cache.
///
/// Creates a new cache entry with the current timestamp and stores it
/// in the data file, then updates the index.
///
/// # Arguments
/// * `path` - Absolute path to the repository (used as cache key)
/// * `clean` - Whether the repository has uncommitted changes
/// * `behind` - Whether the repository is behind its remote
/// * `branch` - Current branch name, if available
///
/// # Note
/// This appends a new entry rather than updating existing ones.
/// Old entries for the same path are not removed (cache grows indefinitely).
pub fn set_cached_status(path: &str, clean: bool, behind: bool, branch: Option<String>) {
    let entry = StatusEntry {
        clean,
        behind,
        branch,
        timestamp: current_timestamp(),
    };

    // Append entry to data file
    if let Some((offset, length)) = append_data_entry(&entry) {
        // Update the index with new location
        let mut index = load_index();
        index.insert(path.to_string(), (offset, length));
        save_index(&index);
    }
}

/// Clears all cached status information.
///
/// Deletes both the index and data files. This forces all status
/// information to be recomputed on next access.
///
/// Safe to call even if cache files don't exist (no-op in that case).
pub fn clear_status_cache() {
    let index_path = index_path();
    let data_path = data_path();

    if index_path.exists() {
        std::fs::remove_file(index_path).ok();
    }
    if data_path.exists() {
        std::fs::remove_file(data_path).ok();
    }
}
