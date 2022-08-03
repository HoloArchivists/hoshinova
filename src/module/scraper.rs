use super::{Message, Module, Task};
use crate::{config, msgbus::BusTx, APP_USER_AGENT};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::stream::{self, Stream, StreamExt};
use lazy_static::lazy_static;
use reqwest::Client;
use serde::Deserialize;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
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
}

#[derive(Deserialize)]
struct Author {
    name: String,
}

#[derive(Deserialize)]
struct MediaGroup {
    thumbnail: Thumbnail,
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

        // Fetch the RSS feed
        let url = format!(
            "https://www.youtube.com/feeds/videos.xml?channel_id={}",
            channel.id
        );
        let res = self.client.get(&url).send().await?;
        let feed: RSSFeed = quick_xml::de::from_slice(&res.bytes().await?)?;

        // Find matching videos
        let tasks: Vec<Task> = feed
            .entries
            .iter()
            .filter_map(move |entry| {
                let mut scraped = scraped.lock().unwrap();
                if channel
                    .filters
                    .iter()
                    .any(|filter| !filter.is_match(&entry.title))
                    || scraped.contains(&entry.video_id)
                {
                    None
                } else {
                    scraped.insert(entry.video_id.to_owned());
                    Some(Task {
                        title: entry.title.to_owned(),
                        video_id: entry.video_id.to_owned(),
                        video_picture: entry.group.thumbnail.url.to_owned(),
                        channel_name: entry.author.name.to_owned(),
                        channel_id: entry.channel_id.to_owned(),
                        channel_picture: channel.picture_url.clone(),
                        output_directory: channel.outpath.clone(),
                    })
                }
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
            .filter_map(|one| async { one.map_err(|e| error!("Failed to run RSS: {}", e)).ok() })
            .flatten()
    }

    async fn cache_picture_url(&self) -> Result<()> {
        let cfg = self.config.clone();
        let cfg: &mut config::Config = &mut *cfg.write().await;
        for channel in &mut *cfg.channel {
            if channel.picture_url.is_some() {
                continue;
            }

            // Fetch the channel page
            let channel_url = format!("https://www.youtube.com/channel/{}", channel.id);
            let res = self
                .client
                .get(&channel_url)
                .send()
                .await
                .map_err(|e| anyhow!("Error fetching channel page: {}", e))?
                .text()
                .await
                .map_err(|e| anyhow!("Error fetching channel page: {}", e))?;

            // Find the picture URL
            lazy_static! {
                static ref RE: regex::Regex =
                    regex::Regex::new(r#"<meta name="twitter:image" content="(.*?)""#).unwrap();
            }
            let captures = RE
                .captures(&res)
                .ok_or_else(|| anyhow!("Could not find picture URL"))?;
            let picture_url = captures
                .get(1)
                .ok_or_else(|| anyhow!("Could not find picture URL"))?
                .as_str();
            channel.picture_url = Some(picture_url.to_owned());
            debug!("[{}] Found picture URL: {}", channel.id, picture_url);
        }
        Ok(())
    }
}

#[async_trait]
impl Module for RSS {
    fn new(config: Arc<RwLock<config::Config>>) -> Self {
        let client = Client::builder()
            .user_agent(APP_USER_AGENT)
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
