use self::recorder::YTAStatus;
use crate::config::Config;
use anyhow::Result;
use async_trait::async_trait;
use std::fmt::Debug;
use tokio::sync::mpsc;

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
pub trait Module<'a, T: Debug + Clone + Sync = Message> {
    fn new(config: &'a Config) -> Self;
    async fn run(&self, tx: &mpsc::Sender<T>, rx: &mut mpsc::Receiver<T>) -> Result<()>;
}
