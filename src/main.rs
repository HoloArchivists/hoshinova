#[macro_use]
extern crate log;
use crate::module::Module;
use crate::msgbus::MessageBus;
use anyhow::{anyhow, Result};
use clap::Parser;
use std::{process::Command, sync::Arc};
use tokio::sync::RwLock;

mod config;
mod module;
mod msgbus;
mod youtube;

pub static APP_NAME: &str = concat!(
    env!("CARGO_PKG_NAME"),
    " v",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("GIT_HASH_SHORT"),
    ")"
);
pub static APP_USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (+",
    env!("CARGO_PKG_HOMEPAGE"),
    ")"
);

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

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    info!("{}", APP_NAME);
    debug!("Git hash: {}", env!("GIT_HASH"));
    debug!("Built on: {}", env!("BUILD_TIME"));

    // Parse command line arguments
    let args = Args::parse();
    debug!("{:?}", args);

    // Load configuration file
    let config = config::load_config(&args.config)
        .await
        .map_err(|e| anyhow!("Failed to read config file: {}", e))?;
    debug!("{:?}", config);

    // Make sure ffmpeg and ytarchive are installed
    debug!("Found {}", test_ffmpeg()?);
    debug!(
        "Found {}",
        test_ytarchive(&config.ytarchive.executable_path)?
    );

    // Set up message bus
    let mut bus = MessageBus::new(65_536);

    // Set up modules
    macro_rules! run_module {
        ($bus:expr, $module:expr) => {{
            let tx = $bus.add_tx();
            let mut rx = $bus.add_rx();
            let module = $module;
            tokio::spawn(async move {
                if let Err(e) = module.run(&tx, &mut rx).await {
                    error!("{}", e);
                }
            })
        }};
    }

    let config = Arc::new(RwLock::new(config));
    let h_scraper = run_module!(bus, module::scraper::RSS::new(config.clone()));
    let h_recorder = run_module!(bus, module::recorder::RecorderRunner::new(config.clone()));
    let h_notifier = run_module!(bus, module::notifier::Discord::new(config.clone()));
    let h_webserver = run_module!(bus, module::web::WebServer::new(config.clone()));

    // Listen for signals
    let closer = bus.add_tx();
    let h_signal = tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Unable to listen for SIGINT");

        info!("Received signal, shutting down");
        closer.close().await.expect("Failed to close message bus");
    });

    // Start message dispatcher
    let h_bus = tokio::task::spawn(async move { bus.start().await });

    // Wait for all tasks to finish
    futures::try_join!(
        h_scraper,
        h_recorder,
        h_notifier,
        h_signal,
        h_bus,
        h_webserver,
    )
    .map(|_| ())
    .map_err(|e| anyhow!("Task errored: {}", e))
}
