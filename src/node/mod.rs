use crate::util::{statistic::Statistic, IOModel};

pub mod sender;
pub mod receiver;

pub trait Node {
    fn run(&mut self, io_model: IOModel) -> Result<(Statistic, Vec<Statistic>), &'static str>;
    fn io_wait(&mut self, io_model: IOModel) -> Result<(), &'static str>;
}
