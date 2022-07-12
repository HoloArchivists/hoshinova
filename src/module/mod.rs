use crate::msgbus::BusTrx;
use anyhow::Result;
use std::fmt::Debug;

pub mod scraper;

#[derive(Debug, Clone)]
pub enum Message {
    Task(Task),
}

#[derive(Debug, Clone)]
pub struct Task {
    pub title: String,
    pub video_id: String,
    pub channel_name: String,
    pub channel_id: String,
}

pub trait Module<T: Debug + Clone + Sync = Message> {
    fn run(&self, bus: &mut BusTrx<T>) -> Result<()>;
}
