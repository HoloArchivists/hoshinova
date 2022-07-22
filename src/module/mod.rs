use self::recorder::YTAStatus;
use crate::{config::Config, msgbus::BusTx};
use anyhow::Result;
use async_trait::async_trait;
use serde::Serialize;
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
    pub video_picture: String,
    pub channel_name: String,
    pub channel_id: String,
    pub channel_picture: Option<String>,
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

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Waiting,
    Recording,
    Done,
    Failed,
}

impl Serialize for TaskStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(match self {
            TaskStatus::Waiting => "waiting",
            TaskStatus::Recording => "recording",
            TaskStatus::Done => "done",
            TaskStatus::Failed => "failed",
        })
    }
}

impl<'de> serde::Deserialize<'de> for TaskStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match &*s {
            "waiting" => Ok(TaskStatus::Waiting),
            "recording" => Ok(TaskStatus::Recording),
            "done" => Ok(TaskStatus::Done),
            "failed" => Ok(TaskStatus::Failed),
            _ => Err(serde::de::Error::unknown_variant(
                &s,
                &["waiting", "recording", "done", "failed"],
            )),
        }
    }
}

#[async_trait]
pub trait Module<T: Debug + Clone + Sync = Message> {
    fn new(config: Arc<RwLock<Config>>) -> Self;
    async fn run(&self, tx: &BusTx<T>, rx: &mut mpsc::Receiver<T>) -> Result<()>;
}
