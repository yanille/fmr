use crate::git::get_repo_status;
use dialoguer::Select;
use rayon::prelude::*;
use std::path::PathBuf;
use std::process::Command;

pub fn repo_name(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn format_repo_with_status(path: &str) -> String {
    let name = repo_name(path);
    let (clean, behind, _branch) = get_repo_status(path);

    // ANSI color codes
    let green = "\x1b[32m";
    let orange = "\x1b[33m";
    let red = "\x1b[31m";
    let reset = "\x1b[0m";

    let status_indicator = if !clean {
        format!("{}●{} ", red, reset)
    } else if behind {
        format!("{}●{} ", orange, reset)
    } else {
        format!("{}●{} ", green, reset)
    };

    format!("{}{}", status_indicator, name)
}

fn format_repo_simple(path: &str) -> String {
    repo_name(path)
}

pub fn open_repo_in_vscode(path: &str) {
    let status = Command::new("code").arg(path).status();

    match status {
        Ok(_) => println!("Opened {} in VS Code", path),
        Err(e) => println!("Failed to open {}: {}", path, e),
    }
}

pub fn interactive_repo_menu(repos: &Vec<String>) {
    if repos.is_empty() {
        println!("No repositories found.");
        return;
    }

    // For large repos list, skip status indicators to maintain performance
    // Only compute statuses for smaller lists (e.g., < 30 repos)
    let use_status = repos.len() <= 30;

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

pub fn search_and_select(repos: &Vec<String>, query: &str) {
    let matches: Vec<_> = repos
        .iter()
        .filter(|r| repo_name(r).to_lowercase().contains(&query.to_lowercase()))
        .collect();

    if matches.is_empty() {
        println!("No repos found for '{}'.", query);
        return;
    }

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
