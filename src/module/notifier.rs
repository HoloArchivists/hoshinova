use super::{Message, Module, Notification, TaskStatus};
use crate::msgbus::BusTx;
use crate::{config::Config, APP_NAME, APP_USER_AGENT};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

pub struct Discord {
    config: Arc<RwLock<Config>>,
    client: Client,
}

#[derive(Serialize)]
struct WebhookMessage {
    content: String,
    embeds: Vec<DiscordEmbed>,
}

#[derive(Serialize)]
struct DiscordEmbed {
    title: String,
    description: String,
    color: u32,
    author: DiscordEmbedAuthor,
    footer: DiscordEmbedFooter,
    timestamp: String,
    thumbnail: DiscordEmbedThumbnail,
}

#[derive(Serialize)]
struct DiscordEmbedAuthor {
    name: String,
    url: String,
    icon_url: Option<String>,
}

#[derive(Serialize)]
struct DiscordEmbedFooter {
    text: String,
}

#[derive(Serialize)]
struct DiscordEmbedThumbnail {
    url: String,
}

#[async_trait]
impl Module for Discord {
    fn new(config: Arc<RwLock<Config>>) -> Self {
        let client = Client::builder()
            .user_agent(APP_USER_AGENT)
            .build()
            .expect("Failed to create client");
        Self { config, client }
    }

    async fn run(&self, _tx: &BusTx<Message>, rx: &mut mpsc::Receiver<Message>) -> Result<()> {
        // Listen for messages
        while let Some(message) = rx.recv().await {
            // Wait for notifications
            let Notification { task, status } = match message {
                Message::ToNotify(notification) => notification,
                _ => continue,
            };

            // Get configuration
            let cfg = {
                let cfg = self.config.read().await;
                let not = cfg.notifier.clone();
                match (|| not?.discord)() {
                    Some(cfg) => cfg,
                    None => continue,
                }
            };

            // Check if we should notify
            if !cfg.notify_on.contains(&status) {
                debug!("Not notifying on status {:?}", status);
                continue;
            }

            let (title, color) = match status {
                TaskStatus::Waiting => ("Waiting for Live", 0xebd045),
                TaskStatus::Recording => ("Recording", 0x58b9ff),
                TaskStatus::Done => ("Done", 0x45eb45),
                TaskStatus::Failed => ("Failed", 0xeb4545),
            };
            let timestamp = chrono::Utc::now().to_rfc3339();

            // Construct the payload
            let message = WebhookMessage {
                content: "".into(),
                embeds: vec![DiscordEmbed {
                    title: title.into(),
                    description: format!("[{}](https://youtu.be/{})", task.title, task.video_id),
                    color,
                    author: DiscordEmbedAuthor {
                        name: task.channel_name,
                        url: format!("https://www.youtube.com/channel/{}", task.channel_id),
                        icon_url: task.channel_picture,
                    },
                    footer: DiscordEmbedFooter {
                        text: APP_NAME.into(),
                    },
                    timestamp: timestamp,
                    thumbnail: DiscordEmbedThumbnail {
                        url: task.video_picture,
                    },
                }],
            };

            // Send the webhook
            let res = self
                .client
                .post(&cfg.webhook_url)
                .header("Content-Type", "application/json")
                .json(&message)
                .send()
                .await;

            match res {
                Ok(res) => {
                    if res.status().is_success() {
                        info!("Sent Discord webhook");
                    } else {
                        error!("Failed to send Discord webhook: {}", res.status());
                    }
                }
                Err(e) => error!("Failed to send Discord webhook: {}", e),
            }
        }

        debug!("Discord notifications module finished");
        Ok(())
    }
}
