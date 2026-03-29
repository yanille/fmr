use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::fmr_dir;

const CACHE_TTL_SECONDS: u64 = 300; // 5 minutes

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StatusEntry {
    pub clean: bool,
    pub behind: bool,
    pub branch: Option<String>,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct StatusCache {
    entries: HashMap<String, StatusEntry>,
}

pub fn status_cache_path() -> PathBuf {
    let mut path = fmr_dir();
    path.push("status_cache.json");
    path
}

pub fn load_status_cache() -> StatusCache {
    let cache_path = status_cache_path();

    if cache_path.exists() {
        let data = fs::read_to_string(&cache_path).unwrap_or_default();

        if data.trim().is_empty() {
            return StatusCache::default();
        }

        match serde_json::from_str(&data) {
            Ok(cache) => cache,
            Err(_) => StatusCache::default(),
        }
    } else {
        StatusCache::default()
    }
}

pub fn save_status_cache(cache: &StatusCache) {
    let cache_path = status_cache_path();
    let json = serde_json::to_string_pretty(cache).unwrap();
    fs::write(cache_path, json).unwrap();
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn get_cached_status(path: &str) -> Option<StatusEntry> {
    let cache = load_status_cache();
    let entry = cache.entries.get(path)?;

    let now = current_timestamp();
    let age = now.saturating_sub(entry.timestamp);

    if age < CACHE_TTL_SECONDS {
        Some(entry.clone())
    } else {
        None
    }
}

pub fn set_cached_status(path: &str, clean: bool, behind: bool, branch: Option<String>) {
    let mut cache = load_status_cache();

    cache.entries.insert(
        path.to_string(),
        StatusEntry {
            clean,
            behind,
            branch,
            timestamp: current_timestamp(),
        },
    );

    save_status_cache(&cache);
}

#[allow(dead_code)]
pub fn invalidate_cached_status(path: &str) {
    let mut cache = load_status_cache();
    cache.entries.remove(path);
    save_status_cache(&cache);
}

pub fn clear_status_cache() {
    let cache_path = status_cache_path();
    if cache_path.exists() {
        fs::remove_file(cache_path).ok();
    }
}
