use super::super::{Message, Module, Notification, Task, TaskStatus};
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
use json;

pub struct YTDlp {
    config: Arc<RwLock<Config>>,
    active_ids: Arc<RwLock<HashSet<String>>>,
}

impl YTDlp {
    async fn record(cfg: Config, task: Task, bus: &mut BusTx<Message>) -> Result<()> {
        let task_name = format!("[{}][{}][{}]", task.video_id, task.channel_name, task.title);

        // Ensure the working directory exists
        let cfg = cfg.ytdlp;
        tokio::fs::create_dir_all(&cfg.working_directory)
            .await
            .context("Failed to create working directory")?;

        // Ensure the output directory exists
        tokio::fs::create_dir_all(&task.output_directory)
            .await
            .context("Failed to create output directory")?;

        // Construct the command line arguments
        let mut args = cfg.args.clone();

        // Add the --wait-for-video flag if not present
        if !args.iter().any( |e| Regex::new(r"--wait-for-video \d+").unwrap().is_match(e) ) {
            // --wait-for-video requires an arg dictating how often to poll, but at least for youtube it's ignored and when the stream is scheduled is used.
            args.push("--wait-for-video 10".to_string());
        }

        // Add the --live-from-start flag if not present
        if !args.contains(&"--live-from-start".to_string()) {
            args.push("--live-from-start".to_string());
        }

        // Add the --no-colors flag if not present to not output ANSI codes
        if !args.contains(&"--no-colors".to_string()) {
            args.push("--no-colors".to_string());
        }

        let progress_bar_template = r#"
            [download_progress]
            {
              "percentage": "%(progress._percent_str)s",
              "total_size": "%( progress._total_bytes_str)s",
              "estimated_total_size": "%(progress._total_bytes_estimate_str)s",
              "downloaded_size": "%(progress._downloaded_bytes_str)s",
              "speed": "%(progress._speed_str)s",
              "eta": "%(progress._eta_str)s",
              "elapsed_time": "%(progress._elapsed_str)s",
              "total_fragments": "%(progress.fragment_count)s",
              "current_fragment_count": "%(progress.fragment_index)s",
              "format": "%(info.format)s"
            }
        "#.replace(&"\n", "");

        args.push(format!("--progress-template '{progress_bar_template}'").to_string());

        args.push("--exec echo '[download_finished]  output_file: (filepath,_filename|)q'".to_string());

        args.extend(vec![
            format!("https://youtu.be/{}", task.video_id),
            cfg.quality.clone(),
        ]);

        // Start the process
        debug!("{} Starting yt-dlp with args {:?}", task_name, args);
        let mut process = tokio::process::Command::new(&cfg.executable_path)
            .args(args)
            .current_dir(&cfg.working_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to start yt-dlp")?;

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
        let mut status = YTDStatus::new();
        loop {
            let line = match rx.recv().await {
                Some(line) => line,
                None => break,
            };

            // Stop when done
            if done.load(Ordering::Relaxed) {
                break;
            }

            trace!("{}[ytd:out] {}", task_name, line);

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
                YTDState::Waiting(_) => {
                    info!("{} Waiting for stream to go live", task_name);
                    Some(Message::ToNotify(Notification {
                        task: task.clone(),
                        status: TaskStatus::Waiting,
                    }))
                }
                YTDState::Recording => {
                    info!("{} Recording started", task_name);
                    Some(Message::ToNotify(Notification {
                        task: task.clone(),
                        status: TaskStatus::Recording,
                    }))
                }
                YTDState::Finished => {
                    info!("{} Recording finished", task_name);
                    Some(Message::ToNotify(Notification {
                        task: task.clone(),
                        status: TaskStatus::Done,
                    }))
                }
                YTDState::AlreadyProcessed => {
                    info!("{} Video already processed, skipping", task_name);
                    None
                }
                YTDState::Interrupted => {
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
        if status.state != YTDState::Finished {
            return Ok(());
        }

        // Move the video to the output directory
        let frompath = status
            .output_file
            .ok_or(anyhow!("yt-dlp did not emit an output file"))?;
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

struct SpawnTask {
    task: Task,
    cfg: Config,
    tx: BusTx<Message>,
}

#[async_trait]
impl Module for YTDlp {
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

                    if let Err(e) = YTDlp::record(task.cfg, task.task, &mut task.tx).await {
                        error!("Failed to record task: {:?}", e);
                    };

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

        debug!("YTDlp module finished");
        Ok(())
    }
}

/// The current state of ytd-dlp.
#[derive(Debug, Clone, TS, Serialize)]
#[ts(export, export_to = "web/src/bindings/")]
pub struct YTDStatus {
    version: Option<String>,
    state: YTDState,
    last_output: Option<String>,
    last_update: chrono::DateTime<chrono::Utc>,
    video_fragments: Option<u32>,
    audio_fragments: Option<u32>,
    total_size: Option<String>,
    video_quality: Option<String>,
    output_file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, TS, Serialize)]
#[ts(export, export_to = "web/src/bindings/")]
pub enum YTDState {
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

impl YTDStatus {
    pub fn new() -> Self {
        Self {
            version: None,
            state: YTDState::Idle,
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
    ///
    ///   [download] Downloading item 18 of 359
    ///   [youtube] Extracting URL: https://www.youtube.com/watch?v=mNNsaF6ouOE
    ///   [youtube] mNNsaF6ouOE: Downloading webpage
    ///   [youtube] mNNsaF6ouOE: Downloading android player API JSON
    ///   [info] mNNsaF6ouOE: Downloading subtitles: live_chat
    ///   [info] mNNsaF6ouOE: Downloading 1 format(s): 299+251
    ///   [info] Writing video subtitles to: I AM UNDEFEATABLE ｜｜ Tetris w⧸ Viewers [mNNsaF6ouOE].live_chat.json
    ///   [youtube_live_chat] Downloading live chat
    ///   [youtube_live_chat] Total fragments: unknown (live)
    ///   [download] Destination: I AM UNDEFEATABLE ｜｜ Tetris w⧸ Viewers [mNNsaF6ouOE].live_chat.json
    ///   [download]   14.88MiB at  607.14KiB/s (00:00:18) (frag 64)
    ///
    ///   [Cookies] Extracting cookies from firefox
    ///   [Cookies] Extracted 2449 cookies from firefox
    ///   [youtube] Extracting URL: https://www.youtube.com/watch?v=gEdOmal1A6Q
    ///   [youtube] gEdOmal1A6Q: Downloading webpage
    ///   [youtube] gEdOmal1A6Q: Downloading android player API JSON
    ///   [info] gEdOmal1A6Q: Downloading 1 format(s): 299+251
    ///   [info] There's no subtitles for the requested languages
    ///   [info] Writing video metadata as JSON to: im orb [gEdOmal1A6Q].info.json
    ///   [dashsegments] Total fragments: 11
    ///   [download] Destination: im orb [gEdOmal1A6Q].f299.mp4
    ///   WARNING: The download speed shown is only of one thread. This is a known issue
    ///   [download] 100% of  100.96MiB in 00:00:09 at 10.87MiB/s
    ///   [dashsegments] Total fragments: 2
    ///   [download] Destination: im orb [gEdOmal1A6Q].f251.webm
    ///   WARNING: The download speed shown is only of one thread. This is a known issue
    ///   [download] 100% of   15.42MiB in 00:00:01 at 9.39MiB/s
    ///   [Merger] Merging formats into "im orb [gEdOmal1A6Q].mkv"
    ///   Deleting original file im orb [gEdOmal1A6Q].f299.mp4 (pass -k to keep)
    ///   Deleting original file im orb [gEdOmal1A6Q].f251.webm (pass -k to keep)
    ///   [EmbedSubtitle] There aren't any subtitles to embed
    ///   [Metadata] Adding metadata to "im orb [gEdOmal1A6Q].mkv"
    ///
    ///   [Cookies] Extracting cookies from firefox
    ///   [Cookies] Extracted 2450 cookies from firefox
    ///   [youtube] Extracting URL: https://www.youtube.com/watch?v=gEdOmal1A6Q
    ///   [youtube] gEdOmal1A6Q: Downloading webpage
    ///   [youtube] gEdOmal1A6Q: Downloading android player API JSON
    ///   [info] gEdOmal1A6Q: Downloading 1 format(s): 299+251
    ///   [info] There's no subtitles for the requested languages
    ///   [info] Writing video metadata as JSON to: im orb [gEdOmal1A6Q].info.json
    ///   [dashsegments] Total fragments: 11
    ///   [download] Destination: im orb [gEdOmal1A6Q].f299.mp4
    ///   WARNING: The download speed shown is only of one thread. This is a known issue
    ///   [download]   3.2% of ~ 110.00MiB at    1.83MiB/s ETA 00:58 (frag 0/11)
    ///

    pub fn parse_line(&mut self, line: &str) {
        self.last_output = Some(line.to_string());
        self.last_update = chrono::Utc::now();

        if line.starts_with("[download_progress]") {
            // Live downloads
            // [download]   33.90MiB at  587.08KiB/s (00:00:48) (frag 171)
            // VOD downloads
            // [download]   3.2% of ~ 110.00MiB at    1.83MiB/s ETA 00:58 (frag 0/11)

            // lazy_static! {
            //     static ref ProgressBarRegex: Regex = Regex::new(r"
            //         (?x)
            //         \[download\]\s+
            //         (
            //             (?P<percentage>\d+\.\d+)%\s+of\s+~\s+(?P<total_size>\d+\.\d+\w+)
            //             |
            //             (?P<current_size>\d+\.\d+\w+)
            //         )
            //         \s+at\s+(?P<download_speed>\d+\.\d+\w+\/s\s+)?
            //         (
            //             (ETA\s+(?P<eta>\d+:\d+))
            //             |
            //             \((?P<elapsed_time>\d+:\d+:\d+)\)
            //         )
            //         \s+\(frag\s+(?P<current_fragments>\d+)(\/(?P<total_fragment>\d+))?\)
            //     ").unwrap().expect("Failed to compile YTDlp progress bar regex");
            // }
            //
            // let progress_bar_captures = line.captures(&ProgressBarRegex).unwrap();
            // if progress_bar_captures {
            //     self.state = YTDState::Recording;
            //
            //     self.video_fragments = x.trim().parse().ok();
            //     self.audio_fragments = x.trim().parse().ok();
            //
            //     if let Some(x) = parts.next() {
            //         self.total_size = Some(strip_ansi(x.trim()));
            //     };
            // }


            // [download_progress]
            // {
            //   "percentage": "%(progress._percent_str)s",
            //   "total_size": "%( progress._total_bytes_str)s",
            //   "estimated_total_size": "%(progress._total_bytes_estimate_str)s",
            //   "downloaded_size": "%(progress._downloaded_bytes_str)s",
            //   "speed": "%(progress._speed_str)s",
            //   "eta": "%(progress._eta_str)s",
            //   "elapsed_time": "%(progress._elapsed_str)s",
            //   "total_fragments": "%(progress.fragment_count)s",
            //   "current_fragment_count": "%(progress.fragment_index)s",
            //   "format": "%(info.format)s"
            // }

            let parsed_line = json::parse(&line.replace(&"[download_progress]", &"")).unwrap();

            return;
        } else if line.starts_with("Audio Fragments: ") {
            self.state = YTDState::Recording;
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
        // let line = if self.version == Some("0.3.2".into())
        //     && line.len() > 20
        //     && line.chars().nth(4) == Some('/')
        // {
        //     line[20..].trim()
        // } else {
        //     line
        // };

        if self.version == None && line.starts_with("ytarchive ") {
            self.version = Some(strip_ansi(&line[10..]));
        } else if self.video_quality == None && line.starts_with("Selected quality: ") {
            self.video_quality = Some(strip_ansi(&line[18..]));
        } else if line.starts_with("Stream starts at ") {
            let date = DateTime::parse_from_rfc3339(&line[17..42])
                .ok()
                .map(|d| d.into());
            self.state = YTDState::Waiting(date);
        } else if line.starts_with("Stream is ") || line.starts_with("Waiting for stream") {
            self.state = YTDState::Waiting(None);
        } else if line.starts_with("Muxing final file") {
            self.state = YTDState::Muxing;
        } else if line.starts_with("Livestream has been processed") {
            self.state = YTDState::AlreadyProcessed;
        } else if line.starts_with("Livestream has ended and is being processed")
            || line.contains("use yt-dlp to download it.")
        {
            self.state = YTDState::Ended;
        } else if line.starts_with("Final file: ") {
            self.state = YTDState::Finished;
            self.output_file = Some(strip_ansi(&line[12..]));
        } else if line.contains("User Interrupt") {
            self.state = YTDState::Interrupted;
        } else if line.contains("Error retrieving player response")
            || line.contains("unable to retrieve")
            || line.contains("error writing the muxcmd file")
            || line.contains("Something must have gone wrong with ffmpeg")
            || line.contains("At least one error occurred")
        {
            self.state = YTDState::Errored;
        } else if line.trim().is_empty()
            || line.contains("Loaded cookie file")
            || line.starts_with("Video Title: ")
            || line.starts_with("Channel: ")
            || line.starts_with("Waiting for this time to elapse")
            || line.starts_with("Download Finished")
        {
            // Ignore
        } else {
            warn!("Unknown yt-dlp output: {}", line);
        }
    }
}
