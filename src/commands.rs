use crate::cache::{self, cache_path};
use crate::config::{load_or_create_config, save_config};
use crate::status_cache;
use self_update::backends::github::Update;
use std::path::Path;
use std::path::PathBuf;

pub fn add_location(path: String) {
    let mut config = load_or_create_config();
    let path_buf = PathBuf::from(&path);

    if !path_buf.exists() {
        println!("❌ Path does not exist: {}", path);
        return;
    }

    if !path_buf.is_dir() {
        println!("❌ Path is not a directory: {}", path);
        return;
    }

    let canonical_path = path_buf.canonicalize().unwrap_or(path_buf);
    let path_str = canonical_path.display().to_string();

    if config.locations.contains(&path_str) {
        println!("⚠️ Location already exists: {}", path);
        return;
    }

    config.locations.push(path_str);
    save_config(&config);
    println!("✅ Added location: {}", path);
}

pub fn remove_location(path: String) {
    let mut config = load_or_create_config();
    let path_buf = PathBuf::from(&path);
    let canonical_path = path_buf.canonicalize().unwrap_or(path_buf);
    let path_str = canonical_path.display().to_string();

    let initial_len = config.locations.len();
    config.locations.retain(|loc| loc != &path_str);

    if config.locations.len() < initial_len {
        save_config(&config);
        println!("✅ Removed location: {}", path);
    } else {
        println!("⚠️ Location not found: {}", path);
    }
}

pub fn list_locations() {
    let config = load_or_create_config();

    if config.locations.is_empty() {
        println!("No scan locations configured.");
        println!("Default location (~/Desktop) will be used.");
        return;
    }

    println!("Configured scan locations:");
    for (i, location) in config.locations.iter().enumerate() {
        let path = Path::new(location);
        let status = if path.exists() {
            "✅"
        } else {
            "❌ (not found)"
        };
        println!("  {}. {} {}", i + 1, location, status);
    }
}

pub fn upgrade_fmr() {
    let status = Update::configure()
        .repo_owner("yanille")
        .repo_name("fmr")
        .bin_name("fmr")
        .show_download_progress(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .target(self_update::get_target())
        .build()
        .unwrap()
        .update();

    match status {
        Ok(status) => {
            println!("✅ Upgraded to version {}", status.version());
        }
        Err(e) => {
            eprintln!("❌ Upgrade failed: {}", e);
            eprintln!("If installed in a system directory you may need:");
            eprintln!("sudo fmr upgrade");
        }
    }
}

pub fn downgrade_fmr(version: &str) {
    let status = Update::configure()
        .repo_owner("yanille")
        .repo_name("fmr")
        .bin_name("fmr")
        .show_download_progress(true)
        .target(self_update::get_target())
        .current_version(env!("CARGO_PKG_VERSION"))
        .target_version_tag(&format!("v{}", version))
        .build()
        .unwrap()
        .update();

    match status {
        Ok(status) => {
            println!("⬇️ Downgraded to version {}", status.version());
        }
        Err(e) => {
            eprintln!("❌ Downgrade failed: {}", e);
            eprintln!("If installed in a system directory you may need:");
            eprintln!("sudo fmr downgrade {}", version);
        }
    }
}

pub fn refresh_repos() {
    let repos = cache::scan_repos();
    let json = serde_json::to_string(&repos).unwrap();
    std::fs::write(cache_path(), json).unwrap();
    println!("Indexed {} repositories", repos.len());
}

pub fn refresh_status() {
    status_cache::clear_status_cache();
    println!("Cleared git status cache");
}

pub fn refresh_all() {
    refresh_repos();
    refresh_status();
}
