use crate::config::{fmr_dir, load_or_create_config};
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

pub fn cache_path() -> PathBuf {
    let mut path = fmr_dir();
    path.push("repos.json");
    path
}

pub fn scan_repos() -> Vec<String> {
    let config = load_or_create_config();
    let mut all_repos = Vec::new();

    for location in &config.locations {
        let path = PathBuf::from(location);

        if !path.exists() || !path.is_dir() {
            continue;
        }

        let dirs: Vec<_> = WalkDir::new(&path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_dir())
            .map(|e| e.path().to_path_buf())
            .collect();

        let repos: Vec<String> = dirs
            .par_iter()
            .filter(|path| path.join(".git").is_dir())
            .map(|path| path.display().to_string())
            .collect();

        all_repos.extend(repos);
    }

    all_repos.sort();
    all_repos.dedup();
    all_repos
}

pub fn load_or_create_cache() -> Vec<String> {
    let cache = cache_path();

    if cache.exists() {
        let data = fs::read_to_string(&cache).unwrap_or_default();

        if data.trim().is_empty() {
            return rebuild_cache(cache);
        }

        match serde_json::from_str(&data) {
            Ok(repos) => repos,
            Err(_) => rebuild_cache(cache),
        }
    } else {
        rebuild_cache(cache)
    }
}

pub fn rebuild_cache(cache: PathBuf) -> Vec<String> {
    println!("Building repo cache...\n");

    let repos = scan_repos();
    let json = serde_json::to_string(&repos).unwrap();

    fs::write(cache, json).unwrap();

    println!("Indexed {} repositories\n", repos.len());
    repos
}
