//! Memory-mapped status cache for Git repository status information.
//!
//! This module provides a high-performance caching mechanism for git status
//! data using a two-file architecture:
//!
//! - `status_lookup.bin`: Maps repository paths to entries file offsets (loaded into memory)
//! - `status_entries.bin`: Binary-encoded status entries (memory-mapped for lazy loading)
//!
//! # Architecture
//!
//! The cache uses an in-memory lookup table with a compacted entries file. This design
//! provides O(1) lookups with minimal memory usage:
//!
//! 1. Lookup table is loaded into a HashMap on first access
//! 2. Status entries are read from memory-mapped file only when needed
//! 3. When updating an entry, the cache is compacted to remove stale entries
//!
//! # Cache TTL
//!
//! Status entries expire after 5 minutes (`CACHE_TTL_SECONDS`). Expired entries
//! are automatically recomputed on next access.
//!
//! # Files
//!
//! All cache files are stored in `~/.fmr/`:
//! - `~/.fmr/status_lookup.bin` - Path → offset mappings for quick lookup
//! - `~/.fmr/status_entries.bin` - Binary-encoded status entries

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

/// Filename for the lookup file (path → offset mappings).
const LOOKUP_FILE: &str = "status_lookup.bin";

/// Filename for the entries file (binary status entries).
const ENTRIES_FILE: &str = "status_entries.bin";

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

/// Type alias for lookup entries.
///
/// Stores (offset in entries file, length of serialized entry) for each repository path.
type IndexEntry = (u64, u32);

/// Returns the path to the lookup file.
///
/// The lookup table is stored at `~/.fmr/status_lookup.bin`.
fn lookup_path() -> PathBuf {
    let mut path = fmr_dir();
    path.push(LOOKUP_FILE);
    path
}

/// Returns the path to the entries file.
///
/// Status entries are stored at `~/.fmr/status_entries.bin`.
fn entries_path() -> PathBuf {
    let mut path = fmr_dir();
    path.push(ENTRIES_FILE);
    path
}

/// Returns the current Unix timestamp in seconds.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Loads the lookup table from disk into memory.
///
/// Deserializes the lookup file into a HashMap for O(1) lookups.
/// If the file doesn't exist or can't be read, returns an empty map.
///
/// # Performance
/// This operation is fast because the lookup table only stores path → offset mappings,
/// not the actual status data.
fn load_index() -> HashMap<String, IndexEntry> {
    let lookup_file = lookup_path();

    if !lookup_file.exists() {
        return HashMap::new();
    }

    let file = match File::open(&lookup_file) {
        Ok(f) => f,
        Err(_) => return HashMap::new(),
    };

    // Memory map the lookup file for fast reads
    let mmap = match unsafe { Mmap::map(&file) } {
        Ok(m) => m,
        Err(_) => return HashMap::new(),
    };

    deserialize(&mmap).unwrap_or_default()
}

/// Saves the lookup table to disk.
///
/// Serializes the HashMap to binary and writes to the lookup file.
/// Creates the file if it doesn't exist, truncating any existing content.
///
/// # Panics
/// Panics if file operations fail.
fn save_index(index: &HashMap<String, IndexEntry>) {
    let lookup_file = lookup_path();
    let encoded = serialize(index).unwrap_or_default();

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lookup_file)
        .unwrap();

    file.write_all(&encoded).unwrap();
    file.flush().unwrap();
}

/// Reads a specific status entry from the entries file.
///
/// Uses memory-mapped I/O for efficient access to the entry at the given offset.
/// Only reads the specific bytes needed for this entry.
///
/// # Arguments
/// * `offset` - Byte offset in the entries file where the entry begins
/// * `length` - Length of the serialized entry in bytes
///
/// # Returns
/// * `Some(StatusEntry)` - Successfully read and deserialized entry
/// * `None` - Entry not found, file missing, or deserialization failed
///
/// # Safety
/// Uses `unsafe` for memory mapping. The file is opened read-only, so this is safe.
fn read_data_entry(offset: u64, length: u32) -> Option<StatusEntry> {
    let entries_file = entries_path();

    if !entries_file.exists() {
        return None;
    }

    let file = File::open(&entries_file).ok()?;

    // Memory map the entries file for efficient access
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

/// Appends a new status entry to the entries file.
///
/// Serializes the entry and appends it to the entries file. Returns the offset
/// and length needed to update the lookup table.
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
    let entries_file = entries_path();
    let encoded = serialize(entry).ok()?;
    let length = encoded.len() as u32;

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(&entries_file)
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
/// - Loads only the lookup table into memory (fast)
/// - Reads status entries via memory mapping (lazy)
/// - O(1) lookup via HashMap
pub fn get_cached_status(path: &str) -> Option<StatusEntry> {
    // Load only the lookup table (small, fast operation)
    let lookup = load_index();

    // Look up the entry location in the lookup table
    let (offset, length) = lookup.get(path)?;

    // Read only this specific entry from the entries file (lazy load)
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

/// Compacts the cache by removing stale entries and rewriting the entries file.
///
/// This function reads all current valid entries from the cache, excluding
/// the specified path (which will be replaced with a new entry). It then
/// rewrites the entries file with only these entries and rebuilds the lookup table.
///
/// # Arguments
/// * `exclude_path` - Path to exclude from the compacted cache (the one being updated)
///
/// # Returns
/// * `Some(HashMap<String, IndexEntry>)` - New lookup table with updated offsets
/// * `None` - If compaction fails
fn compact_cache(exclude_path: &str) -> Option<HashMap<String, IndexEntry>> {
    let current_index = load_index();
    let mut new_index = HashMap::new();
    let mut new_data = Vec::new();

    // Copy all entries except the one being updated
    for (path, (offset, length)) in current_index {
        if path == exclude_path {
            continue; // Skip the entry being replaced
        }

        // Read the old entry
        if let Some(entry) = read_data_entry(offset, length) {
            // Serialize and append to new data
            let encoded = serialize(&entry).ok()?;
            let new_offset = new_data.len() as u64;
            let new_length = encoded.len() as u32;

            new_data.extend_from_slice(&encoded);
            new_index.insert(path, (new_offset, new_length));
        }
    }

    // Write the compacted entries file
    let entries_file = entries_path();
    std::fs::write(&entries_file, new_data).ok()?;

    Some(new_index)
}

/// Stores a status entry in the cache.
///
/// Creates a new cache entry with the current timestamp and stores it
/// in the entries file, then updates the lookup table. If an entry for this path
/// already exists, compacts the cache to remove the old entry first,
/// preventing unbounded growth.
///
/// # Arguments
/// * `path` - Absolute path to the repository (used as cache key)
/// * `clean` - Whether the repository has uncommitted changes
/// * `behind` - Whether the repository is behind its remote
/// * `branch` - Current branch name, if available
pub fn set_cached_status(path: &str, clean: bool, behind: bool, branch: Option<String>) {
    let entry = StatusEntry {
        clean,
        behind,
        branch,
        timestamp: current_timestamp(),
    };

    // Check if this path already exists in the cache
    let lookup = load_index();

    let mut new_lookup = if lookup.contains_key(path) {
        // Compact the cache to remove the old entry for this path
        compact_cache(path).unwrap_or_else(|| {
            // If compaction fails, start fresh
            clear_status_cache();
            HashMap::new()
        })
    } else {
        lookup
    };

    // Append entry to entries file
    if let Some((offset, length)) = append_data_entry(&entry) {
        // Update the lookup table with new location
        new_lookup.insert(path.to_string(), (offset, length));
        save_index(&new_lookup);
    }
}

/// Clears all cached status information.
///
/// Deletes both the lookup and entries files. This forces all status
/// information to be recomputed on next access.
///
/// Safe to call even if cache files don't exist (no-op in that case).
pub fn clear_status_cache() {
    let lookup_file = lookup_path();
    let entries_file = entries_path();

    if lookup_file.exists() {
        std::fs::remove_file(lookup_file).ok();
    }
    if entries_file.exists() {
        std::fs::remove_file(entries_file).ok();
    }
}
