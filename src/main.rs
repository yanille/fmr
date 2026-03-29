mod cache;
mod commands;
mod config;
mod git;
mod status_cache;
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
    /// Upgrade fmr to the latest version
    Upgrade,

    /// Downgrade fmr to a specific version
    Downgrade { version: String },

    /// Refresh caches (repos, status, or all)
    #[command(subcommand)]
    Refresh(RefreshCommands),

    /// Manage scan locations
    #[command(subcommand)]
    Locations(LocationCommands),

    /// Sync repositories by pulling latest changes
    Sync {
        /// Sync all repositories
        #[arg(long)]
        all: bool,

        /// Repository name to sync
        repo: Option<String>,
    },
}

#[derive(Subcommand)]
enum RefreshCommands {
    /// Rebuild the repo list cache
    #[command(alias = "repos")]
    List,

    /// Clear the git status cache
    Status,

    /// Refresh both repo list and status cache
    All,
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
        Some(Commands::Upgrade) => commands::upgrade_fmr(),

        Some(Commands::Downgrade { version }) => commands::downgrade_fmr(version),

        Some(Commands::Refresh(cmd)) => match cmd {
            RefreshCommands::List => commands::refresh_repos(),
            RefreshCommands::Status => commands::refresh_status(),
            RefreshCommands::All => commands::refresh_all(),
        },

        Some(Commands::Locations(cmd)) => match cmd {
            LocationCommands::Add { path } => commands::add_location(path.clone()),
            LocationCommands::Remove { path } => commands::remove_location(path.clone()),
            LocationCommands::List => commands::list_locations(),
        },

        Some(Commands::Sync { all, repo }) => {
            let repos = cache::load_or_create_cache();
            commands::sync_repos(&repos, *all, repo.clone());
        }

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
