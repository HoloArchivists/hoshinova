use super::{recorder::YTAStatus, Message, Module, Task};
use crate::{
    config::{Config, WebserverConfig},
    msgbus::BusTx,
};
use actix_web::{web::Data, App, HttpResponse, HttpServer, Responder};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::{
    select,
    sync::{mpsc, RwLock},
};

pub struct WebServer {
    config: Arc<RwLock<Config>>,
}

#[derive(Debug, Clone)]
pub struct TaskWithStatus {
    pub task: Task,
    pub status: YTAStatus,
}

type TaskMap = Data<RwLock<HashMap<String, TaskWithStatus>>>;

async fn index(data: TaskMap) -> actix_web::Result<impl Responder> {
    Ok(data
        .read()
        .await
        .iter()
        .map(|(k, v)| format!("[{}] {}", k, v.task.title))
        .collect::<Vec<_>>()
        .join("\n"))
    .map(|s| {
        HttpResponse::Ok()
            .insert_header(("Content-Type", "text/plain; charset=utf-8"))
            .body(s)
    })
}

/// Configure routes for the webserver
fn configure(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.route("/", actix_web::web::get().to(index));
}

impl WebServer {
    /// Return the webserver configuration
    async fn get_wsconfig(&self) -> Option<WebserverConfig> {
        let config = &*self.config.read().await;
        config.webserver.to_owned()
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
            Some(cfg) => cfg,
            None => {
                debug!("No webserver configured");
                return Ok(());
            }
        };

        // Create a HashMap to hold the tasks
        let tasks = Data::new(RwLock::new(HashMap::new()));

        // Listen to the bus
        let busll = self.bus_listen_loop(rx, tasks.clone());

        // Set up webserver
        info!("Starting webserver on {}", ws_cfg.bind_address);
        let config = Data::new(self.config.clone());
        let tx = Data::new(tx.clone());
        let ws = HttpServer::new(move || {
            App::new()
                .app_data(config.clone())
                .app_data(tx.clone())
                .app_data(tasks.clone())
                .configure(configure)
        })
        .bind(ws_cfg.bind_address)
        .map_err(|e| anyhow!("Failed to bind to address: {}", e))?
        .run();

        let handle = ws.handle();

        select! {
            ret = ws => {
                // Close the receiver if the webserver stops
                rx.close();
                ret.map_err(|e| anyhow!("Failed to start webserver: {}", e))
            },
            ret = busll => {
                // Stop the webserver if the bus loop stops
                handle.stop(true).await;
                ret.map_err(|e| anyhow!("Bus loop crashed: {}", e))
            }
        }
    }
}