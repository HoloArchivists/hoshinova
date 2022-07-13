use super::{Message, Module, Notification, Task, TaskStatus};
use crate::{
    config,
    msgbus::{BusRx, BusTx},
};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::process::Stdio;

pub struct YTArchive<'a> {
    config: &'a config::Config,
}

#[derive(Debug)]
enum RecMsg {
    Event(Notification),
    Close,
}

impl<'a> YTArchive<'a> {
    pub fn new(config: &'a config::Config) -> Self {
        Self { config }
    }

    fn record(&self, task: Task, msg: crossbeam::channel::Sender<RecMsg>) -> Result<()> {
        // Ensure the working directory exists
        let cfg = &self.config.ytarchive;
        std::fs::create_dir_all(&cfg.working_directory)
            .map_err(|e| anyhow!("Failed to create working directory: {}", e))?;

        // Construct the command line arguments
        let mut args = cfg.args.clone();
        args.extend(vec![
            format!("https://youtu.be/{}", task.video_id),
            cfg.quality.clone(),
        ]);

        // Start the process
        debug!(
            "[{}] Starting ytarchive with args {:?}",
            task.video_id, args
        );
        let mut process = std::process::Command::new(&cfg.executable_path)
            .args(args)
            .current_dir(&cfg.working_directory)
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("Failed to start process: {}", e))?;

        let stdout = process
            .stdout
            .take()
            .ok_or(anyhow!("Failed to get stdout"))?;

        // Read the stdout and stderr streams
        debug!("[{}] Reading stdout", task.video_id);
        let state = BufReader::new(stdout)
            .lines()
            .scan(YTAStatus::new(), |status, line| match line {
                Ok(line) => {
                    let old = status.clone();
                    status.parse_line(&line);
                    trace!("[{}] {:?}", task.video_id, status);

                    if old.state != status.state {
                        match status.state {
                            YTAState::Waiting(_) => {
                                info!("[{}] Waiting for stream to go live", task.video_id);
                                msg.send(RecMsg::Event(Notification {
                                    task: task.clone(),
                                    status: TaskStatus::Waiting,
                                }))
                            }
                            YTAState::Recording => {
                                info!("[{}] Recording started", task.video_id);
                                msg.send(RecMsg::Event(Notification {
                                    task: task.clone(),
                                    status: TaskStatus::Recording,
                                }))
                            }
                            YTAState::Finished => {
                                info!("[{}] Recording finished", task.video_id);
                                msg.send(RecMsg::Event(Notification {
                                    task: task.clone(),
                                    status: TaskStatus::Done,
                                }))
                            }
                            YTAState::AlreadyProcessed => {
                                info!("[{}] Video already processed, skipping", task.video_id);
                                Ok(())
                            }
                            YTAState::Interrupted => {
                                info!("[{}] Recording failed: interrupted", task.video_id);
                                msg.send(RecMsg::Event(Notification {
                                    task: task.clone(),
                                    status: TaskStatus::Failed,
                                }))
                            }
                            _ => Ok(()),
                        }
                        .map(|_| status.clone())
                        .ok()
                    } else {
                        Some(status.clone())
                    }
                }
                Err(_) => None,
            })
            .last();

        debug!("[{}] Waiting for the process to finish", task.video_id);
        let result = process.wait();
        debug!("[{}] Process exited with {:?}", task.video_id, result);
        debug!("[{}] Final status: {:?}", task.video_id, state);
        Ok(())
    }
}

impl<'a> Module for YTArchive<'a> {
    fn run(&self, tx: &BusTx<Message>, rx: &mut BusRx<Message>) -> Result<()> {
        // Set up a channel to communicate with the recorder threads
        let (ttx, trx) = crossbeam::channel::unbounded();

        let res = crossbeam::scope(|s| {
            // Listen for done signals
            s.spawn(|_| {
                for task in trx {
                    match task {
                        RecMsg::Event(notif) => {
                            debug!("Sending notification: {:?}", notif);
                            if let Err(e) = tx.send(Message::ToNotify(notif)) {
                                debug!("Error sending task to be notified: {}", e);
                                return;
                            }
                        }
                        RecMsg::Close => break,
                    }
                }
                debug!("Thread signal listener quit");
            });

            // Listen for new messages
            loop {
                match rx.recv() {
                    Ok(Message::ToRecord(task)) => {
                        debug!("Spawning thread for task: {:?}", task);
                        let ttx = ttx.clone();
                        s.spawn(move |_| self.record(task, ttx));
                    }
                    Err(_) => break,
                    _ => (),
                }
            }

            debug!("Loop exited. Closing channel and waiting for all threads to finish...");

            // Close the channel
            ttx.send(RecMsg::Close)
                .unwrap_or_else(|e| error!("Failed to close channel: {}", e));
        })
        .map(|_| ())
        .map_err(|e| anyhow!("{:?}", e));

        debug!("YTArchive module finished");
        return res;
    }
}

/// The current state of ytarchive.
#[derive(Debug, Clone)]
pub struct YTAStatus {
    version: Option<String>,
    state: YTAState,
    last_output: Option<String>,
    video_fragments: Option<u32>,
    audio_fragments: Option<u32>,
    total_size: Option<String>,
    video_quality: Option<String>,
    output_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum YTAState {
    Idle,
    Waiting(Option<DateTime<Utc>>),
    Recording,
    Muxing,
    Finished,
    AlreadyProcessed,
    Interrupted,
}

fn strip_ansi(s: &str) -> String {
    lazy_static! {
        static ref RE: Regex = Regex::new(concat!(
            r"[\u001B\u009B][[\\]()#;?]*",
            r"(?:(?:(?:[a-zA-Z\\d]*(?:;[a-zA-Z\\d]*)*)?\u0007)|",
            r"(?:(?:\\d{1,4}(?:;\\d{0,4})*)?[\\dA-PRZcf-ntqry=><~]))"
        ))
        .expect("Failed to compile ANSI stripping regex");
    }
    RE.replace_all(s, "").to_string()
}

impl YTAStatus {
    pub fn new() -> Self {
        Self {
            version: None,
            state: YTAState::Idle,
            last_output: None,
            video_fragments: None,
            audio_fragments: None,
            total_size: None,
            video_quality: None,
            output_file: None,
        }
    }

    /// parse_line parses a line of output from the ytarchive process.
    ///
    /// Sample output:
    ///
    ///   ytarchive 0.3.1-15663af
    ///   Stream starts at 2022-03-14T14:00:00+00:00 in 11075 seconds. Waiting for this time to elapse...
    ///   Stream is 30 seconds late...
    ///   Selected quality: 1080p60 (h264)
    ///   Video Fragments: 1215; Audio Fragments: 1215; Total Downloaded: 133.12MiB
    ///   Download Finished
    ///   Muxing final file...
    ///   Final file: /path/to/output.mp4
    pub fn parse_line(&mut self, line: &str) {
        self.last_output = Some(line.to_string());

        if line.starts_with("Video Fragments: ") {
            self.state = YTAState::Recording;
            let mut parts = line.split(';').map(|s| s.split(':').nth(1).unwrap_or(""));
            if let Some(x) = parts.next() {
                self.video_fragments = x.parse().ok();
            };
            if let Some(x) = parts.next() {
                self.audio_fragments = x.parse().ok();
            };
            if let Some(x) = parts.next() {
                self.total_size = Some(strip_ansi(x));
            };
        } else if self.version == None && line.starts_with("ytarchive ") {
            self.version = Some(strip_ansi(&line[10..]));
        } else if self.video_quality == None && line.starts_with("Selected quality: ") {
            self.video_quality = Some(strip_ansi(&line[18..]));
        } else if line.starts_with("Stream starts at ") {
            let date = DateTime::parse_from_rfc3339(&line[17..42])
                .ok()
                .map(|d| d.into());
            self.state = YTAState::Waiting(date);
        } else if line.starts_with("Stream is ") {
            self.state = YTAState::Waiting(None);
        } else if line.starts_with("Muxing final file") {
            self.state = YTAState::Muxing;
        } else if line.starts_with("Livestream has been processed") {
            self.state = YTAState::AlreadyProcessed;
        } else if line.starts_with("Final file: ") {
            self.state = YTAState::Finished;
            self.output_file = Some(strip_ansi(&line[12..]));
        } else if line.contains("User Interrupt") {
            self.state = YTAState::Interrupted;
        }
    }
}
