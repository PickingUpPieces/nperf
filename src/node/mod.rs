use std::sync::mpsc;
use crate::util::{statistic::Statistic, IOModel};

pub mod client;
pub mod server;

pub trait Node {
    fn run(&mut self, io_model: IOModel, tx: mpsc::Sender<Option<Statistic>>) -> Result<Statistic, &'static str>;
    fn io_wait(&mut self, io_model: IOModel) -> Result<(), &'static str>;
}
