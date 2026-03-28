use clap::{Parser, Subcommand};
use dialoguer::Select;
use dirs;
use rayon::prelude::*;
use self_update::backends::github::Update;
use serde::{Deserialize, Serialize};
use serde_json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

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
    Downgrade { version: String },

    /// Rebuild the repo cache
    Refresh,

    /// Manage scan locations
    #[command(subcommand)]
    Locations(LocationCommands),
}

#[derive(Subcommand)]
enum LocationCommands {
    /// Add a new location to scan
    Add { path: String },

    /// Remove a location from scanning
    Remove { path: String },

    /// List all configured scan locations
    List,
}

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    locations: Vec<String>,
}

impl Config {
    fn default_locations() -> Vec<String> {
        let home = dirs::home_dir().expect("Cannot find home directory");
        vec![home.join("Desktop").display().to_string()]
    }

    fn default() -> Self {
        Config {
            locations: Self::default_locations(),
        }
    }
}

fn fmr_dir() -> PathBuf {
    let mut path = dirs::home_dir().expect("Cannot find home directory");
    path.push(".fmr");
    fs::create_dir_all(&path).ok();
    path
}

fn cache_path() -> PathBuf {
    let mut path = fmr_dir();
    path.push("repos.json");
    path
}

fn config_path() -> PathBuf {
    let mut path = fmr_dir();
    path.push("config.json");
    path
}

fn load_or_create_config() -> Config {
    let config_file = config_path();

    if config_file.exists() {
        let data = fs::read_to_string(&config_file).unwrap_or_default();

        if data.trim().is_empty() {
            let config = Config::default();
            save_config(&config);
            return config;
        }

        match serde_json::from_str(&data) {
            Ok(config) => config,
            Err(_) => {
                let config = Config::default();
                save_config(&config);
                config
            }
        }
    } else {
        let config = Config::default();
        save_config(&config);
        config
    }
}

fn save_config(config: &Config) {
    let config_file = config_path();
    let json = serde_json::to_string_pretty(config).unwrap();
    fs::write(config_file, json).unwrap();
}

fn add_location(path: String) {
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

fn remove_location(path: String) {
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

fn list_locations() {
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

fn scan_repos() -> Vec<String> {
    let config = load_or_create_config();
    let mut all_repos = Vec::new();

    for location in &config.locations {
        let path = PathBuf::from(location);

        if !path.exists() || !path.is_dir() {
            continue;
        }

        let dirs: Vec<_> = WalkDir::new(&path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_dir())
            .map(|e| e.path().to_path_buf())
            .collect();

        let repos: Vec<String> = dirs
            .par_iter()
            .filter(|path| path.join(".git").is_dir())
            .map(|path| path.display().to_string())
            .collect();

        all_repos.extend(repos);
    }

    all_repos.sort();
    all_repos.dedup();
    all_repos
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
    let status = Command::new("code").arg(path).status();

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

    match &cli.command {
        Some(Commands::Update) => update_fmr(),

        Some(Commands::Downgrade { version }) => downgrade_fmr(version),

        Some(Commands::Refresh) => {
            let repos = scan_repos();
            let json = serde_json::to_string(&repos).unwrap();
            fs::write(cache_path(), json).unwrap();
            println!("Indexed {} repositories", repos.len());
        }

        Some(Commands::Locations(cmd)) => match cmd {
            LocationCommands::Add { path } => add_location(path.clone()),
            LocationCommands::Remove { path } => remove_location(path.clone()),
            LocationCommands::List => list_locations(),
        },

        None => {
            let repos = load_or_create_cache();
            if let Some(query) = cli.query {
                search_and_select(&repos, &query);
            } else {
                interactive_repo_menu(&repos);
            }
        }
    }
}
