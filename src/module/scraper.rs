use super::{Message, Module, Task};
use crate::config;
use anyhow::Result;
use bus::BusReader;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::sync::mpsc;
use std::thread;

pub struct Scraper<'a> {
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

impl<'a> Scraper<'a> {
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

    fn runloop(&self) -> Result<Vec<Task>> {
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
                {
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

impl<'a> Module for Scraper<'a> {
    fn run(&self, send: mpsc::SyncSender<Message>, recv: &mut BusReader<Message>) -> Result<()> {
        loop {
            if let Ok(message) = recv.try_recv() {
                if let Message::Quit = message {
                    return Ok(());
                }
            }
            match self.runloop() {
                Ok(tasks) => {
                    for task in tasks {
                        send.send(Message::Task(task))?;
                    }
                }
                Err(e) => {
                    error!("Error scraping channel {}: {}", self.channel.name, e);
                }
            }

            // Sleep
            thread::sleep(self.config.scraper.rss.poll_interval);
        }
    }
}
