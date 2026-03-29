use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub locations: Vec<String>,
}

impl Config {
    fn default_locations() -> Vec<String> {
        let home = dirs::home_dir().expect("Cannot find home directory");
        vec![home.join("Desktop").display().to_string()]
    }

    pub fn default() -> Self {
        Config {
            locations: Self::default_locations(),
        }
    }
}

pub fn fmr_dir() -> PathBuf {
    let mut path = dirs::home_dir().expect("Cannot find home directory");
    path.push(".fmr");
    fs::create_dir_all(&path).ok();
    path
}

pub fn config_path() -> PathBuf {
    let mut path = fmr_dir();
    path.push("config.json");
    path
}

pub fn load_or_create_config() -> Config {
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

pub fn save_config(config: &Config) {
    let config_file = config_path();
    let json = serde_json::to_string_pretty(config).unwrap();
    fs::write(config_file, json).unwrap();
}
