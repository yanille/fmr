use clap::{Parser, Subcommand};
use walkdir::WalkDir;
use dirs;
use std::path::PathBuf;
use std::fs;
use std::process::Command;
use serde_json;
use dialoguer::Select;
use self_update::backends::github::Update;

/// Find My Repo
#[derive(Parser)]
#[command(name = "fmr", version)]
#[command(about = "Find My Repo", long_about = None)]
struct Cli {
    /// Optional subcommand
    #[command(subcommand)]
    command: Option<Commands>,

    /// Optional search query (if no subcommand is used)
    query: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Update fmr to the latest version
    Update,

    /// Downgrade fmr to a specific version
    Downgrade { 
        version: String
    },

    /// Rebuild the repo cache
    Refresh,
}

fn cache_path() -> PathBuf {
    let mut path = dirs::home_dir().expect("Cannot find home directory");
    path.push(".fmr");
    fs::create_dir_all(&path).ok();
    path.push("repos.json");
    path
}

fn scan_repos() -> Vec<String> {
    let home = dirs::home_dir().expect("Cannot find home directory");
    let desktop = home.join("Desktop");
    let mut repos = Vec::new();

    if !desktop.exists() {
        return repos;
    }

    for entry in WalkDir::new(&desktop)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
    {
        let path = entry.path();
        if path.join(".git").is_dir() {
            repos.push(path.display().to_string());
        }
    }

    repos
}

fn load_or_create_cache() -> Vec<String> {
    let cache = cache_path();

    if cache.exists() {
        let data = fs::read_to_string(&cache).unwrap_or_default();

        if data.trim().is_empty() {
            return rebuild_cache(cache);
        }

        match serde_json::from_str(&data) {
            Ok(repos) => repos,
            Err(_) => rebuild_cache(cache),
        }
    } else {
        rebuild_cache(cache)
    }
}

fn rebuild_cache(cache: PathBuf) -> Vec<String> {
    println!("Building repo cache...\n");

    let repos = scan_repos();
    let json = serde_json::to_string(&repos).unwrap();

    fs::write(cache, json).unwrap();

    println!("Indexed {} repositories\n", repos.len());
    repos
}

fn open_repo_in_vscode(path: &str) {
    let status = Command::new("code")
        .arg(path)
        .status();

    match status {
        Ok(_) => println!("Opened {} in VS Code", path),
        Err(e) => println!("Failed to open {}: {}", path, e),
    }
}

fn repo_name(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn interactive_repo_menu(repos: &Vec<String>) {
    if repos.is_empty() {
        println!("No repositories found.");
        return;
    }

    let names: Vec<String> = repos.iter().map(|r| repo_name(r)).collect();

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

fn search_and_select(repos: &Vec<String>, query: &str) {
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

    let names: Vec<String> = matches.iter().map(|r| repo_name(r)).collect();

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

fn update_fmr() {
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
            println!("✅ Updated to version {}", status.version());
        }
        Err(e) => {
            eprintln!("❌ Update failed: {}", e);
            eprintln!("If installed in a system directory you may need:");
            eprintln!("sudo fmr update");
        }
    }
}

fn downgrade_fmr(version: &str) {
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

fn main() {
    let cli = Cli::parse();
    let repos = load_or_create_cache();

    match &cli.command {
        Some(Commands::Update) => update_fmr(),

        Some(Commands::Downgrade { version }) => downgrade_fmr(version),

        Some(Commands::Refresh) => {
            let repos = scan_repos();
            let json = serde_json::to_string(&repos).unwrap();
            fs::write(cache_path(), json).unwrap();
            println!("Indexed {} repositories", repos.len());
        }

        None => {
            if let Some(query) = cli.query {
                search_and_select(&repos, &query);
            } else {
                interactive_repo_menu(&repos);
            }
        }
    }
}