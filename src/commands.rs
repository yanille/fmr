use crate::cache::{self, cache_path};
use crate::config::{load_or_create_config, save_config};
use crate::git::{branch_exists, checkout_branch, get_repo_status, pull_repo};
use crate::status_cache;
use self_update::backends::github::Update;
use std::env;
use std::path::Path;
use std::path::PathBuf;

/// Check if the current directory is a repository in the list
/// Returns Some(repo_path) if current dir is in the list, None otherwise
fn get_current_repo(repos: &[String]) -> Option<String> {
    let current_dir = env::current_dir().ok()?;
    let canonical_current = current_dir.canonicalize().unwrap_or(current_dir);
    let current_str = canonical_current.display().to_string();

    // Check if current directory is exactly a repo in the list
    if repos.contains(&current_str) {
        return Some(current_str);
    }

    // Check if current directory is inside any repo (as a subdirectory)
    for repo in repos {
        if current_str.starts_with(repo) {
            return Some(repo.clone());
        }
    }

    None
}

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

pub fn sync_repos(repos: &[String], all: bool, current: bool) {
    let mut synced = 0;
    let mut skipped_dirty = 0;
    let mut skipped_clean = 0;

    if all {
        // Sync all repos
        for repo_path in repos {
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
        println!("  ⏭️  Already up-to-date: {}", skipped_clean);
    } else if current {
        // Sync only current repository
        match get_current_repo(repos) {
            Some(current_repo) => {
                let (clean, behind, _) = get_repo_status(&current_repo);

                if !clean {
                    println!("⏸️  Skipped (uncommitted changes): {}", current_repo);
                    return;
                }

                if !behind {
                    println!("⏭️  Already up-to-date: {}", current_repo);
                    return;
                }

                print!("🔄 Syncing: {} ... ", current_repo);
                if pull_repo(&current_repo) {
                    println!("✅");
                } else {
                    println!("❌ Failed");
                }
            }
            None => {
                println!("Not currently in a tracked repository directory.");
            }
        }
    }
}

pub fn checkout_repos(repos: &[String], all: bool, current: bool, branch: &str) {
    let mut checked_out = 0;
    let mut skipped_no_branch = 0;
    let mut skipped_dirty = 0;
    let mut skipped_already_on_branch = 0;

    if all {
        // Checkout in all repos
        for repo_path in repos {
            let (clean, _, current_branch) = get_repo_status(repo_path);

            // Check if already on the target branch
            if let Some(ref current) = current_branch {
                if current == branch {
                    skipped_already_on_branch += 1;
                    continue;
                }
            }

            // Check for uncommitted changes
            if !clean {
                skipped_dirty += 1;
                println!("⏸️  Skipped (uncommitted changes): {}", repo_path);
                continue;
            }

            // Check if branch exists
            if !branch_exists(repo_path, branch) {
                skipped_no_branch += 1;
                println!("⏸️  Skipped (branch '{}' not found): {}", branch, repo_path);
                continue;
            }

            // Perform checkout
            print!("🔄 Checking out '{}' in: {} ... ", branch, repo_path);
            if checkout_branch(repo_path, branch) {
                checked_out += 1;
                println!("✅");
            } else {
                println!("❌ Failed");
            }
        }

        println!();
        println!("Checkout complete:");
        println!("  ✅ Checked out: {}", checked_out);
        println!("  ⏸️  Skipped (no branch): {}", skipped_no_branch);
        println!("  ⏸️  Skipped (dirty): {}", skipped_dirty);
        println!(
            "  ⏭️  Already on branch '{}': {}",
            branch, skipped_already_on_branch
        );
    } else if current {
        // Checkout only in current repository
        match get_current_repo(repos) {
            Some(current_repo) => {
                let (clean, _, current_branch) = get_repo_status(&current_repo);

                // Check if already on the target branch
                if let Some(ref current) = current_branch {
                    if current == branch {
                        println!("⏭️  Already on branch '{}': {}", branch, current_repo);
                        return;
                    }
                }

                // Check for uncommitted changes
                if !clean {
                    println!("⏸️  Skipped (uncommitted changes): {}", current_repo);
                    return;
                }

                // Check if branch exists
                if !branch_exists(&current_repo, branch) {
                    println!(
                        "⏸️  Skipped (branch '{}' not found): {}",
                        branch, current_repo
                    );
                    return;
                }

                // Perform checkout
                print!("🔄 Checking out '{}' in: {} ... ", branch, current_repo);
                if checkout_branch(&current_repo, branch) {
                    println!("✅");
                } else {
                    println!("❌ Failed");
                }
            }
            None => {
                println!("Not currently in a tracked repository directory.");
            }
        }
    }
}
