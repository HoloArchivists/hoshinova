use super::super::{Message, Module, Notification, Task, TaskStatus};
use super::{YTState, YTStatus};
use crate::msgbus::BusTx;
use crate::{config::Config, module::RecordingStatus};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use serde::Serialize;
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


pub struct YTArchive;

impl YTArchive {
    pub async fn record(cfg: Config, task: Task, bus: &mut BusTx<Message>) -> Result<()> {
        let task_name = format!("[{}][{}][{}]", task.video_id, task.channel_name, task.title);

        // Ensure the working directory exists
        let cfg = cfg.ytarchive;
        tokio::fs::create_dir_all(&cfg.working_directory)
            .await
            .context("Failed to create working directory")?;

        // Ensure the output directory exists
        tokio::fs::create_dir_all(&task.output_directory)
            .await
            .context("Failed to create output directory")?;

        // Construct the command line arguments
        let mut args = cfg.args.clone();

        // Add the --wait flag if not present
        if !args.contains(&"-w".to_string()) && !args.contains(&"--wait".to_string()) {
            args.push("--wait".to_string());
        }

        args.extend(vec![
            format!("https://youtu.be/{}", task.video_id),
            cfg.quality.clone(),
        ]);

        // Start the process
        debug!("{} Starting ytarchive with args {:?}", task_name, args);
        let mut process = tokio::process::Command::new(&cfg.executable_path)
            .args(args)
            .current_dir(&cfg.working_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to start ytarchive")?;

        // Grab stdout/stderr byte iterators
        let mut stdout = BufReader::new(
            process
                .stdout
                .take()
                .ok_or(anyhow!("Failed to take stdout"))?,
        );
        let mut stderr = BufReader::new(
            process
                .stderr
                .take()
                .ok_or(anyhow!("Failed to take stderr"))?,
        );

        // Create a channel to consolidate stdout and stderr
        let (tx, mut rx) = mpsc::channel(1);

        // Flag to mark when the process has exited
        let done = Arc::from(AtomicBool::new(false));

        macro_rules! read_line {
            ($reader:expr, $tx:expr) => {{
                // Read bytes until a \r or \n is returned
                let mut bytes = Vec::new();
                loop {
                    match $reader.read_u8().await {
                        Ok(byte) => {
                            if byte == b'\r' || byte == b'\n' {
                                break;
                            }
                            bytes.push(byte);
                        }
                        _ => break,
                    }
                }

                // Skip if there are no bytes
                if bytes.is_empty() {
                    continue;
                }

                // Convert to a string
                let line = match std::str::from_utf8(&bytes) {
                    Ok(line) => line.to_owned(),
                    Err(e) => {
                        trace!("Failed to read utf8: {:?}", e);
                        break;
                    }
                };

                // Send the line to the channel
                if let Err(e) = $tx.send(line).await {
                    trace!("Failed to send line: {:?}", e);
                    break;
                }
            }};
        }

        // Read stdout
        let h_stdout = tokio::spawn({
            let done = done.clone();
            let task_name = task_name.clone();
            let tx = tx.clone();
            async move {
                while !done.load(Ordering::Relaxed) {
                    read_line!(&mut stdout, tx);
                }
                trace!("{} stdout reader exited", task_name);
            }
        });

        // Read stderr
        let h_stderr = tokio::spawn({
            let done = done.clone();
            let task_name = task_name.clone();
            let tx = tx.clone();
            async move {
                while !done.load(Ordering::Relaxed) {
                    read_line!(&mut stderr, tx);
                }
                trace!("{} stderr reader exited", task_name);
            }
        });

        // Wait for the process to exit
        let h_wait = tokio::spawn({
            let done = done.clone();
            let task_name = task_name.clone();
            async move {
                let result = process.wait().await;

                // Wait a bit for the stdout to be completely read
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                // Stop threads
                done.store(true, Ordering::Relaxed);
                debug!("{} Process exited with {:?}", task_name, result);

                // Send a blank message to unblock the status monitor thread
                let _ = tx.send("".into());

                result
            }
        });

        // Parse each line
        let mut status = YTStatus::new();
        loop {
            let line = match rx.recv().await {
                Some(line) => line,
                None => break,
            };

            // Stop when done
            if done.load(Ordering::Relaxed) {
                break;
            }

            trace!("{}[yta:out] {}", task_name, line);

            let old = status.clone();
            status.parse_line(&line);

            // Push the current status to the bus
            if let Err(_) = bus
                .send(Message::RecordingStatus(RecordingStatus {
                    task: task.clone(),
                    status: status.clone(),
                }))
                .await
            {
                break;
            }

            // Check if status changed
            if old.state == status.state {
                continue;
            }

            let message = match status.state {
                YTState::Waiting(_) => {
                    info!("{} Waiting for stream to go live", task_name);
                    Some(Message::ToNotify(Notification {
                        task: task.clone(),
                        status: TaskStatus::Waiting,
                    }))
                }
                YTState::Recording => {
                    info!("{} Recording started", task_name);
                    Some(Message::ToNotify(Notification {
                        task: task.clone(),
                        status: TaskStatus::Recording,
                    }))
                }
                YTState::Finished => {
                    info!("{} Recording finished", task_name);
                    Some(Message::ToNotify(Notification {
                        task: task.clone(),
                        status: TaskStatus::Done,
                    }))
                }
                YTState::AlreadyProcessed => {
                    info!("{} Video already processed, skipping", task_name);
                    None
                }
                YTState::Interrupted => {
                    info!("{} Recording failed: interrupted", task_name);
                    Some(Message::ToNotify(Notification {
                        task: task.clone(),
                        status: TaskStatus::Failed,
                    }))
                }
                _ => None,
            };

            if let Some(message) = message {
                // Exit the loop if message failed to send
                if let Err(_) = bus.send(message).await {
                    break;
                }
            }
        }

        trace!("{} Status loop exited: {:?}", task_name, status);

        // Wait for threads to finish
        let (r_wait, r_stdout, r_stderr) = futures::join!(h_wait, h_stdout, h_stderr);
        trace!("{} Process monitor exited: {:?}", task_name, r_wait);
        trace!("{} Stdout monitor quit: {:?}", task_name, r_stdout);
        trace!("{} Stderr monitor quit: {:?}", task_name, r_stderr);

        // Skip moving files if it didn't finish
        if status.state != YTState::Finished {
            return Ok(());
        }

        // Move the video to the output directory
        let frompath = status
            .output_file
            .ok_or(anyhow!("ytarchive did not emit an output file"))?;
        let frompath = Path::new(&frompath);
        let filename = frompath
            .file_name()
            .ok_or(anyhow!("Failed to get filename"))?;
        let destpath = Path::new(&task.output_directory).join(filename);

        // Try to rename the file into the output directory
        if let Err(_) = fs::rename(frompath, &destpath) {
            debug!(
                "{} Failed to rename file to output, trying to copy",
                task_name,
            );

            // Copy the file into the output directory
            fs::copy(frompath, &destpath)
                .with_context(|| format!("Failed to copy file to output: {:?}", destpath))?;
            info!(
                "{} Copied output file to {}, removing original",
                task_name,
                destpath.display(),
            );
            fs::remove_file(frompath)
                .with_context(|| format!("Failed to remove original file: {:?}", frompath))?;
        }

        info!("{} Moved output file to {}", task_name, destpath.display());
        Ok(())
    }
}

fn strip_ansi(s: &str) -> String {
    lazy_static! {
        static ref RE: Regex = Regex::new(concat!(
            r"[\u001B\u009B][[\\]()#;?]*",
            r"(?:(?:(?:[a-zA-Z\\d]*(?:;[a-zA-Z\\d]*)*)?\u0007)|",
            r"(?:(?:\\d{1,4}(?:;\\d{0,4})*)?[\\dA-PRZcf-ntqry=><~]))",
        ))
        .expect("Failed to compile ANSI stripping regex");
    }
    let stripped = RE.replace_all(s, "").to_string();
    stripped
        .strip_suffix("\u{001b}[K")
        .unwrap_or(&stripped)
        .to_string()
}

impl YTStatus {
    pub fn new() -> Self {
        Self {
            version: None,
            state: YTState::Idle,
            last_output: None,
            last_update: chrono::Utc::now(),
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
        self.last_update = chrono::Utc::now();

        if line.starts_with("Video Fragments: ") {
            self.state = YTState::Recording;
            let mut parts = line.split(';').map(|s| s.split(':').nth(1).unwrap_or(""));
            if let Some(x) = parts.next() {
                self.video_fragments = x.trim().parse().ok();
            };
            if let Some(x) = parts.next() {
                self.audio_fragments = x.trim().parse().ok();
            };
            if let Some(x) = parts.next() {
                self.total_size = Some(strip_ansi(x.trim()));
            };
            return;
        } else if line.starts_with("Audio Fragments: ") {
            self.state = YTState::Recording;
            let mut parts = line.split(';').map(|s| s.split(':').nth(1).unwrap_or(""));
            if let Some(x) = parts.next() {
                self.audio_fragments = x.trim().parse().ok();
            };
            if let Some(x) = parts.next() {
                self.total_size = Some(strip_ansi(x.trim()));
            };
            return;
        }

        // New versions of ytarchive prepend a timestamp to the output
        let line = if self.version == Some("0.3.2".into())
            && line.len() > 20
            && line.chars().nth(4) == Some('/')
        {
            line[20..].trim()
        } else {
            line
        };

        if self.version == None && line.starts_with("ytarchive ") {
            self.version = Some(strip_ansi(&line[10..]));
        } else if self.video_quality == None && line.starts_with("Selected quality: ") {
            self.video_quality = Some(strip_ansi(&line[18..]));
        } else if line.starts_with("Stream starts at ") {
            let date = DateTime::parse_from_rfc3339(&line[17..42])
                .ok()
                .map(|d| d.into());
            self.state = YTState::Waiting(date);
        } else if line.starts_with("Stream is ") || line.starts_with("Waiting for stream") {
            self.state = YTState::Waiting(None);
        } else if line.starts_with("Muxing final file") {
            self.state = YTState::Muxing;
        } else if line.starts_with("Livestream has been processed") {
            self.state = YTState::AlreadyProcessed;
        } else if line.starts_with("Livestream has ended and is being processed")
            || line.contains("use yt-dlp to download it.")
        {
            self.state = YTState::Ended;
        } else if line.starts_with("Final file: ") {
            self.state = YTState::Finished;
            self.output_file = Some(strip_ansi(&line[12..]));
        } else if line.contains("User Interrupt") {
            self.state = YTState::Interrupted;
        } else if line.contains("Error retrieving player response")
            || line.contains("unable to retrieve")
            || line.contains("error writing the muxcmd file")
            || line.contains("Something must have gone wrong with ffmpeg")
            || line.contains("At least one error occurred")
        {
            self.state = YTState::Errored;
        } else if line.trim().is_empty()
            || line.contains("Loaded cookie file")
            || line.starts_with("Video Title: ")
            || line.starts_with("Channel: ")
            || line.starts_with("Waiting for this time to elapse")
            || line.starts_with("Download Finished")
        {
            // Ignore
        } else {
            warn!("Unknown ytarchive output: {}", line);
        }
    }
}
