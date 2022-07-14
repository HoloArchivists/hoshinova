use super::{Message, Module, Notification, Task, TaskStatus};
use crate::{
    config,
    msgbus::{BusRx, BusTx},
};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use std::io::{BufRead, BufReader, Read};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct YTArchive<'a> {
    config: &'a config::Config,
}

#[derive(Debug)]
enum RecMsg {
    Event(Notification),
    Close,
}

impl<'a> YTArchive<'a> {
    fn record(&self, task: Task, msg: &BusTx<Message>) -> Result<()> {
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
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("Failed to start process: {}", e))?;

        let mut stdout = process
            .stdout
            .take()
            .ok_or(anyhow!("Failed to take stdout"))?
            .bytes();
        let mut stderr = process
            .stderr
            .take()
            .ok_or(anyhow!("Failed to take stderr"))?
            .bytes();

        let (tx, rx) = crossbeam::channel::unbounded();

        let done = AtomicBool::new(false);

        crossbeam::scope(|s| {
            macro_rules! read_line {
                ($reader:expr) => {{
                    let mut bytes = Vec::new();
                    for byte in $reader {
                        match byte {
                            Ok(b'\n') | Ok(b'\r') => break,
                            Ok(byte) => bytes.push(byte),
                            _ => (),
                        };
                    }
                    match std::str::from_utf8(&bytes) {
                        Ok(line) => Ok(line.to_owned()),
                        Err(e) => Err(anyhow!("Failed to read utf8: {:?}", e)),
                    }
                }};
            }

            // Read stdout
            let h_stdout = s.spawn(|_| {
                while !done.load(Ordering::Relaxed) {
                    match read_line!(&mut stdout) {
                        Ok(line) => {
                            if let Err(e) = tx.send(line) {
                                trace!("Failed to send stdout: {}", e);
                                return;
                            }
                        }
                        Err(e) => {
                            trace!("Failed to read stdout: {}", e);
                            return;
                        }
                    };
                }
            });

            // Read stderr
            let h_stderr = s.spawn(|_| {
                while !done.load(Ordering::Relaxed) {
                    match read_line!(&mut stderr) {
                        Ok(line) => {
                            if let Err(e) = tx.send(line) {
                                trace!("Failed to send stderr: {}", e);
                                return;
                            }
                        }
                        Err(e) => {
                            trace!("Failed to read stderr: {}", e);
                            return;
                        }
                    };
                }
            });

            // Parse each line
            let h_status = s.spawn(|_| {
                let mut status = YTAStatus::new();

                for line in rx {
                    // Stop when done
                    if done.load(Ordering::Relaxed) {
                        break;
                    } else if line == "" {
                        continue;
                    }

                    trace!("[{}] {}", task.video_id, line);

                    let old = status.clone();
                    status.parse_line(&line);
                    trace!("[{}] {:?}", task.video_id, status);

                    // Check if status changed
                    if old.state != status.state {
                        let err = match status.state {
                            YTAState::Waiting(_) => {
                                info!("[{}] Waiting for stream to go live", task.video_id);
                                msg.send(Message::ToNotify(Notification {
                                    task: task.clone(),
                                    status: TaskStatus::Waiting,
                                }))
                            }
                            YTAState::Recording => {
                                info!("[{}] Recording started", task.video_id);
                                msg.send(Message::ToNotify(Notification {
                                    task: task.clone(),
                                    status: TaskStatus::Recording,
                                }))
                            }
                            YTAState::Finished => {
                                info!("[{}] Recording finished", task.video_id);
                                msg.send(Message::ToNotify(Notification {
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
                                msg.send(Message::ToNotify(Notification {
                                    task: task.clone(),
                                    status: TaskStatus::Failed,
                                }))
                            }
                            _ => Ok(()),
                        }
                        .is_err();

                        if err {
                            break;
                        }
                    }
                }

                // Return final status
                status
            });

            let video_id = task.video_id.clone();
            trace!("[{}] Output monitor started", video_id);

            // Wait for the process to exit
            let result = process.wait();

            // Stop threads
            done.store(true, Ordering::Relaxed);
            debug!("[{}] Process exited with {:?}", video_id, result);

            // Wait for threads to finish
            trace!("[{}] Stdout monitor quit: {:?}", video_id, h_stdout.join());
            trace!("[{}] Stderr monitor quit: {:?}", video_id, h_stderr.join());
            trace!("[{}] Status monitor quit: {:?}", video_id, h_status.join());
        })
        .map_err(|e| anyhow!("Failed to exit: {:?}", e))?;

        Ok(())
    }
}

impl<'a> Module<'a> for YTArchive<'a> {
    fn new(config: &'a config::Config) -> Self {
        Self { config }
    }

    fn run(&self, tx: &BusTx<Message>, rx: &mut BusRx<Message>) -> Result<()> {
        let res = crossbeam::scope(|s| {
            // Listen for new messages
            loop {
                match rx.recv() {
                    Ok(Message::ToRecord(task)) => {
                        debug!("Spawning thread for task: {:?}", task);
                        let tx = tx.clone();
                        s.spawn(move |_| self.record(task, tx));
                    }
                    Err(_) => break,
                    _ => (),
                }
            }

            debug!("Loop exited. Waiting for all threads to finish...");
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
