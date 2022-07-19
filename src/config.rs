use anyhow::Result;
use serde::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct Config {
    pub ytarchive: YtarchiveConfig,
    pub scraper: ScraperConfig,
    pub channel: Vec<ChannelConfig>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct YtarchiveConfig {
    pub executable_path: String,
    pub working_directory: String,
    pub args: Vec<String>,
    pub quality: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct ScraperConfig {
    pub rss: ScraperRSSConfig,
}

#[derive(Clone, Deserialize, Debug)]
pub struct ScraperRSSConfig {
    #[serde(with = "humantime_serde")]
    pub poll_interval: std::time::Duration,
}

#[derive(Clone, Deserialize, Debug)]
pub struct ChannelConfig {
    pub id: String,
    pub name: String,
    #[serde(with = "serde_regex")]
    pub filters: Vec<regex::Regex>,
    pub outpath: String,
}

pub fn load_config(path: &str) -> Result<Config> {
    let config = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&config)?;
    Ok(config)
}
