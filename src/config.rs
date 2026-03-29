//! Configuration management for fmr.
//!
//! This module handles reading and writing the fmr configuration file
//! located at `~/.fmr/config.json`. Configuration includes the list of
//! directories to scan for Git repositories.
//!
//! Default configuration:
//! - Scan location: `~/Desktop`

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Application configuration structure.
///
/// Stores user preferences including scan locations.
/// Serialized to JSON at `~/.fmr/config.json`.
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub locations: Vec<String>,
}

impl Config {
    /// Returns the default scan locations.
    ///
    /// Default is the user's Desktop directory.
    fn default_locations() -> Vec<String> {
        let home = dirs::home_dir().expect("Cannot find home directory");
        vec![home.join("Desktop").display().to_string()]
    }

    /// Creates a new configuration with default values.
    pub fn default() -> Self {
        Config {
            locations: Self::default_locations(),
        }
    }
}

/// Returns the path to the fmr configuration directory.
///
/// Creates the directory at `~/.fmr` if it doesn't exist.
pub fn fmr_dir() -> PathBuf {
    let mut path = dirs::home_dir().expect("Cannot find home directory");
    path.push(".fmr");
    fs::create_dir_all(&path).ok();
    path
}

/// Returns the path to the configuration file.
///
/// Configuration is stored at `~/.fmr/config.json`.
pub fn config_path() -> PathBuf {
    let mut path = fmr_dir();
    path.push("config.json");
    path
}

/// Loads the configuration from disk, or creates default if missing/invalid.
///
/// Attempts to read and parse the config file. If the file doesn't exist,
/// is empty, or contains invalid JSON, creates and saves default configuration.
///
/// # Returns
/// The loaded or newly created configuration.
pub fn load_or_create_config() -> Config {
    let config_file = config_path();

    if config_file.exists() {
        let data = fs::read_to_string(&config_file).unwrap_or_default();

        // Empty file - create default
        if data.trim().is_empty() {
            let config = Config::default();
            save_config(&config);
            return config;
        }

        // Parse or fall back to default on error
        match serde_json::from_str(&data) {
            Ok(config) => config,
            Err(_) => {
                let config = Config::default();
                save_config(&config);
                config
            }
        }
    } else {
        // No config exists - create default
        let config = Config::default();
        save_config(&config);
        config
    }
}

/// Saves the configuration to disk.
///
/// Serializes the config to pretty-printed JSON and writes to the
/// configuration file.
///
/// # Panics
/// Panics if JSON serialization or file write fails.
pub fn save_config(config: &Config) {
    let config_file = config_path();
    let json = serde_json::to_string_pretty(config).unwrap();
    fs::write(config_file, json).unwrap();
}
