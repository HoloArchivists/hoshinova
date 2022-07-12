use super::{Message, Module, Task};
use crate::{
    config,
    msgbus::{BusRx, BusTx},
};
use anyhow::{anyhow, Result};

pub struct YTArchive<'a> {
    config: &'a config::Config,
}

#[derive(Debug)]
enum RecMsg {
    Done(Task),
    Fail(Task),
    Close,
}

impl<'a> YTArchive<'a> {
    pub fn new(config: &'a config::Config) -> Self {
        Self { config }
    }

    fn record(&self, task: Task, done: crossbeam::channel::Sender<RecMsg>) -> Result<()> {
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
            "Starting YTArchive for {} with args {:?}",
            task.video_id, args
        );
        let mut process = std::process::Command::new(&cfg.executable_path)
            .args(args)
            .current_dir(&cfg.working_directory)
            .spawn()
            .map_err(|e| anyhow!("Failed to start process: {}", e))?;

        debug!("Waiting for process to finish");
        let result = process.wait();
        debug!("Process finished with result {:?}", result);

        debug!("Finished recording {}", task.video_id);
        done.send(RecMsg::Fail(task))?;
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
                        RecMsg::Done(task) => {
                            info!("{}", task.title);
                            if let Err(e) = tx.send(Message::ToNotify(task)) {
                                debug!("Error sending task to be notified: {}", e);
                                return;
                            }
                        }
                        RecMsg::Fail(task) => {
                            error!("{}", task.title);
                            if let Err(e) = tx.send(Message::ToNotify(task)) {
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
                        let ttx = ttx.clone();
                        s.spawn(move |_| self.record(task, ttx));
                    }
                    Ok(a) => debug!("Got other message: {:?}", a),
                    Err(_) => break,
                }
            }

            info!("Waiting for all videos to finish recording");

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
