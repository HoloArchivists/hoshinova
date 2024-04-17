use super::super::{Message, Module, Notification, Task, TaskStatus};
use super::{RecorderState, VideoStatus};
use crate::msgbus::BusTx;
use crate::{config::Config, module::RecordingStatus};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration, Local};
use lazy_static::lazy_static;
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
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

pub struct YTDlp {
    config: Arc<RwLock<Config>>,
    active_ids: Arc<RwLock<HashSet<String>>>,
}

const PROGRESS_BAR_FORMAT_MAP: [[&str; 2]; 10] = [
    ["percentage", "progress._percent_str"],
    ["total_size", "progress._total_bytes_str"],
    ["estimated_total_size", "progress._total_bytes_estimate_str"],
    ["downloaded_size", "progress._downloaded_bytes_str"],
    ["speed", "progress._speed_str"],
    ["eta", "progress._eta_str"],
    ["elapsed_time", "progress._elapsed_str"],
    ["total_fragments", "progress.fragment_count"],
    ["current_fragment_count", "progress.fragment_index"],
    ["format", "info.format"],
];

impl YTDlp {
    pub async fn record(cfg: Config, task: Task, bus: &mut BusTx<Message>) -> Result<()> {
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
        if !args.contains(&"--wait-for-video".to_string()) {
            // --wait-for-video requires an arg dictating how often to poll, but at least for youtube it's appears to be ignored and yt-dlp uses the scheduled start time instead.
            args.extend(vec![
                "--wait-for-video".to_string(),
                "10".to_string(),
            ]);
        }

        // Add the --live-from-start flag if not present
        if !args.contains(&"--live-from-start".to_string()) {
            args.push("--live-from-start".to_string());
        }

        // Add the --no-colors flag if not present to not output ANSI codes
        if !args.contains(&"--no-colors".to_string()) {
            args.push("--no-colors".to_string());
        }

        if !args.contains(&"--newline".to_string()) {
            args.push("--newline".to_string());
        }

        let progress_bar_template = PROGRESS_BAR_FORMAT_MAP.map(|x| format!("%({})s", x[1]) ).join(",");
        args.extend(vec![
            "--progress-template".to_string(),
            format!("[download_progress] {progress_bar_template}\n").to_string(),
        ]);

        args.extend(vec![
            "--exec".to_string(),
            r#""echo '[download_finished] output_file: (filepath,_filename|)q'""#.to_string(),
        ]);

        args.extend(vec![
            format!("https://www.youtube.com/watch?v={}", task.video_id),
            cfg.quality.clone(),
        ]);

        // TODO: This code almost completely same between ytarchive and yt-dlp. Share it.

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
        let mut parser = YTDOutputParser::new();
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

            let old_state = parser.video_status.state.clone();
            parser.parse_line(&line);

            // Push the current status to the bus
            if let Err(_) = bus
                .send(Message::RecordingStatus(RecordingStatus {
                    task: task.clone(),
                    status: parser.video_status.clone(),
                }))
                .await
            {
                break;
            }

            // Check if status changed
            if old_state == parser.video_status.state {
                continue;
            }

            let message = match parser.video_status.state {
                RecorderState::Waiting(_) => {
                    info!("{} Waiting for stream to go live", task_name);
                    Some(Message::ToNotify(Notification {
                        task: task.clone(),
                        status: TaskStatus::Waiting,
                    }))
                }
                RecorderState::Recording => {
                    info!("{} Recording started", task_name);
                    Some(Message::ToNotify(Notification {
                        task: task.clone(),
                        status: TaskStatus::Recording,
                    }))
                }
                RecorderState::Finished => {
                    info!("{} Recording finished", task_name);
                    Some(Message::ToNotify(Notification {
                        task: task.clone(),
                        status: TaskStatus::Done,
                    }))
                }
                RecorderState::AlreadyProcessed => {
                    info!("{} Video already processed, skipping", task_name);
                    None
                }
                RecorderState::Interrupted => {
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

        trace!("{} Status loop exited: {:?}", task_name, parser.video_status);

        // Wait for threads to finish
        let (r_wait, r_stdout, r_stderr) = futures::join!(h_wait, h_stdout, h_stderr);
        trace!("{} Process monitor exited: {:?}", task_name, r_wait);
        trace!("{} Stdout monitor quit: {:?}", task_name, r_stdout);
        trace!("{} Stderr monitor quit: {:?}", task_name, r_stderr);

        // Skip moving files if it didn't finish
        if parser.video_status.state != RecorderState::Finished {
            return Ok(());
        }

        // Move the video to the output directory
        let frompath = parser.video_status
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

pub struct YTDOutputParser {
    video_status: VideoStatus,
}

impl YTDOutputParser {
    pub fn new() -> Self {
        Self {
            video_status: VideoStatus::new()
        }
    }

    /// parse_line parses a line of output from the yt-dlp process.
    ///
    /// Sample output:
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
    ///   1: [download_progress]   1.2%,       N/A,       1.17GiB,  47.86MiB,   5.61MiB/s,Unknown,00:00:08,NA,278,299 - 1920x1080 (DASH video)
    ///   2: [download_progress]   1.2%,       N/A,       40MiB,   8.56MiB, 149.68KiB/s,Unknown,00:00:08,NA,414,140 - audio only (DASH audio)
    ///   [dashsegments] Total fragments: 2
    ///   [download] Destination: im orb [gEdOmal1A6Q].f251.webm
    ///   WARNING: The download speed shown is only of one thread. This is a known issue
    ///   [download] 100% of   15.42MiB in 00:00:01 at 9.39MiB/s
    ///   [Merger] Merging formats into "im orb [gEdOmal1A6Q].mkv"
    ///   Deleting original file im orb [gEdOmal1A6Q].f299.mp4 (pass -k to keep)
    ///   Deleting original file im orb [gEdOmal1A6Q].f251.webm (pass -k to keep)
    ///   [EmbedSubtitle] There aren't any subtitles to embed
    ///   [Metadata] Adding metadata to "im orb [gEdOmal1A6Q].mkv"
    pub fn parse_line(&mut self, line: &str) {
        self.video_status.last_output = Some(line.to_string());
        self.video_status.last_update = chrono::Utc::now();

        if line.contains("[download_progress]") {
            self.video_status.state = RecorderState::Recording;
            //
            let re = Regex::new(r"(\d:\s)?\[download_progress\]").unwrap();
            let line = re.replace(line, "");
            let line_values: Vec<_> = line.split(",").map(|x| x.trim()).collect();

            let parsed_line: HashMap<String, String> = PROGRESS_BAR_FORMAT_MAP.map(|x| x[0] ).iter().zip(
                line_values.iter()
            ).map(|(x, y)| (x.to_string(), y.to_string())).collect();
            self.video_status.state = RecorderState::Recording;
            self.video_status.last_update = chrono::Utc::now();

            let total_size = parsed_line.get("total_size");
            if !total_size.eq(&Some(&"N/A".to_string())) {
                self.video_status.total_size = total_size.cloned();
            } else {
                self.video_status.total_size = parsed_line.get("estimated_total_size").cloned();
            };

            // This works a bit different than ytarchive.
            // This will be something like "299 - 1920x1080 (DASH video)". It's the youtube format and it's specific to the track so a download will have multiple.
            // Setting this in the format property probably isn't right
            self.video_status.video_quality = parsed_line.get("format").cloned();

            // Hacky as this will only work DASH live streams. There is probably a better way to differentiate audio track from video track based format that will work in all cases.
            if Regex::new(r"\d+x\d+").unwrap().is_match( self.video_status.video_quality.as_ref().unwrap()) {
                self.video_status.video_fragments = parsed_line.get("current_fragment_count").unwrap().parse().ok();
            } else if self.video_status.video_quality.as_ref().unwrap().contains("audio only") {
                self.video_status.audio_fragments = parsed_line.get("current_fragment_count").unwrap().parse().ok();
            }

            return;
        }

        let waiting_text = "[wait] Remaining time until next attempt:"; // Does it make sense to have this as a variable? Move to constant?
        if line.starts_with(waiting_text) {
            // I like using split as it seems less error prone than referencing a specific position in the string, like is done in some places in ytarchive module.
            // Not sure if it really matters or if there is a performance implication of doing it this way.
            // [wait] Remaining time until next attempt: 759:35:3459:59:52
            let duration = line.rsplit(waiting_text).next().unwrap();

            let mut duration_split = duration.rsplit(":").to_owned();

            let mut seconds: i64 = duration_split.next().unwrap().parse().unwrap();
            if let Some(minutes) = duration_split.next() {
                let minutes: i64 = minutes.parse().unwrap();
                seconds += minutes * 60;
            };
            if let Some(hours) = duration_split.next() {
                let hours: i64 = hours.parse().unwrap();
                seconds += hours * 60 * 60;
            };

            let date: DateTime<Utc> = (Local::now() + Duration::seconds(seconds)).with_timezone(&Utc);

            self.video_status.state = RecorderState::Waiting(Some(date));
        } else if line.starts_with("[wait]") {
            // Trying to handle the case when we are just waiting for the stream and there isn't a timestamp returned.
            // I'm not sure of the yt-dlp output in that case.
            self.video_status.state = RecorderState::Waiting(None);
        } else if line.starts_with("[Merger]") ||
            line.starts_with("[Metadata]") || // yt-dlp is adding metadata to file. It can take some time depending on disk speed. For now treat it the same as muxing.
            line.starts_with("[EmbedSubtitle]") // yt-dlp is adding subtitles to file. It can take a while depending on disk speed. For now treat it the same as muxing.
        {
            self.video_status.state = RecorderState::Muxing;
        } else if line.starts_with("[download_finished]") {
            self.video_status.state = RecorderState::Finished;
            self.video_status.output_file =  Some(line.rsplit("[download_finished] output_file: ").next().unwrap().trim().to_string());
        }  else if line.contains("ERROR: Interrupted by user") {
            self.video_status.state = RecorderState::Interrupted;
        } else if line.starts_with("ERROR:") { // Need to investigate if this is the best way to catch errors
            self.video_status.state = RecorderState::Errored;
        } else if line.trim().is_empty()
            || line.starts_with("[Cookies]")
            || line.starts_with("[youtube]")
            || line.starts_with("[info]")
            || line.starts_with("[dashsegments]")
            || line.starts_with("WARNING:")
            || line.starts_with("[download]")
            || line.starts_with("[generic]")
        {
            // There are probably more that need to be handled
            // Ignore
        } else {
            warn!("Unknown yt-dlp output: {}", line);
        }

        // RecorderState::Ended and  RecorderState::AlreadyProcessed aren't relevant for yt-dlp
        // May need to add an extra configuration to ignore older videos, or just be fine with scraper.rss ignore_older_than
    }
}

#[cfg(test)]
mod tests {
    use super::YTDOutputParser;
    use super::{RecorderState, VideoStatus};

    #[test]
    fn test_download_progress_parsing() {
        let line = "[download_progress]   2.2%,       N/A,   3.17GiB,  70.03MiB,   1.99MiB/s,01:04,00:00:01,325,7,299 - 1920x1080 (1080p60)";

        let mut parser = YTDOutputParser::new();
        parser.parse_line(line);

        assert_eq!(
            parser.video_status.last_output,
            Some(line.into())
        );

        assert_eq!(
            parser.video_status.state,
            RecorderState::Recording
        );

        assert_eq!(
            parser.video_status.total_size,
            Some("3.17GiB".into())
        );

        assert_eq!(
            parser.video_status.video_quality,
            Some("299 - 1920x1080 (1080p60)".into())
        );

        assert_eq!(
            parser.video_status.video_fragments,
            Some(7)
        );
    }
}
