#[macro_use]
extern crate log;
use anyhow::{anyhow, Result};
use bus::Bus;
use clap::Parser;
use module::Module;
use std::process::Command;
use std::sync::mpsc;

mod config;
mod module;

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
    let mut bus = Bus::new(32);
    let (tx, mix_rx) = mpsc::sync_channel(32);

    // Set up modules
    let mut modules = Vec::new();
    for i in 0..config.channel.len() {
        let scraper = module::scraper::Scraper::new(&config, i);
        modules.push(scraper);
    }

    // Set up scoped threads
    crossbeam::scope(|s| {
        // Start modules
        for module in modules {
            let tx = tx.clone();
            let mut rx = bus.add_rx();

            s.spawn(move |_| {
                if let Err(e) = module.run(tx, &mut rx) {
                    error!("{}", e);
                }
            });
        }

        // Start message dispatcher
        s.spawn(move |_| {
            for m in mix_rx.iter() {
                bus.broadcast(m);
            }
        });
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
