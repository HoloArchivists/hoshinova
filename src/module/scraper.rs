use super::{Message, Module, Task};
use crate::config;
use crate::msgbus::{BusRx, BusTx};
use anyhow::Result;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashSet;

pub struct RSS<'a> {
    channel: &'a config::ChannelConfig,
    config: &'a config::Config,
    url: String,
    client: Client,
}

#[derive(Deserialize, Debug)]
struct RSSFeed {
    #[serde(rename = "entry", default)]
    entries: Vec<FeedEntry>,
}

#[derive(Deserialize, Debug)]
struct FeedEntry {
    #[serde(rename = "videoId")]
    video_id: String,
    #[serde(rename = "channelId")]
    channel_id: String,
    title: String,
    author: Author,
    published: chrono::DateTime<chrono::Utc>,
    updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize, Debug)]
struct Author {
    name: String,
    uri: String,
}

impl<'a> RSS<'a> {
    pub fn new(config: &'a config::Config, index: usize) -> Self {
        let channel = &config.channel[index];
        let url = format!(
            "https://www.youtube.com/feeds/videos.xml?channel_id={}",
            channel.id
        );
        let client = Client::new();
        Self {
            channel,
            config,
            url,
            client,
        }
    }

    fn runloop(&self, scraped: &mut HashSet<String>) -> Result<Vec<Task>> {
        debug!("Fetching RSS for {}", self.channel.name);

        // Fetch the RSS feed
        let res = self.client.get(&self.url).send()?;
        let feed: RSSFeed = quick_xml::de::from_slice(&res.bytes()?)?;

        // Find matching videos
        Ok(feed
            .entries
            .iter()
            .filter_map(move |entry| {
                if self
                    .channel
                    .filters
                    .iter()
                    .any(|filter| filter.is_match(&entry.title))
                    && !scraped.contains(&entry.video_id)
                {
                    scraped.insert(entry.video_id.to_owned());
                    Some(Task {
                        title: entry.title.to_owned(),
                        video_id: entry.video_id.to_owned(),
                        channel_name: entry.author.name.to_owned(),
                        channel_id: entry.channel_id.to_owned(),
                    })
                } else {
                    None
                }
            })
            .collect())
    }
}

impl<'a> Module for RSS<'a> {
    fn run(&self, tx: &BusTx<Message>, rx: &mut BusRx<Message>) -> Result<()> {
        let mut scraped = HashSet::<String>::new();
        loop {
            match self.runloop(&mut scraped) {
                Ok(tasks) => {
                    for task in tasks {
                        tx.send(Message::ToRecord(task))?;
                    }
                }
                Err(e) => {
                    error!("Error scraping channel {}: {}", self.channel.name, e);
                }
            }

            // Sleep
            if rx.wait_until_closed(self.config.scraper.rss.poll_interval) {
                debug!("Stopped scraping RSS for {}", self.channel.name);
                return Ok(());
            }
        }
    }
}
