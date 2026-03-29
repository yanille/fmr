use crate::git::{is_behind_remote, is_repo_clean};
use dialoguer::Select;
use std::path::PathBuf;
use std::process::Command;

pub fn repo_name(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

pub fn format_repo_display(path: &str) -> String {
    let name = repo_name(path);
    let clean = is_repo_clean(path);
    let behind = is_behind_remote(path);

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

    let names: Vec<String> = repos.iter().map(|r| format_repo_display(r)).collect();

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

    let names: Vec<String> = matches.iter().map(|r| format_repo_display(r)).collect();

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
