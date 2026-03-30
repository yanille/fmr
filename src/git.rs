//! Git operations and status checking.
//!
//! This module provides functions for interacting with Git repositories,
//! including checking repository status.
//!
//! All functions use the `git` command-line tool via `std::process::Command`.
//! Status information is cached using the `status_cache` module to improve
//! performance when displaying repository lists.

use crate::status_cache::{get_cached_status, set_cached_status};
use std::process::Command;

/// Retrieves the current Git branch name for a repository.
///
/// Runs `git branch --show-current` in the specified repository.
///
/// # Arguments
/// * `path` - Absolute path to the Git repository
///
/// # Returns
/// * `Some(branch_name)` - The current branch name
/// * `None` - If not in a git repository or on a detached HEAD
///
/// # Errors
/// Returns `None` if the git command fails or produces invalid UTF-8.
pub fn get_current_branch(path: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["-C", path, "branch", "--show-current"])
        .output()
        .ok()?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    }
}

/// Checks if a repository has uncommitted changes.
///
/// Runs `git status --porcelain` and checks if output is empty.
///
/// # Arguments
/// * `path` - Absolute path to the Git repository
///
/// # Returns
/// * `true` - No uncommitted changes (clean working directory)
/// * `false` - Has uncommitted changes or check failed
///
/// # Note
/// Returns `true` (clean) if the git command fails, as a safety fallback.
fn is_repo_clean_raw(path: &str) -> bool {
    let output = Command::new("git")
        .args(["-C", path, "status", "--porcelain"])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            // Empty output means no changes
            String::from_utf8_lossy(&output.stdout).trim().is_empty()
        }
        _ => true, // Assume clean if we can't check
    }
}

/// Checks if the local branch is behind its remote tracking branch.
///
/// Uses `git rev-list --left-right --count` to compare local and remote.
/// Output format is `<behind>\t<ahead>` where behind is commits on remote not in local.
///
/// # Arguments
/// * `path` - Absolute path to the Git repository
///
/// # Returns
/// * `true` - Local branch is behind remote (needs pull)
/// * `false` - Up to date, ahead of remote, or check failed
fn is_behind_remote_raw(path: &str) -> bool {
    let branch = match get_current_branch(path) {
        Some(b) => b,
        None => {
            eprintln!("Warning: Could not determine current branch for {}", path);
            return false;
        }
    };

    // Query rev-list for commit counts between local and remote
    let output = Command::new("git")
        .args([
            "-C",
            path,
            "rev-list",
            "--left-right",
            "--count",
            &format!("{}...origin/{}", branch, branch),
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            // Parse output: "ahead\tbehind" (left=local commits not in origin, right=origin commits not in local)
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(tab_pos) = stdout.find('\t') {
                // Right side (after tab) shows commits in origin but not in local = behind count
                let behind = &stdout[tab_pos + 1..];
                behind.trim() != "0"
            } else {
                eprintln!(
                    "Warning: Unexpected git rev-list output format for {}: {:?}",
                    path, stdout
                );
                false
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!(
                "Warning: git rev-list failed for {}: {}",
                path,
                stderr.trim()
            );
            false
        }
        Err(e) => {
            eprintln!("Warning: Could not run git rev-list for {}: {}", path, e);
            false
        }
    }
}

/// Retrieves comprehensive repository status with caching support.
///
/// First checks the status cache for recent data. On cache miss, queries
/// the repository for all status information and stores it in cache.
///
/// # Arguments
/// * `path` - Absolute path to the Git repository
///
/// # Returns
/// Tuple of (is_clean, is_behind_remote, current_branch):
/// - `is_clean`: Whether working directory has uncommitted changes
/// - `is_behind_remote`: Whether local branch is behind its remote tracking branch
/// - `current_branch`: The current branch name, or None if detached HEAD
///
/// # Caching
/// Status information is cached for 5 minutes to improve performance.
/// Use `status_cache::clear_status_cache()` to force fresh checks.
pub fn get_repo_status(path: &str) -> (bool, bool, Option<String>) {
    // Check cache first
    if let Some(cached) = get_cached_status(path) {
        return (cached.clean, cached.behind, cached.branch);
    }

    // Cache miss - compute all statuses
    let clean = is_repo_clean_raw(path);
    let behind = is_behind_remote_raw(path);
    let branch = get_current_branch(path);

    // Store in cache
    set_cached_status(path, clean, behind, branch.clone());

    (clean, behind, branch)
}
