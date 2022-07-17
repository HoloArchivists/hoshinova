use self::recorder::YTAStatus;
use crate::config::Config;
use crate::msgbus::{BusRx, BusTx};
use anyhow::Result;
use std::fmt::Debug;

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

pub trait Module<'a, T: Debug + Clone + Sync = Message> {
    fn new(config: &'a Config) -> Self;
    fn run(&self, tx: &BusTx<T>, rx: &mut BusRx<T>) -> Result<()>;
}
