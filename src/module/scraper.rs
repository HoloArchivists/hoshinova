use super::{Message, Module, Task};
use crate::config;
use anyhow::Result;
use async_trait::async_trait;
use futures::stream::FuturesUnordered;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashSet;
use tokio::sync::mpsc;

pub struct RSS<'a> {
    config: &'a config::Config,
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
    fn run_one(
        &self,
        scraped: &mut HashSet<String>,
        channel: &config::ChannelConfig,
    ) -> Result<Vec<Task>> {
        debug!("Fetching RSS for {}", channel.name);

        // Fetch the RSS feed
        let url = format!(
            "https://www.youtube.com/feeds/videos.xml?channel_id={}",
            channel.id
        );
        let res = self.client.get(&url).send()?;
        let feed: RSSFeed = quick_xml::de::from_slice(&res.bytes()?)?;

        // Find matching videos
        Ok(feed
            .entries
            .iter()
            .filter_map(move |entry| {
                if channel
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
                        output_directory: channel.outpath.clone(),
                    })
                } else {
                    None
                }
            })
            .collect())
    }

    fn run_loop(&self, mut scraped: &mut HashSet<String>) -> Vec<Task> {
        self.config
            .channel
            .iter()
            .flat_map(|channel| {
                self.run_one(&mut scraped, channel).unwrap_or_else(|e| {
                    error!("Failed to run RSS for {}: {}", channel.name, e);
                    vec![]
                })
            })
            .collect()
    }
}

#[async_trait]
impl<'a> Module<'a> for RSS<'a> {
    fn new(config: &'a config::Config) -> Self {
        let client = Client::new();
        Self { config, client }
    }

    async fn run(
        &self,
        tx: &mpsc::Sender<Message>,
        rx: &mut mpsc::Receiver<Message>,
    ) -> Result<()> {
        let mut scraped = HashSet::<String>::new();
        loop {
            if futures::future::join_all(self.run_loop(&mut scraped).iter().map(|task| async {
                info!(
                    "[{}] [{}] Found new matching video: {}",
                    task.video_id, task.channel_name, task.title,
                );
                tx.send(Message::ToRecord(task.clone())).await.is_ok()
            }))
            .await
            .iter()
            .any(|x| !x)
            {
                debug!("Failed to send message to bus");
                break;
            }

            // Sleep
            // tokio::sync::

            // if rx.wait_until_closed(self.config.scraper.rss.poll_interval) {
            // debug!("Stopped scraping RSS");
            // break;
            // }
        }
        Ok(())
    }
}
