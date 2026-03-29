//! User interface and interaction handling.
//!
//! This module provides functions for interactive repository selection,
//! status display formatting, and opening repositories in VS Code.
//!
//! # Features
//!
//! - Interactive menu using `dialoguer` for repository selection
//! - Color-coded status indicators (green/orange/red circles)
//! - Parallel status computation for performance
//! - Automatic VS Code integration

use crate::git::get_repo_status;
use dialoguer::Select;
use rayon::prelude::*;
use std::path::PathBuf;
use std::process::Command;

/// Threshold for skipping status indicators in large repository lists.
///
/// When the repository count exceeds this value, status indicators are
/// disabled to maintain UI responsiveness.
const STATUS_COMPUTE_THRESHOLD: usize = 30;

/// Extracts the repository name from a full path.
///
/// Returns the final directory component of the path. If the path
/// ends with a separator or has no file name, returns the full path.
///
/// # Arguments
/// * `path` - Full absolute path to the repository
///
/// # Returns
/// The repository directory name or the full path if extraction fails.
///
/// # Examples
/// ```
/// let name = repo_name("/home/user/projects/my-app");
/// assert_eq!(name, "my-app");
/// ```
pub fn repo_name(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// Formats a repository path with status indicators and branch name.
///
/// Creates a display string with:
/// - Color-coded status indicator (●)
///   - Green: Clean and up-to-date
///   - Orange: Behind remote (needs pull)
///   - Red: Has uncommitted changes
/// - Repository name
/// - Current branch name (if available)
///
/// # Arguments
/// * `path` - Absolute path to the repository
///
/// # Returns
/// ANSI-colored formatted string for display in terminal.
fn format_repo_with_status(path: &str) -> String {
    let name = repo_name(path);
    let (clean, behind, branch) = get_repo_status(path);

    // ANSI color codes for terminal output
    let green = "\x1b[32m"; // Clean
    let orange = "\x1b[33m"; // Behind
    let red = "\x1b[31m"; // Dirty
    let cyan = "\x1b[36m"; // Branch name
    let reset = "\x1b[0m"; // Reset

    // Determine status indicator color
    let status_indicator = if !clean {
        format!("{}●{} ", red, reset) // Red: uncommitted changes
    } else if behind {
        format!("{}●{} ", orange, reset) // Orange: behind remote
    } else {
        format!("{}●{} ", green, reset) // Green: clean and up-to-date
    };

    // Format branch name if available
    let branch_str = match branch {
        Some(b) => format!(" {}[{}]{}", cyan, b, reset),
        None => String::new(),
    };

    format!("{}{}{}", status_indicator, name, branch_str)
}

/// Simple repository name formatter without status indicators.
///
/// Used as a fallback for large repository lists where computing
/// status for all entries would be too slow.
///
/// # Arguments
/// * `path` - Absolute path to the repository
///
/// # Returns
/// Repository name only, no status information.
fn format_repo_simple(path: &str) -> String {
    repo_name(path)
}

/// Opens a repository in Visual Studio Code.
///
/// Executes the `code` command with the repository path as argument.
/// Requires VS Code to be installed and `code` command available in PATH.
///
/// # Arguments
/// * `path` - Absolute path to the repository to open
///
/// # Output
/// Prints success or failure message to stdout.
pub fn open_repo_in_vscode(path: &str) {
    let status = Command::new("code").arg(path).status();

    match status {
        Ok(_) => println!("Opened {} in VS Code", path),
        Err(e) => println!("Failed to open {}: {}", path, e),
    }
}

/// Displays an interactive menu for repository selection.
///
/// Shows all repositories in a scrollable list using `dialoguer::Select`.
/// For large repository lists (>30), skips status indicators to maintain
/// performance. Uses parallel processing for status computation when enabled.
///
/// # Arguments
/// * `repos` - Vector of repository paths to display
///
/// # Behavior
/// - If no repositories exist, prints a message and returns
/// - User selects with arrow keys and Enter
/// - Ctrl+C or Esc exits without opening
/// - Selection opens the repository in VS Code
///
/// # Performance
/// - Parallel status computation using Rayon
/// - Falls back to simple names for large lists
pub fn interactive_repo_menu(repos: &Vec<String>) {
    if repos.is_empty() {
        println!("No repositories found.");
        return;
    }

    // For large repos list, skip status indicators to maintain performance
    // Only compute statuses for smaller lists to keep UI responsive
    let use_status = repos.len() <= STATUS_COMPUTE_THRESHOLD;

    let names: Vec<String> = if use_status {
        // Compute statuses in parallel for better performance
        repos
            .par_iter()
            .map(|r| format_repo_with_status(r))
            .collect()
    } else {
        repos.iter().map(|r| format_repo_simple(r)).collect()
    };

    let result = Select::new()
        .with_prompt("Select a repository (Ctrl+C to exit)")
        .items(&names)
        .default(0)
        .interact();

    match result {
        Ok(selection) => open_repo_in_vscode(&repos[selection]),
        Err(_) => {
            println!("\nExiting...");
        }
    }
}

/// Searches repositories by name and displays matching results.
///
/// Performs case-insensitive substring matching on repository names.
/// If exactly one match is found, opens it immediately. Otherwise,
/// displays an interactive selection menu with status indicators.
///
/// # Arguments
/// * `repos` - Full list of repository paths
/// * `query` - Search query string (case-insensitive)
///
/// # Behavior
/// - Filters repositories where name contains the query (case-insensitive)
/// - No matches: prints "No repos found" message
/// - Single match: opens immediately in VS Code
/// - Multiple matches: shows interactive menu with status indicators
///
/// # Performance
/// Always shows status indicators for search results (typically small sets).
/// Uses parallel processing for status computation.
pub fn search_and_select(repos: &Vec<String>, query: &str) {
    let matches: Vec<_> = repos
        .iter()
        .filter(|r| repo_name(r).to_lowercase().contains(&query.to_lowercase()))
        .collect();

    if matches.is_empty() {
        println!("No repos found for '{}'.", query);
        return;
    }

    // Single match - open immediately
    if matches.len() == 1 {
        open_repo_in_vscode(matches[0]);
        return;
    }

    // Search results are typically small, so always show status indicators
    // Use parallel processing for faster status computation
    let names: Vec<String> = matches
        .par_iter()
        .map(|r| format_repo_with_status(r))
        .collect();

    let result = Select::new()
        .with_prompt("Select a repository (Ctrl+C to exit)")
        .items(&names)
        .default(0)
        .interact();

    match result {
        Ok(selection) => open_repo_in_vscode(matches[selection]),
        Err(_) => {
            println!("\nExiting...");
        }
    }
}
