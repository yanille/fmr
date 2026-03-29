//! Git operations and status checking.
//!
//! This module provides functions for interacting with Git repositories,
//! including checking repository status, pulling changes, and switching branches.
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
        None => return false,
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
            // Parse output: "behind\tahead"
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(tab_pos) = stdout.find('\t') {
                let behind = &stdout[..tab_pos];
                behind.trim() != "0"
            } else {
                false
            }
        }
        _ => false,
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

/// Pulls latest changes from the remote repository.
///
/// Runs `git pull` in the specified repository.
///
/// # Arguments
/// * `path` - Absolute path to the Git repository
///
/// # Returns
/// * `true` - Pull completed successfully
/// * `false` - Pull failed or git command error
///
/// # Note
/// This function does not check for uncommitted changes or merge conflicts.
/// Repositories should be checked as clean before calling this.
pub fn pull_repo(path: &str) -> bool {
    let output = Command::new("git").args(["-C", path, "pull"]).output();

    match output {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// Checks if a specific branch exists in the repository.
///
/// Runs `git branch --list <branch>` to check for branch existence.
///
/// # Arguments
/// * `path` - Absolute path to the Git repository
/// * `branch` - Name of the branch to check
///
/// # Returns
/// * `true` - Branch exists in the repository
/// * `false` - Branch does not exist or check failed
///
/// # Note
/// This checks for local branches only, not remote-tracking branches.
pub fn branch_exists(path: &str, branch: &str) -> bool {
    let output = Command::new("git")
        .args(["-C", path, "branch", "--list", branch])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            !String::from_utf8_lossy(&output.stdout).trim().is_empty()
        }
        _ => false,
    }
}

/// Switches to a specified branch in the repository.
///
/// Runs `git checkout <branch>` in the specified repository.
///
/// # Arguments
/// * `path` - Absolute path to the Git repository
/// * `branch` - Name of the branch to checkout
///
/// # Returns
/// * `true` - Checkout completed successfully
/// * `false` - Checkout failed (branch doesn't exist, uncommitted changes, etc.)
///
/// # Note
/// This function does not verify the branch exists first. Call `branch_exists()`
/// if you need to check before attempting checkout.
pub fn checkout_branch(path: &str, branch: &str) -> bool {
    let output = Command::new("git")
        .args(["-C", path, "checkout", branch])
        .output();

    match output {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}
