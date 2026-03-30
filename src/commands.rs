//! Command implementations for fmr CLI operations.
//!
//! This module contains the implementation of all fmr subcommands including:
//! - Repository management (scan locations)
//! - Self-update (upgrade/downgrade)
//! - Cache management (refresh)
//! - Repository operations (sync, checkout)
//!
//! Each public function corresponds to a CLI subcommand and handles
//! the business logic for that operation.

use crate::cache::{self, cache_path};
use crate::config::{load_or_create_config, save_config};
use crate::git::{
    branch_exists, checkout_branch, fetch_repo, get_current_branch, get_repo_status, pull_repo,
};
use crate::status_cache::{clear_status_cache, set_cached_status};
use self_update::backends::github::Update;
use std::env;
use std::path::Path;
use std::path::PathBuf;

/// Determines if the current working directory is within a tracked repository.
///
/// Checks if the current directory (or any parent) matches a repository
/// in the provided list. Uses canonical paths for accurate comparison.
///
/// # Arguments
/// * `repos` - List of tracked repository paths
///
/// # Returns
/// * `Some(repo_path)` - The matching repository path from the list
/// * `None` - Current directory is not in any tracked repository
///
/// # Algorithm
/// 1. Canonicalizes the current directory
/// 2. Checks for exact match in repo list
/// 3. Falls back to prefix matching (handles subdirectories within repos)
///
/// # Note
/// Uses string prefix matching which could have edge cases with similarly
/// named directories (e.g., /home/user/project matches /home/user/project-sub).
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

/// Adds a new directory to the scan locations.
///
/// Validates that the path exists and is a directory before adding.
/// Duplicate locations are automatically rejected.
///
/// # Arguments
/// * `path` - Path to add (can be relative, will be canonicalized)
///
/// # Output
/// Prints status message to stdout:
/// - ✅ Success: Location added
/// - ❌ Error: Path doesn't exist or isn't a directory
/// - ⚠️ Warning: Location already in list
pub fn add_location(path: String) {
    let mut config = load_or_create_config();
    let path_buf = PathBuf::from(&path);

    // Validate path exists
    if !path_buf.exists() {
        println!("❌ Path does not exist: {}", path);
        return;
    }

    // Validate path is a directory
    if !path_buf.is_dir() {
        println!("❌ Path is not a directory: {}", path);
        return;
    }

    // Canonicalize to get absolute path
    let canonical_path = path_buf.canonicalize().unwrap_or(path_buf);
    let path_str = canonical_path.display().to_string();

    // Check for duplicates
    if config.locations.contains(&path_str) {
        println!("⚠️ Location already exists: {}", path);
        return;
    }

    // Add and save
    config.locations.push(path_str);
    save_config(&config);
    println!("✅ Added location: {}", path);
}

/// Removes a directory from the scan locations.
///
/// Attempts to canonicalize the provided path and removes the matching
/// entry from the configuration. No-op if location not found.
///
/// # Arguments
/// * `path` - Path to remove (will be canonicalized for matching)
///
/// # Output
/// Prints status message to stdout:
/// - ✅ Success: Location removed
/// - ⚠️ Warning: Location not found in list
pub fn remove_location(path: String) {
    let mut config = load_or_create_config();
    let path_buf = PathBuf::from(&path);
    let canonical_path = path_buf.canonicalize().unwrap_or(path_buf);
    let path_str = canonical_path.display().to_string();

    // Remove matching location
    let initial_len = config.locations.len();
    config.locations.retain(|loc| loc != &path_str);

    // Report result
    if config.locations.len() < initial_len {
        save_config(&config);
        println!("✅ Removed location: {}", path);
    } else {
        println!("⚠️ Location not found: {}", path);
    }
}

/// Lists all configured scan locations.
///
/// Displays each configured location with an existence indicator.
/// Shows a message if using default configuration.
///
/// # Output
/// Prints formatted list to stdout with status indicators:
/// - ✅ Path exists
/// - ❌ (not found) Path doesn't exist
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

/// Upgrades fmr to the latest version from GitHub releases.
///
/// Uses the `self_update` crate to download and install the latest release
/// from the yanille/fmr repository. Shows download progress during update.
///
/// # Output
/// Prints status message to stdout:
/// - ✅ Success: New version number
/// - ❌ Error: Failure message with sudo hint
///
/// # Note
/// May require `sudo` if installed in a system directory like `/usr/local/bin`.
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

/// Downgrades fmr to a specific version.
///
/// Downloads and installs a specific release version from GitHub.
/// Version string should be in format "x.y.z" without the 'v' prefix.
///
/// # Arguments
/// * `version` - Target version (e.g., "0.1.0")
///
/// # Output
/// Prints status message to stdout:
/// - ⬇️ Success: New version number
/// - ❌ Error: Failure message with sudo hint
///
/// # Note
/// May require `sudo` if installed in a system directory.
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

/// Refreshes the repository list cache.
///
/// Performs a fresh scan of all configured locations and updates
/// the cache file. Displays the count of discovered repositories.
///
/// # Output
/// Prints "Indexed N repositories" to stdout.
///
/// # Panics
/// Panics if JSON serialization or file write fails.
pub fn refresh_repos() {
    let repos = cache::scan_repos();
    let json = serde_json::to_string(&repos).unwrap();
    std::fs::write(cache_path(), json).unwrap();
    println!("Indexed {} repositories", repos.len());
}

/// Clears the git status cache.
///
/// Deletes all cached status information, forcing fresh git status
/// checks on next repository display.
///
/// # Output
/// Prints confirmation message to stdout.
pub fn refresh_status() {
    clear_status_cache();
    println!("Cleared git status cache");
}

/// Refreshes both repository list and status cache.
///
/// Convenience function that runs both `refresh_repos()` and
/// `refresh_status()` in sequence.
pub fn refresh_all() {
    refresh_repos();
    refresh_status();
}

/// Syncs repositories by pulling latest changes from remote.
///
/// Pulls changes for repositories that are behind their remote tracking branch.
/// Repositories with uncommitted changes are automatically skipped to prevent
/// merge conflicts. Can operate on all repositories or just the current one.
///
/// # Arguments
/// * `repos` - List of all tracked repository paths
/// * `all` - If true, sync all repositories
/// * `current` - If true, sync only the current repository
///
/// # Behavior
/// ## All Mode
/// Iterates through all repositories:
/// - Skips repositories with uncommitted changes (red indicator)
/// - Skips repositories already up-to-date
/// - Pulls changes for repositories behind remote (orange indicator)
/// - Displays summary statistics
///
/// ## Current Mode
/// Syncs only the repository containing the current working directory.
///
/// # Output
/// Prints progress and summary to stdout:
/// - 🔄 Syncing messages
/// - ⏸️  Skipped messages with reasons
/// - Summary with counts
///
/// # Safety
/// Only operates on clean repositories to avoid merge conflicts.
pub fn sync_repos(repos: &[String], all: bool, current: bool) {
    let mut synced = 0;
    let mut skipped_dirty = 0;
    let mut skipped_clean = 0;

    // Clear cache to ensure fresh git status checks
    clear_status_cache();

    if all {
        // Sync all repos
        for repo_path in repos {
            // Fetch latest remote info first
            fetch_repo(repo_path);

            let (clean, behind, _) = get_repo_status(repo_path);

            // Skip repositories with uncommitted changes
            if !clean {
                skipped_dirty += 1;
                println!("⏸️  Skipped (uncommitted changes): {}", repo_path);
                continue;
            }

            // Skip repositories already up-to-date
            if !behind {
                skipped_clean += 1;
                continue;
            }

            // Pull latest changes
            print!("🔄 Syncing: {} ... ", repo_path);
            if pull_repo(repo_path) {
                synced += 1;
                println!("✅");
                // Update cache: now clean and up-to-date
                let branch = get_current_branch(repo_path);
                set_cached_status(repo_path, true, false, branch);
            } else {
                println!("❌ Failed");
            }
        }

        // Print summary
        println!();
        println!("Sync complete:");
        println!("  ✅ Synced: {}", synced);
        println!("  ⏸️  Skipped (dirty): {}", skipped_dirty);
        println!("  ⏭️  Already up-to-date: {}", skipped_clean);
    } else if current {
        // Sync only current repository
        match get_current_repo(repos) {
            Some(current_repo) => {
                // Fetch latest remote info first
                fetch_repo(&current_repo);

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
                    // Update cache: now clean and up-to-date
                    let branch = get_current_branch(&current_repo);
                    set_cached_status(&current_repo, true, false, branch);
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

/// Checks out a branch across repositories.
///
/// Switches to the specified branch in selected repositories.
/// Repositories are skipped if:
/// - Already on the target branch
/// - Have uncommitted changes (to prevent conflicts)
/// - Don't have the target branch
///
/// Can operate on all repositories or just the current one.
///
/// # Arguments
/// * `repos` - List of all tracked repository paths
/// * `all` - If true, checkout in all repositories
/// * `current` - If true, checkout only in current repository
/// * `branch` - Name of the branch to checkout
///
/// # Behavior
/// ## All Mode
/// Attempts to checkout the branch in every repository:
/// - Checks current branch first (skip if already on target)
/// - Verifies working directory is clean
/// - Verifies branch exists locally
/// - Performs checkout with progress output
/// - Displays summary statistics
///
/// ## Current Mode
/// Checks out the branch only in the repository containing the
/// current working directory.
///
/// # Output
/// Prints progress and summary to stdout:
/// - 🔄 Checkout messages
/// - ⏸️  Skipped messages with reasons
/// - ⏭️  Already on branch messages
/// - Summary with counts
///
/// # Safety
/// Only operates on clean repositories and verifies branch existence
/// before attempting checkout.
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

            // Check if branch exists locally
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
                // Update cache with new branch, assume clean (we checked) and not behind (optimistic)
                set_cached_status(repo_path, true, false, Some(branch.to_string()));
            } else {
                println!("❌ Failed");
            }
        }

        // Print summary
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
                    // Update cache with new branch, assume clean (we checked) and not behind (optimistic)
                    set_cached_status(&current_repo, true, false, Some(branch.to_string()));
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
