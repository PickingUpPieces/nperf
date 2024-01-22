
pub mod client;
pub mod server;

pub trait Node {
    fn run(&mut self) -> Result<(), &'static str>;
}