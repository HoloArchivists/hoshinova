use self::recorder::YTAStatus;
use crate::{config::Config, msgbus::BusTx};
use anyhow::Result;
use async_trait::async_trait;
use std::{fmt::Debug, sync::Arc};
use tokio::sync::{mpsc, RwLock};

pub mod notifier;
pub mod recorder;
pub mod scraper;

#[derive(Debug, Clone)]
pub enum Message {
    ToRecord(Task),
    ToNotify(Notification),
    RecordingStatus(RecordingStatus),
}

#[derive(Debug, Clone)]
pub struct Task {
    pub title: String,
    pub video_id: String,
    pub channel_name: String,
    pub channel_id: String,
    pub output_directory: String,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub task: Task,
    pub status: TaskStatus,
}

#[derive(Debug, Clone)]
pub struct RecordingStatus {
    pub task: Task,
    pub status: YTAStatus,
}

#[derive(Debug, Clone)]
pub enum TaskStatus {
    Waiting,
    Recording,
    Done,
    Failed,
}

#[async_trait]
pub trait Module<T: Debug + Clone + Sync = Message> {
    fn new(config: Arc<RwLock<Config>>) -> Self;
    async fn run(&self, tx: &BusTx<T>, rx: &mut mpsc::Receiver<T>) -> Result<()>;
}
