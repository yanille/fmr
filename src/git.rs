use crate::status_cache::{get_cached_status, set_cached_status};
use std::process::Command;

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

fn is_repo_clean_raw(path: &str) -> bool {
    let output = Command::new("git")
        .args(["-C", path, "status", "--porcelain"])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().is_empty()
        }
        _ => true, // Assume clean if we can't check
    }
}

fn is_behind_remote_raw(path: &str) -> bool {
    let branch = match get_current_branch(path) {
        Some(b) => b,
        None => return false,
    };

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

/// Get all git status info with caching support
/// Returns (clean, behind, branch)
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

/// Check if repo is clean (cached version)
#[allow(dead_code)]
pub fn is_repo_clean(path: &str) -> bool {
    get_repo_status(path).0
}

/// Check if repo is behind remote (cached version)
#[allow(dead_code)]
pub fn is_behind_remote(path: &str) -> bool {
    get_repo_status(path).1
}

/// Pull latest changes from remote
/// Returns true if successful, false otherwise
pub fn pull_repo(path: &str) -> bool {
    let output = Command::new("git").args(["-C", path, "pull"]).output();

    match output {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// Check if a branch exists in the repository
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

/// Checkout a branch in the repository
/// Returns true if successful, false otherwise
pub fn checkout_branch(path: &str, branch: &str) -> bool {
    let output = Command::new("git")
        .args(["-C", path, "checkout", branch])
        .output();

    match output {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}
