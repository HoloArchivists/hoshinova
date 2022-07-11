use anyhow::Result;
use bus::BusReader;
use std::sync::mpsc;

pub mod scraper;

#[derive(Debug, Clone)]
pub enum Message {
    Task(Task),
    Quit,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub title: String,
    pub video_id: String,
    pub channel_name: String,
    pub channel_id: String,
}

pub trait Module<T = Message> {
    fn run(&self, send: mpsc::SyncSender<T>, recv: &mut BusReader<T>) -> Result<()>;
}
