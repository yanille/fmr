//! Repository cache management.
//!
//! This module handles scanning for Git repositories and maintaining a cache
//! of discovered repositories for fast lookups. The cache is stored as binary
//! in `~/.fmr/repos.bin` using bincode for efficient serialization.
//!
//! The scanning process:
//! 1. Reads configured scan locations from config
//! 2. Uses walkdir to recursively traverse each location
//! 3. Filters directories containing a `.git` subdirectory
//! 4. Stores results in a sorted, deduplicated vector
//!
//! Parallel processing with Rayon is used for performance when checking
//! for `.git` directories.

use crate::config::{fmr_dir, load_or_create_config};
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

/// Returns the path to the repository cache file.
///
/// The cache is stored at `~/.fmr/repos.bin`.
pub fn cache_path() -> PathBuf {
    let mut path = fmr_dir();
    path.push("repos.bin");
    path
}

/// Scans all configured locations for Git repositories.
///
/// Walks through each configured scan location, looking for directories
/// that contain a `.git` subdirectory. Uses parallel processing for
/// improved performance.
///
/// # Returns
/// A sorted, deduplicated vector of absolute paths to Git repositories.
///
/// # Performance
/// - Uses Rayon for parallel `.git` directory detection
/// - Follows symbolic links during traversal
/// - Skips non-existent or non-directory paths silently
pub fn scan_repos() -> Vec<String> {
    let config = load_or_create_config();
    let mut all_repos = Vec::new();

    // Iterate through each configured scan location
    for location in &config.locations {
        let path = PathBuf::from(location);

        // Skip invalid paths silently
        if !path.exists() || !path.is_dir() {
            continue;
        }

        // Collect all directories using walkdir
        let dirs: Vec<_> = WalkDir::new(&path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_dir())
            .map(|e| e.path().to_path_buf())
            .collect();

        // Check for .git subdirectory in parallel
        let repos: Vec<String> = dirs
            .par_iter()
            .filter(|path| path.join(".git").is_dir())
            .map(|path| path.display().to_string())
            .collect();

        all_repos.extend(repos);
    }

    // Ensure consistent ordering and remove duplicates
    all_repos.sort();
    all_repos.dedup();
    all_repos
}

/// Loads the repository cache from disk, or rebuilds it if missing/invalid.
///
/// Attempts to read and parse the cache file using binary deserialization.
/// If the cache doesn't exist, is empty, or contains invalid data, triggers a rebuild.
///
/// # Returns
/// A vector of repository paths, either from cache or freshly scanned.
///
/// # Errors
/// Panics if cache rebuilding fails (file system issues).
pub fn load_or_create_cache() -> Vec<String> {
    let cache = cache_path();

    if cache.exists() {
        let data = fs::read(&cache).unwrap_or_default();

        // Empty cache file needs rebuilding
        if data.is_empty() {
            return rebuild_cache(cache);
        }

        // Parse binary data or rebuild on error
        match bincode::deserialize(&data) {
            Ok(repos) => repos,
            Err(_) => rebuild_cache(cache),
        }
    } else {
        // No cache exists yet, build it
        rebuild_cache(cache)
    }
}

/// Rebuilds the repository cache by scanning and saving results.
///
/// Performs a fresh scan of all configured locations and writes the
/// results to the cache file using binary serialization. Displays progress
/// information to stdout.
///
/// # Arguments
/// * `cache` - Path to the cache file
///
/// # Returns
/// The vector of discovered repository paths.
///
/// # Panics
/// Panics if bincode serialization or file write fails.
pub fn rebuild_cache(cache: PathBuf) -> Vec<String> {
    println!("Building repo cache...\n");

    let repos = scan_repos();
    let encoded = bincode::serialize(&repos).unwrap();

    fs::write(cache, encoded).unwrap();

    println!("Indexed {} repositories\n", repos.len());
    repos
}
