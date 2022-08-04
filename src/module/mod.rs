use self::recorder::{JsonStatus, YTAStatus};
use crate::{config::Config, msgbus::BusTx};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, sync::Arc};
use tokio::sync::{mpsc, RwLock};
use ts_rs::TS;

pub mod notifier;
pub mod recorder;
pub mod scraper;
pub mod web;

#[derive(Debug, Clone, TS)]
#[ts(export, export_to = "web/src/bindings/")]
pub enum Message {
    ToRecord(Task),
    ToNotify(Notification),
    RecordingStatus(RecordingStatus),
    MetadataStatus(MetadataStatus),
}

#[derive(Debug, Clone, TS, Serialize, Deserialize)]
#[ts(export, export_to = "web/src/bindings/")]
pub struct Task {
    pub title: String,
    pub video_id: String,
    pub video_picture: String,
    pub channel_name: String,
    pub channel_id: String,
    pub channel_picture: Option<String>,
    pub output_directory: String,
}

#[derive(Debug, Clone, TS)]
#[ts(export, export_to = "web/src/bindings/")]
pub struct Notification {
    pub task: Task,
    pub status: Status,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Status {
    Task(TaskStatus),
    Playability(PlayabilityStatus),
}

#[derive(Debug, Clone, TS)]
#[ts(export, export_to = "web/src/bindings/")]
pub struct RecordingStatus {
    pub task: Task,
    pub status: YTAStatus,
}

#[derive(Debug, Clone, TS)]
#[ts(export, export_to = "web/src/bindings/")]
pub struct MetadataStatus {
    pub task: Task,
    pub status: JsonStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "web/src/bindings/")]
pub enum PlayabilityStatus {
    MembersOnly,
    Privated,
    Copyrighted,
    Removed,
    Unlisted,
    OnLive,
    Ok,
    Offline,
    LoginRequired,
    Unknown,
}
impl Serialize for PlayabilityStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(match self {
            PlayabilityStatus::Ok => "ok",
            PlayabilityStatus::OnLive => "live",
            PlayabilityStatus::Removed => "removed",
            PlayabilityStatus::Offline => "offline",
            PlayabilityStatus::MembersOnly => "members_only",
            PlayabilityStatus::Unknown => "unknown",
            PlayabilityStatus::Privated => "privated",
            PlayabilityStatus::Unlisted => "unlisted",
            PlayabilityStatus::Copyrighted => "copyrighted",
            PlayabilityStatus::LoginRequired => "login_required",
        })
    }
}

impl<'de> Deserialize<'de> for PlayabilityStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match &*s {
            "ok" => Ok(PlayabilityStatus::Ok),
            "live" => Ok(PlayabilityStatus::OnLive),
            "removed" => Ok(PlayabilityStatus::Removed),
            "offline" => Ok(PlayabilityStatus::Offline),
            "members_only" => Ok(PlayabilityStatus::MembersOnly),
            "unknown" => Ok(PlayabilityStatus::Unknown),
            "privated" => Ok(PlayabilityStatus::Privated),
            "unlisted" => Ok(PlayabilityStatus::Unlisted),
            "copyrighted" => Ok(PlayabilityStatus::Copyrighted),
            "login_required" => Ok(PlayabilityStatus::LoginRequired),
            _ => Err(serde::de::Error::unknown_variant(
                &s,
                &[
                    "ok",
                    "live",
                    "removed",
                    "offline",
                    "members_only",
                    "unknown",
                    "privated",
                    "unlisted",
                    "copyrighted",
                    "login_required",
                ],
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, TS)]
#[ts(export, export_to = "web/src/bindings/")]
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

impl<'de> Deserialize<'de> for TaskStatus {
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

impl<'de> Deserialize<'de> for Status {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match &*s {
            "ok" => Ok(Status::Playability(PlayabilityStatus::Ok)),
            "live" => Ok(Status::Playability(PlayabilityStatus::OnLive)),
            "removed" => Ok(Status::Playability(PlayabilityStatus::Removed)),
            "offline" => Ok(Status::Playability(PlayabilityStatus::Offline)),
            "members_only" => Ok(Status::Playability(PlayabilityStatus::MembersOnly)),
            "unknown" => Ok(Status::Playability(PlayabilityStatus::Unknown)),
            "privated" => Ok(Status::Playability(PlayabilityStatus::Privated)),
            "unlisted" => Ok(Status::Playability(PlayabilityStatus::Unlisted)),
            "copyrighted" => Ok(Status::Playability(PlayabilityStatus::Copyrighted)),
            "login_required" => Ok(Status::Playability(PlayabilityStatus::LoginRequired)),
            "waiting" => Ok(Status::Task(TaskStatus::Waiting)),
            "recording" => Ok(Status::Task(TaskStatus::Recording)),
            "done" => Ok(Status::Task(TaskStatus::Done)),
            "failed" => Ok(Status::Task(TaskStatus::Failed)),
            _ => Err(serde::de::Error::unknown_variant(
                &s,
                &[
                    "ok",
                    "live",
                    "removed",
                    "offline",
                    "members_only",
                    "unknown",
                    "privated",
                    "unlisted",
                    "copyrighted",
                    "login_required",
                    "waiting",
                    "recording",
                    "done",
                    "failed",
                ],
            )),
        }
    }
}

#[async_trait]
pub trait Module<T: Debug + Clone + Sync = Message> {
    fn new(config: Arc<RwLock<Config>>) -> Self;
    async fn run(&self, tx: &BusTx<T>, rx: &mut mpsc::Receiver<T>) -> Result<()>;
}
