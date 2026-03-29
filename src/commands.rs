use crate::cache::{self, cache_path};
use crate::config::{load_or_create_config, save_config};
use crate::git::{get_repo_status, pull_repo};
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

pub fn sync_repos(repos: &[String], all: bool, repo_name: Option<String>) {
    if !all && repo_name.is_none() {
        println!("Usage: fmr sync --all  OR  fmr sync <repo-name>");
        return;
    }

    let mut synced = 0;
    let mut skipped_dirty = 0;
    let mut skipped_clean = 0;

    let repos_to_sync: Vec<&String> = if all {
        repos.iter().collect()
    } else {
        let target = repo_name.unwrap();
        let matches: Vec<_> = repos
            .iter()
            .filter(|r| {
                PathBuf::from(r)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| r.to_string())
                    .to_lowercase()
                    .contains(&target.to_lowercase())
            })
            .collect();

        if matches.is_empty() {
            println!("No repository found matching '{}'", target);
            return;
        }

        if matches.len() > 1 {
            println!("Multiple matches found for '{}'", target);
            for m in &matches {
                println!("  - {}", m);
            }
            return;
        }

        matches
    };

    for repo_path in repos_to_sync {
        let (clean, behind, _) = get_repo_status(repo_path);

        if !clean {
            skipped_dirty += 1;
            println!("⏸️  Skipped (uncommitted changes): {}", repo_path);
            continue;
        }

        if !behind {
            skipped_clean += 1;
            continue;
        }

        print!("🔄 Syncing: {} ... ", repo_path);
        if pull_repo(repo_path) {
            synced += 1;
            println!("✅");
        } else {
            println!("❌ Failed");
        }
    }

    println!();
    println!("Sync complete:");
    println!("  ✅ Synced: {}", synced);
    println!("  ⏸️  Skipped (dirty): {}", skipped_dirty);
    if all {
        println!("  ⏭️  Already up-to-date: {}", skipped_clean);
    }
}
