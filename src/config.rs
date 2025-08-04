use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub port: u16,
    pub data_dir: Option<String>,
    pub bootstrap_peers: Vec<String>,
    pub security: crate::security::SecurityConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 4001,
            data_dir: None,
            bootstrap_peers: Vec::new(),
            security: crate::security::SecurityConfig::default(),
        }
    }
}

pub fn load_config(path: &Path) -> Result<Config> {
    if path.exists() {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    } else {
        Ok(Config::default())
    }
}

pub fn save_config(path: &Path, config: &Config) -> Result<()> {
    let content = toml::to_string_pretty(config)?;
    fs::write(path, content)?;
    Ok(())
}
