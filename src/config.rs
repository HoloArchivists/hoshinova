use crate::module::TaskStatus;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Config {
    pub ytarchive: YtarchiveConfig,
    pub scraper: ScraperConfig,
    pub notifier: Option<NotifierConfig>,
    pub webserver: Option<WebserverConfig>,
    pub channel: Vec<ChannelConfig>,

    #[serde(skip)]
    config_path: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct YtarchiveConfig {
    pub executable_path: String,
    pub working_directory: String,
    pub args: Vec<String>,
    pub quality: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ScraperConfig {
    pub rss: ScraperRSSConfig,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ScraperRSSConfig {
    #[serde(with = "humantime_serde")]
    pub poll_interval: std::time::Duration,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NotifierConfig {
    pub discord: Option<NotifierDiscordConfig>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NotifierDiscordConfig {
    pub webhook_url: String,
    pub notify_on: Vec<TaskStatus>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct WebserverConfig {
    pub bind_address: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChannelConfig {
    pub id: String,
    pub name: String,
    #[serde(with = "serde_regex")]
    pub filters: Vec<regex::Regex>,
    pub outpath: String,
    /// If not present, will be fetched during runtime.
    pub picture_url: Option<String>,
}

pub fn load_config(path: &str) -> Result<Config> {
    let config = std::fs::read_to_string(path)?;
    let mut config: Config = toml::from_str(&config)?;
    config.config_path = path.to_string();
    Ok(config)
}

impl Config {
    /// Reads the config file and replaces the current config with the new one.
    pub fn reload(&mut self) -> Result<()> {
        info!("Reloading config");
        let config = load_config(&self.config_path)?;
        *self = config;
        Ok(())
    }
}
