use super::{Message, Module, Notification, Task, TaskStatus};
use crate::{
    config,
    module::RecordingStatus,
    msgbus::{BusRx, BusTx},
};
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{fs, process::Stdio};
use std::{io::Read, path::Path};

pub struct YTArchive<'a> {
    config: &'a config::Config,
}

impl<'a> YTArchive<'a> {
    fn record(&self, task: Task, bus: &BusTx<Message>) -> Result<()> {
        // Ensure the working directory exists
        let cfg = &self.config.ytarchive;
        fs::create_dir_all(&cfg.working_directory)
            .map_err(|e| anyhow!("Failed to create working directory: {}", e))?;

        // Ensure the output directory exists
        fs::create_dir_all(&task.output_directory)
            .map_err(|e| anyhow!("Failed to create output directory: {:?}", e))?;

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

        // Grab stdout/stderr byte iterators
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

        // Create a channel to consolidate stdout and stderr
        let (tx, rx) = crossbeam::channel::unbounded();

        // Flag to mark when the process has exited
        let done = AtomicBool::new(false);

        let status = crossbeam::scope(|s| {
            macro_rules! read_line {
                ($reader:expr, $tx:expr) => {{
                    // Read bytes until a \r or \n is returned
                    let bytes = $reader
                        .by_ref()
                        .take_while(|byte| match byte {
                            Ok(b'\n') | Ok(b'\r') => false,
                            Ok(_) => true,
                            _ => false,
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .unwrap_or(Vec::new());

                    // Skip if there are no bytes
                    if bytes.is_empty() {
                        continue;
                    }

                    // Convert to a string
                    let line = match std::str::from_utf8(&bytes) {
                        Ok(line) => line.to_owned(),
                        Err(e) => {
                            trace!("Failed to read utf8: {:?}", e);
                            return;
                        }
                    };

                    // Send the line to the channel
                    if let Err(e) = $tx.send(line) {
                        trace!("Failed to send line: {:?}", e);
                        return;
                    }
                }};
            }

            // Read stdout
            let h_stdout = s.spawn(|_| {
                while !done.load(Ordering::Relaxed) {
                    read_line!(&mut stdout, tx);
                }
            });

            // Read stderr
            let h_stderr = s.spawn(|_| {
                while !done.load(Ordering::Relaxed) {
                    read_line!(&mut stderr, tx);
                }
            });

            // Parse each line
            let h_status = s.spawn(|_| {
                let mut status = YTAStatus::new();

                for line in rx {
                    // Stop when done
                    if done.load(Ordering::Relaxed) {
                        break;
                    }

                    trace!("[{}][yta:out] {}", task.video_id, line);

                    let old = status.clone();
                    status.parse_line(&line);

                    // Push the current status to the bus
                    if let Err(_) = bus.send(Message::RecordingStatus(RecordingStatus {
                        task: task.clone(),
                        status: status.clone(),
                    })) {
                        break;
                    }

                    // Check if status changed
                    if old.state != status.state {
                        let err = match status.state {
                            YTAState::Waiting(_) => {
                                info!("[{}] Waiting for stream to go live", task.video_id);
                                bus.send(Message::ToNotify(Notification {
                                    task: task.clone(),
                                    status: TaskStatus::Waiting,
                                }))
                            }
                            YTAState::Recording => {
                                info!("[{}] Recording started", task.video_id);
                                bus.send(Message::ToNotify(Notification {
                                    task: task.clone(),
                                    status: TaskStatus::Recording,
                                }))
                            }
                            YTAState::Finished => {
                                info!("[{}] Recording finished", task.video_id);
                                bus.send(Message::ToNotify(Notification {
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
                                bus.send(Message::ToNotify(Notification {
                                    task: task.clone(),
                                    status: TaskStatus::Failed,
                                }))
                            }
                            _ => Ok(()),
                        }
                        .is_err();

                        // Exit the loop if message failed to send
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

            // Send a blank message to unblock the status monitor thread
            let _ = tx.try_send("".into());

            // Wait for threads to finish
            trace!("[{}] Stdout monitor quit: {:?}", video_id, h_stdout.join());
            trace!("[{}] Stderr monitor quit: {:?}", video_id, h_stderr.join());
            let status = h_status.join();
            trace!("[{}] Status monitor quit: {:?}", video_id, status);

            // Return the status
            status.map_err(|e| anyhow!("Status monitor thread panicked: {:?}", e))
        })
        .map_err(|e| anyhow!("Failed to exit: {:?}", e))??;

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
        match fs::rename(frompath, &destpath) {
            Ok(()) => {
                info!(
                    "[{}] Moved output file to {}",
                    task.video_id,
                    destpath.display(),
                );
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
                debug!(
                    "[{}] Failed to rename file to output, trying to copy",
                    task.video_id
                );

                // Copy the file into the output directory
                fs::copy(frompath, &destpath)
                    .map_err(|e| anyhow!("Failed to copy file to output: {:?}", e))?;
                info!(
                    "[{}] Copied output file to {}, removing original",
                    task.video_id,
                    destpath.display(),
                );
                fs::remove_file(frompath)
                    .map_err(|e| anyhow!("Failed to remove original file: {:?}", e))
            }
            Err(e) => {
                error!("[{}] Failed to move output file: {:?}", task.video_id, e);
                Err(anyhow!("Failed to move output file: {:?}", e))
            }
        }
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
