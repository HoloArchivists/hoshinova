use super::{Message, Module, Task};
use crate::{config, msgbus::BusTx, youtube, APP_USER_AGENT};
use anyhow::{Context, Result};
use async_trait::async_trait;
use futures::stream::{self, Stream, StreamExt};
use reqwest::Client;
use serde::Deserialize;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, RwLock};

pub struct RSS {
    config: Arc<RwLock<config::Config>>,
    client: Client,
}

#[derive(Deserialize)]
struct RSSFeed {
    #[serde(rename = "entry", default)]
    entries: Vec<FeedEntry>,
}

#[derive(Deserialize)]
struct FeedEntry {
    #[serde(rename = "videoId")]
    video_id: String,
    #[serde(rename = "channelId")]
    channel_id: String,
    title: String,
    author: Author,
    group: MediaGroup,
    updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct Author {
    name: String,
}

#[derive(Deserialize)]
struct MediaGroup {
    thumbnail: Thumbnail,
    description: String,
}

#[derive(Deserialize)]
struct Thumbnail {
    url: String,
}

impl RSS {
    async fn run_one(
        &self,
        scraped: Arc<Mutex<HashSet<String>>>,
        channel: config::ChannelConfig,
    ) -> Result<impl Stream<Item = Task>> {
        debug!("Fetching RSS for {}", channel.name);

        // Get config
        let max_age =
            chrono::Duration::from_std(self.config.read().await.scraper.rss.ignore_older_than)
                .context("Failed to convert ignore_older_than to chrono::Duration")?;
        debug!(
            "Ignoring videos older than {}",
            max_age
                .to_std()
                .map(humantime::format_duration)
                .map(|s| s.to_string())
                .unwrap_or_else(|_| "???".into())
        );

        // Fetch the RSS feed
        let url = format!(
            "https://www.youtube.com/feeds/videos.xml?channel_id={}",
            channel.id
        );
        let res = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch RSS feed")?;
        let feed: RSSFeed =
            quick_xml::de::from_slice(&res.bytes().await.context("Failed to read RSS feed body")?)
                .context("Failed to parse RSS feed")?;

        // Find matching videos
        let tasks: Vec<Task> = feed
            .entries
            .iter()
            .filter_map(move |entry| {
                let mut scraped = scraped.lock().unwrap();

                if scraped.contains(&entry.video_id) {
                    // Skip if video has already been scraped
                    debug!("Skipping {}: already scraped", entry.video_id);
                    return None;
                } else if entry.updated < chrono::Utc::now() - max_age {
                    // Or if the video is too old
                    debug!(
                        "Skipping {}: too old ({} < {})",
                        entry.video_id,
                        entry.updated,
                        chrono::Utc::now() - max_age
                    );
                    return None;
                } else if !channel.filters.iter().any(|filter| {
                    filter.is_match(&entry.title)
                        || (channel.match_description && filter.is_match(&entry.group.description))
                }) {
                    // Or if the video doesn't match the filters
                    debug!("Skipping {}: doesn't match filters", entry.video_id);
                    return None;
                }

                // Add to scraped set
                scraped.insert(entry.video_id.clone());

                // Return the task
                Some(Task {
                    title: entry.title.to_owned(),
                    video_id: entry.video_id.to_owned(),
                    video_picture: entry.group.thumbnail.url.to_owned(),
                    channel_name: entry.author.name.to_owned(),
                    channel_id: entry.channel_id.to_owned(),
                    channel_picture: channel.picture_url.clone(),
                    output_directory: channel.outpath.clone(),
                    recorder: channel.recorder.clone(),
                })
            })
            .collect();

        Ok(stream::iter(tasks))
    }

    async fn run_loop(
        &self,
        scraped: Arc<Mutex<HashSet<String>>>,
    ) -> impl Stream<Item = Task> + '_ {
        let config = self.config.read().await;
        stream::iter(config.channel.clone())
            .map(move |channel| self.run_one(scraped.clone(), channel))
            .buffer_unordered(4)
            .filter_map(|one| async { one.map_err(|e| error!("Failed to run RSS: {:?}", e)).ok() })
            .flatten()
    }

    async fn cache_picture_url(&self) -> Result<()> {
        let cfg = self.config.clone();
        let cfg: &mut config::Config = &mut *cfg.write().await;
        for channel in &mut *cfg.channel {
            if channel.picture_url.is_some() {
                continue;
            }

            channel.picture_url = Some(
                youtube::channel::fetch_picture_url(self.client.clone(), &channel.id)
                    .await
                    .context("Failed to fetch channel picture URL")?,
            );
        }
        Ok(())
    }
}

#[async_trait]
impl Module for RSS {
    fn new(config: Arc<RwLock<config::Config>>) -> Self {
        let client = Client::builder()
            .user_agent(APP_USER_AGENT)
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create client");
        Self { config, client }
    }

    async fn run(&self, tx: &BusTx<Message>, rx: &mut mpsc::Receiver<Message>) -> Result<()> {
        let scraped = Arc::new(Mutex::new(HashSet::<String>::new()));
        loop {
            // Cache channel image URLs
            if let Err(e) = self.cache_picture_url().await {
                warn!("Failed to cache channel image URLs: {}", e);
            }

            // Scrape the RSS feeds
            let err = self
                .run_loop(scraped.clone())
                .await
                .map(|task| tx.send(Message::ToRecord(task.clone())))
                .buffer_unordered(4)
                .collect::<Vec<Result<_, _>>>()
                .await
                .iter()
                .any(|x| x.is_err());

            if err {
                debug!("Failed to send message to bus");
                return Ok(());
            }

            // Determine when to wake up
            let wakeup = {
                let cfg = self.config.read().await;
                std::time::Instant::now() + cfg.scraper.rss.poll_interval
            };

            // Sleep
            while std::time::Instant::now() < wakeup {
                match rx.try_recv() {
                    Ok(_) => continue,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        debug!("Stopped scraping RSS");
                        return Ok(());
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        }
    }
}
