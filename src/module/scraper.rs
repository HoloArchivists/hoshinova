use super::{Message, Module, Task};
use crate::{config, msgbus::BusTx};
use anyhow::Result;
use async_trait::async_trait;
use futures::stream::{self, Stream, StreamExt, TryStreamExt};
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
                        channel_name: entry.author.name.to_owned(),
                        channel_id: entry.channel_id.to_owned(),
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
        let config = &*self.config.read().await;
        stream::iter(config.channel.clone())
            .map(move |channel| self.run_one(scraped.clone(), channel))
            .buffer_unordered(4)
            .filter_map(|one| async { one.map_err(|e| error!("Failed to run RSS: {}", e)).ok() })
            .flatten()
    }
}

#[async_trait]
impl Module for RSS {
    fn new(config: Arc<RwLock<config::Config>>) -> Self {
        let client = Client::new();
        Self { config, client }
    }

    async fn run(&self, tx: &BusTx<Message>, rx: &mut mpsc::Receiver<Message>) -> Result<()> {
        let scraped = Arc::new(Mutex::new(HashSet::<String>::new()));
        loop {
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

            // Sleep
            // TODO: sleep!
            // todo!("Sleep");

            // When to wake up
            let cfg = &*self.config.read().await;
            let wakeup = std::time::Instant::now() + cfg.scraper.rss.poll_interval;
            while std::time::Instant::now() < wakeup {
                if let Err(mpsc::error::TryRecvError::Disconnected) = rx.try_recv() {
                    debug!("Stopped scraping RSS");
                    return Ok(());
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }

            // if rx.wait_until_closed(self.config.scraper.rss.poll_interval) {
            // debug!("Stopped scraping RSS");
            // break;
            // }
        }
    }
}
