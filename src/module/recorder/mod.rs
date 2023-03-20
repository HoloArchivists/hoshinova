use crate::module::Module;

mod ytarchive;
use ytarchive::YTArchive;
pub mod ytdlp;
use ytdlp::YTDlp;
use super::{Message, Notification, Task, TaskStatus};
use crate::msgbus::BusTx;
use crate::{config::Config, module::RecordingStatus};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::{
    fs,
    path::Path,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    io::{AsyncReadExt, BufReader},
    sync::{mpsc, RwLock},
};
use ts_rs::TS;

/// The current state of video.
#[derive(Debug, Clone, TS, Serialize)]
#[ts(export, export_to = "web/src/bindings/")]
pub struct VideoStatus {
    version: Option<String>,
    state: RecorderState,
    last_output: Option<String>,
    last_update: chrono::DateTime<chrono::Utc>,
    video_fragments: Option<u32>,
    audio_fragments: Option<u32>,
    total_size: Option<String>,
    video_quality: Option<String>,
    output_file: Option<String>,
}

impl VideoStatus {
    pub fn new() -> Self {
        Self {
            version: None,
            state: RecorderState::Idle,
            last_output: None,
            last_update: chrono::Utc::now(),
            video_fragments: None,
            audio_fragments: None,
            total_size: None,
            video_quality: None,
            output_file: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, TS, Serialize, Deserialize)]
#[ts(export, export_to = "web/src/bindings/")]
pub enum RecorderState {
    Idle,
    Waiting(Option<DateTime<Utc>>),
    Recording,
    Muxing,
    Finished,
    AlreadyProcessed,
    Ended,
    Interrupted,
    Errored,
}
impl RecorderState {
    fn default() -> Self { RecorderState::Idle }
}

struct SpawnTask {
    task: Task,
    cfg: Config,
    tx: BusTx<Message>,
}

pub struct RecorderRunner {
    config: Arc<RwLock<Config>>,
    active_ids: Arc<RwLock<HashSet<String>>>,
}

#[async_trait]
impl Module for RecorderRunner {
    fn new(config: Arc<RwLock<Config>>) -> Self {
        let active_ids = Arc::new(RwLock::new(HashSet::new()));
        Self { config, active_ids }
    }

    async fn run(&self, tx: &BusTx<Message>, rx: &mut mpsc::Receiver<Message>) -> Result<()> {
        // Create a spawn queue
        let (spawn_tx, mut spawn_rx) = mpsc::unbounded_channel::<SpawnTask>();

        // Future to handle spawning new tasks
        let active_ids = self.active_ids.clone();
        let f_spawner = async move {
            while let Some(mut task) = spawn_rx.recv().await {
                let active_ids = active_ids.clone();
                let delay = task.cfg.ytarchive.delay_start;

                debug!("Spawning thread for task: {:?}", task.task);
                tokio::spawn(async move {
                    let video_id = task.task.video_id.clone();
                    active_ids.write().await.insert(video_id.clone());

                     match task.task.recorder.as_str() {
                        "ytarchive" => {
                             if let Err(e) = YTArchive::record(task.cfg, task.task, &mut task.tx).await {
                                error!("Failed to record task: {:?}", e);
                             }
                        }
                        "yt-dlp" => {
                             if let Err(e) = YTDlp::record(task.cfg, task.task, &mut task.tx).await {
                                error!("Failed to record task: {:?}", e);
                             }
                        }
                        _ => error!("Failed to record task: invalid recorder {:?}", task.task.recorder),
                    }

                    active_ids.write().await.remove(&video_id);
                });

                // Wait a bit before starting the next task
                tokio::time::sleep(delay).await;
            }

            Ok::<(), anyhow::Error>(())
        };

        // Future to handle incoming messages
        let f_message = async move {
            while let Some(message) = rx.recv().await {
                match message {
                    Message::ToRecord(task) => {
                        // Check if the task is already active
                        if self.active_ids.read().await.contains(&task.video_id) {
                            warn!("Task {} is already active, skipping", task.video_id);
                            continue;
                        }

                        debug!("Adding task to spawn queue: {:?}", task);
                        let tx = tx.clone();
                        let cfg = self.config.read().await;
                        let cfg = cfg.clone();

                        if let Err(_) = spawn_tx.send(SpawnTask { task, cfg, tx }) {
                            debug!("Spawn queue closed, exiting");
                            break;
                        }
                    }
                    _ => (),
                }
            }

            Ok::<(), anyhow::Error>(())
        };

        // Run the futures
        tokio::try_join!(f_spawner, f_message)?;

        debug!("RecorderRunner module finished");
        Ok(())
    }
}