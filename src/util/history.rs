use std::time::Duration;
use log::debug;

#[derive(Debug)]
pub struct History {
    pub start_time: std::time::Instant,
    pub end_time: std::time::Instant,
    total_time: std::time::Duration,
    total_data: f64,
    pub amount_datagrams: u64,
    pub amount_data_bytes: usize,
    pub amount_reordered_datagrams: u64,
    pub amount_duplicated_datagrams: u64,
    pub amount_omitted_datagrams: i64,
    data_rate: f64,
    packet_loss: f64,
}

impl History {
    pub fn new() -> History {
        History {
            start_time: std::time::Instant::now(),
            end_time: std::time::Instant::now(),
            total_time: Duration::new(0, 0),
            total_data: 0.0,
            amount_datagrams: 0,
            amount_data_bytes: 0,
            amount_reordered_datagrams: 0,
            amount_duplicated_datagrams: 0,
            amount_omitted_datagrams: 0,
            data_rate: 0.0,
            packet_loss: 0.0,
        }
    }

    fn update(&mut self) {
        debug!("Updating history...");
        self.total_data = self.calculate_total_data();
        self.total_time = self.calculate_total_time();
        self.data_rate = self.calculate_data_rate();
        self.packet_loss = self.calculate_packet_loss();
        debug!("History updated: {:?}", self);
    }

    pub fn print(&mut self) {
        self.update();
        println!("Total time: {:.2}s", self.total_time.as_secs_f64());
        println!("Total data: {:.2} GiBytes", self.total_data);
        println!("Amount of datagrams: {}", self.amount_datagrams);
        println!("Amount of reordered datagrams: {}", self.amount_reordered_datagrams);
        println!("Amount of duplicated datagrams: {}", self.amount_duplicated_datagrams);
        println!("Amount of omitted datagrams: {}", self.amount_omitted_datagrams);
        println!("Data rate: {:.2} GiBytes/s / {:.2} Gibit/s", self.data_rate, (self.data_rate * 8.0));
        println!("Packet loss: {:.2}%", self.packet_loss);
    }

    fn calculate_total_data(&self) -> f64 {
        let total_data = self.amount_data_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
        total_data 
    }
    
    fn calculate_data_rate(&self) -> f64{
        let elapsed_time = self.end_time - self.start_time;
        let elapsed_time_in_seconds = elapsed_time.as_secs_f64();
        let data_rate = self.total_data / elapsed_time_in_seconds;
        data_rate
    }
    
    fn calculate_packet_loss(&self) -> f64 {
        let packet_loss = (self.amount_omitted_datagrams as f64 / self.amount_datagrams as f64) * 100.0;
        packet_loss
    }
    
    fn calculate_total_time(&self) -> std::time::Duration {
        let total_time = self.end_time - self.start_time;
        total_time
    }
}
