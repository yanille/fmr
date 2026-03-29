use bincode::{deserialize, serialize};
use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::fmr_dir;

const CACHE_TTL_SECONDS: u64 = 300; // 5 minutes
const INDEX_FILE: &str = "status_index.bin";
const DATA_FILE: &str = "status_data.bin";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StatusEntry {
    pub clean: bool,
    pub behind: bool,
    pub branch: Option<String>,
    pub timestamp: u64,
}

// In-memory index entry: (offset in data file, length of serialized data)
type IndexEntry = (u64, u32);

fn index_path() -> PathBuf {
    let mut path = fmr_dir();
    path.push(INDEX_FILE);
    path
}

fn data_path() -> PathBuf {
    let mut path = fmr_dir();
    path.push(DATA_FILE);
    path
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Load the index from disk into memory
/// This is fast because the index is small (just path -> offset mappings)
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

/// Save the index to disk
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

/// Read a specific entry from the data file using the offset and length
fn read_data_entry(offset: u64, length: u32) -> Option<StatusEntry> {
    let data_path = data_path();

    if !data_path.exists() {
        return None;
    }

    let file = File::open(&data_path).ok()?;

    // Memory map the data file
    let mmap = unsafe { Mmap::map(&file).ok()? };

    // Calculate the slice for this specific entry
    let start = offset as usize;
    let end = start + length as usize;

    if end > mmap.len() {
        return None;
    }

    let entry_bytes = &mmap[start..end];
    deserialize(entry_bytes).ok()
}

/// Append a new entry to the data file, returns (offset, length)
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

    // Get current position (where we'll write)
    let offset = file.seek(SeekFrom::End(0)).ok()?;

    // Write the entry
    file.write_all(&encoded).ok()?;
    file.flush().ok()?;

    Some((offset, length))
}

pub fn get_cached_status(path: &str) -> Option<StatusEntry> {
    // Load only the index (small, fast)
    let index = load_index();

    // Look up the entry location
    let (offset, length) = index.get(path)?;

    // Read only this specific entry from the data file (lazy load)
    let entry = read_data_entry(*offset, *length)?;

    // Check if expired
    let now = current_timestamp();
    let age = now.saturating_sub(entry.timestamp);

    if age < CACHE_TTL_SECONDS {
        Some(entry)
    } else {
        None
    }
}

pub fn set_cached_status(path: &str, clean: bool, behind: bool, branch: Option<String>) {
    let entry = StatusEntry {
        clean,
        behind,
        branch,
        timestamp: current_timestamp(),
    };

    // Append to data file
    if let Some((offset, length)) = append_data_entry(&entry) {
        // Update index
        let mut index = load_index();
        index.insert(path.to_string(), (offset, length));
        save_index(&index);
    }
}

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
