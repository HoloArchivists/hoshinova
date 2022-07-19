/*
use super::{Message, Module, Notification, Task, TaskStatus};
use crate::{
    config,
    msgbus::{BusRx, BusTx},
};
use anyhow::{anyhow, Result};

pub struct Discord<'a> {
    config: &'a config::Config,
}

impl<'a> Module<'a> for Discord<'a> {
    fn new(config: &'a config::Config) -> Self {
        Self { config }
    }

    fn run(&self, _tx: &BusTx<Message>, _rx: &mut BusRx<Message>) -> Result<()> {
        Ok(())
    }
}
*/
