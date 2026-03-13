use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::rss::types::Feed;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub download_dir: PathBuf,
    pub default_speed: f32,
    pub default_volume: u8,
    pub max_episodes_per_feed: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            download_dir: dirs::audio_dir()
                .or_else(dirs::home_dir)
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Podcasts"),
            default_speed: 1.0,
            default_volume: 80,
            max_episodes_per_feed: 50,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub feeds: Vec<Feed>,
    pub settings: Settings,
}

pub fn config_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .context("Could not determine config directory")?
        .join("podcast-tui");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("config.json"))
}

pub fn load() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let data = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config: {}", path.display()))?;
    let config: Config = serde_json::from_str(&data)
        .with_context(|| "Failed to parse config JSON")?;
    Ok(config)
}

pub fn save(config: &Config) -> Result<()> {
    let path = config_path()?;
    let tmp = path.with_extension("tmp");
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}
