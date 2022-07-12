#[macro_use]
extern crate log;
use crate::module::Module;
use crate::msgbus::MessageBus;
use anyhow::{anyhow, Result};
use clap::Parser;
use signal_hook::iterator::Signals;
use std::process::Command;

mod config;
mod module;
mod msgbus;

/// Garbage ytarchive manager
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the configuration file
    #[clap(short, long, value_parser, default_value = "config.toml")]
    config: String,
}

fn test_ffmpeg() -> Result<String> {
    let cmd = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map_err(|e| anyhow!("Failed to execute ffmpeg: {}", e))?;
    if !cmd.status.success() {
        return Err(anyhow!(
            "Failed to determine ffmpeg version: {}",
            cmd.status
        ));
    }
    let stdout = String::from_utf8_lossy(&cmd.stdout);
    Ok(stdout
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join(" "))
}

fn test_ytarchive(path: &str) -> Result<String> {
    let cmd = Command::new(path)
        .arg("--version")
        .output()
        .map_err(|e| anyhow!("Failed to execute ytarchive: {}", e))?;
    if !cmd.status.success() {
        return Err(anyhow!(
            "Failed to determine ytarchive version: {}",
            cmd.status
        ));
    }
    let stdout = String::from_utf8_lossy(&cmd.stdout);
    Ok(stdout.trim().to_string())
}

macro_rules! run_module {
    ($bus:expr, $scope:expr, $module:expr) => {{
        let tx = $bus.add_tx();
        let mut rx = $bus.add_rx();
        let module = $module;
        $scope.spawn(move |_| {
            if let Err(e) = module.run(&tx, &mut rx) {
                error!("{}", e);
            }
        })
    }};
}

fn run() -> Result<()> {
    // Initialize logging
    env_logger::init();
    info!("hoshinova v{}", env!("CARGO_PKG_VERSION"));

    // Parse command line arguments
    let args = Args::parse();
    debug!("{:?}", args);

    // Load configuration file
    let config = config::load_config(&args.config)
        .map_err(|e| anyhow!("Failed to read config file: {}", e))?;
    debug!("{:?}", config);

    // Make sure ffmpeg and ytarchive are installed
    debug!("Found {}", test_ffmpeg()?);
    debug!(
        "Found {}",
        test_ytarchive(&config.ytarchive.executable_path)?
    );

    // Set up message bus
    let mut bus = MessageBus::new(32);

    // Start threads
    crossbeam::scope(|s| {
        // Set up modules
        for i in 0..config.channel.len() {
            run_module!(bus, s, module::scraper::RSS::new(&config, i));
            run_module!(bus, s, module::recorder::YTArchive::new(&config));
        }

        // Listen for signals
        let closer = bus.add_tx();
        s.spawn(move |_| {
            let mut signals =
                Signals::new(&[signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM])
                    .expect("Failed to create signal iterator");
            for _ in signals.forever() {
                info!("Received signal, shutting down");
                closer.close().expect("Failed to close message bus");
                return;
            }
        });

        // Start message dispatcher
        s.spawn(|_| bus.start());
    })
    .map_err(|e| anyhow!("Could not exit cleanly: {:?}", e))
}

fn main() {
    if let Err(e) = run() {
        error!("{}", e);
        std::process::exit(1);
    }
    debug!("Exit");
}
