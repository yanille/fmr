//! Command implementations for fmr CLI operations.
//!
//! This module contains the implementation of all fmr subcommands including:
//! - Repository management (scan locations)
//! - Self-update (upgrade/downgrade)
//! - Cache management (refresh)
//!
//! Each public function corresponds to a CLI subcommand and handles
//! the business logic for that operation.

use crate::cache::{self, cache_path};
use crate::config::{load_or_create_config, save_config};
use crate::status_cache::clear_status_cache;
use self_update::backends::github::Update;
use std::path::Path;
use std::path::PathBuf;

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
