mod cache;
mod commands;
mod config;
mod git;
mod ui;

use clap::{Parser, Subcommand};

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

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Update) => commands::update_fmr(),

        Some(Commands::Downgrade { version }) => commands::downgrade_fmr(version),

        Some(Commands::Refresh) => {
            let repos = cache::scan_repos();
            let json = serde_json::to_string(&repos).unwrap();
            std::fs::write(cache::cache_path(), json).unwrap();
            println!("Indexed {} repositories", repos.len());
        }

        Some(Commands::Locations(cmd)) => match cmd {
            LocationCommands::Add { path } => commands::add_location(path.clone()),
            LocationCommands::Remove { path } => commands::remove_location(path.clone()),
            LocationCommands::List => commands::list_locations(),
        },

        None => {
            let repos = cache::load_or_create_cache();
            if let Some(query) = cli.query {
                ui::search_and_select(&repos, &query);
            } else {
                ui::interactive_repo_menu(&repos);
            }
        }
    }
}
