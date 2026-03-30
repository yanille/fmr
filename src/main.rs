//! Find My Repo (fmr) - A fast CLI for finding and opening local Git repositories.
//!
//! This is the main entry point for the fmr application. It handles:
//! - CLI argument parsing using clap
//! - Command routing and dispatch
//! - Default behavior (search/interactive mode when no subcommand is provided)
//!
//! The application uses a cache-based architecture for fast repository lookups
//! and memory-mapped status caching for optimal performance.

mod cache;
mod commands;
mod config;
mod git;
mod status_cache;
mod ui;

use clap::{Parser, Subcommand};

/// Command-line interface for the fmr application.
///
/// This struct defines all CLI arguments and subcommands using clap's derive macros.
/// When no subcommand is provided, fmr enters interactive search mode.
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

/// Available subcommands for the fmr CLI.
///
/// Each variant represents a distinct operation that can be performed
/// on the repository cache or the fmr installation itself.
#[derive(Subcommand)]
enum Commands {
    /// Upgrade fmr to the latest version from GitHub releases.
    ///
    /// Downloads and installs the latest release automatically.
    /// May require sudo if installed in a system directory.
    Upgrade,

    /// Downgrade fmr to a specific version.
    ///
    /// Allows installing an older release by specifying the version number.
    /// Example: `fmr downgrade 0.1.0`
    Downgrade {
        /// Version string in format "x.y.z"
        version: String,
    },

    /// Refresh caches (repos, status, or all).
    ///
    /// Repos cache rescans configured locations.
    /// Status cache clears git status information.
    /// All performs both operations.
    #[command(subcommand)]
    Refresh(RefreshCommands),

    /// Manage scan locations.
    ///
    /// Add, remove, or list directories that fmr will scan for repositories.
    #[command(subcommand)]
    Locations(LocationCommands),
}

#[derive(Subcommand)]
enum RefreshCommands {
    /// Rebuild the repository list cache.
    ///
    /// Rescans all configured locations and rebuilds the index.
    /// Alias: `repos`
    #[command(alias = "repos")]
    List,

    /// Clear the git status cache.
    ///
    /// Forces fresh git status checks on next display.
    /// Status cache has a 5-minute TTL by default.
    Status,

    /// Refresh both repository list and status cache.
    ///
    /// Equivalent to running both `refresh list` and `refresh status`.
    All,
}

#[derive(Subcommand)]
enum LocationCommands {
    /// Add a new directory to scan for repositories.
    ///
    /// The path must exist and be a directory.
    /// Duplicate locations are automatically ignored.
    Add {
        /// Absolute or relative path to the directory
        path: String,
    },

    /// Remove a directory from the scan locations.
    ///
    /// The path is matched against configured locations.
    Remove {
        /// Path as previously added (will be canonicalized)
        path: String,
    },

    /// List all configured scan locations.
    ///
    /// Shows each location with an indicator of whether it exists.
    List,
}

/// Main entry point for the fmr application.
///
/// Parses CLI arguments and dispatches to appropriate command handlers.
/// When no subcommand is provided, enters interactive search mode.
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
