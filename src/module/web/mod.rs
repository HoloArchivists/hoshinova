use super::{recorder::VideoStatus, Message, Module, Task};
use crate::{
    config::{Config, WebserverConfig},
    msgbus::BusTx,
};
use actix_web::{web::Data, App, HttpServer};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::{
    select,
    sync::{mpsc, RwLock},
};
use ts_rs::TS;

mod handler;

pub struct WebServer {
    config: Arc<RwLock<Config>>,
}

#[derive(Debug, Clone, TS, Serialize)]
#[ts(export, export_to = "web/src/bindings/")]
pub struct TaskWithStatus {
    pub task: Task,
    pub status: VideoStatus,
}

type TaskMap = Data<RwLock<HashMap<String, TaskWithStatus>>>;

impl WebServer {
    /// Return the webserver configuration
    async fn get_wsconfig(&self) -> Option<WebserverConfig> {
        let config = self.config.read().await;
        config.webserver.clone()
    }

    async fn bus_listen_loop(
        &self,
        rx: &mut mpsc::Receiver<Message>,
        tasks: TaskMap,
    ) -> Result<()> {
        while let Some(msg) = rx.recv().await {
            match msg {
                Message::RecordingStatus(recstat) => {
                    let id = recstat.task.video_id.clone();
                    let mut tasks = tasks.write().await;
                    tasks.insert(
                        id,
                        TaskWithStatus {
                            task: recstat.task,
                            status: recstat.status,
                        },
                    );
                }
                _ => (),
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Module for WebServer {
    fn new(config: Arc<RwLock<Config>>) -> Self {
        Self { config }
    }

    async fn run(&self, tx: &BusTx<Message>, rx: &mut mpsc::Receiver<Message>) -> Result<()> {
        // Get the configuration
        let ws_cfg = match self.get_wsconfig().await {
            Some(cfg) if cfg.bind_address.is_some() || cfg.unix_path.is_some() => cfg,
            _ => {
                debug!("No webserver configured");

                // Noop read the bus
                while rx.recv().await.is_some() {}
                return Ok(());
            }
        };

        // Create a HashMap to hold the tasks
        let tasks = Data::new(RwLock::new(HashMap::new()));

        // Listen to the bus
        let busll = self.bus_listen_loop(rx, tasks.clone());

        // Set up webserver
        let config = Data::new(self.config.clone());
        let tx = Data::new(tx.clone());
        let ws = {
            let mut server = HttpServer::new(move || {
                App::new()
                    .app_data(config.clone())
                    .app_data(tx.clone())
                    .app_data(tasks.clone())
                    .configure(handler::configure)
            })
            .disable_signals();
            if let Some(addr) = ws_cfg.bind_address.as_ref() {
                info!("Starting webserver on {}", addr);
                server = server
                    .bind(addr.clone())
                    .with_context(|| format!("Failed to bind webserver to {}", addr))?;
            }
            #[cfg(unix)]
            if let Some(path) = ws_cfg.unix_path.as_ref() {
                info!("Starting webserver on {}", path);
                server = server
                    .bind_uds(path.clone())
                    .with_context(|| format!("Failed to bind webserver to {}", path))?;
            }
            server.run()
        };

        let handle = ws.handle();

        select! {
            ret = ws => {
                // Close the receiver if the webserver stops
                rx.close();
                ret.context("Webserver stopped unexpectedly")
            },
            ret = busll => {
                // Stop the webserver if the bus loop stops
                handle.stop(true).await;
                ret.context("Bus loop stopped unexpectedly")
            }
        }
    }
}
