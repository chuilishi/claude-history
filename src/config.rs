use crate::error::{AppError, Result};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Deserialize, Debug, Default)]
pub struct ConfigFile {
    pub resume: Option<ResumeConfig>,
}

#[derive(Deserialize, Debug, Default)]
pub struct ResumeConfig {
    pub default_args: Option<Vec<String>>,
}

fn get_config_path() -> Option<PathBuf> {
    home::home_dir().map(|mut path| {
        path.push(".config");
        path.push("claude-history");
        path.push("config.toml");
        path
    })
}

pub fn load_config() -> Result<ConfigFile> {
    let config_path = match get_config_path() {
        Some(path) => path,
        None => return Ok(ConfigFile::default()),
    };

    if !config_path.exists() {
        return Ok(ConfigFile::default());
    }

    let content = fs::read_to_string(&config_path).map_err(|e| {
        AppError::ConfigError(format!(
            "Failed to read config file at '{}': {}",
            config_path.display(),
            e
        ))
    })?;

    toml::from_str(&content).map_err(|e| {
        AppError::ConfigError(format!(
            "Failed to parse config file at '{}': {}",
            config_path.display(),
            e
        ))
    })
}
