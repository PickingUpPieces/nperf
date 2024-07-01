use libc::{getrusage, rusage, RUSAGE_SELF};
use std::time::Instant;

pub struct CpuUtil {
    last_instant: Instant,
    last_usage: rusage,
    first_instant: Instant,
    first_usage: rusage
}

impl CpuUtil {
    pub fn new() -> Self {
        let mut last_usage = unsafe { std::mem::zeroed() };
        unsafe {
            getrusage(RUSAGE_SELF, &mut last_usage);
        }
        CpuUtil {
            last_instant: Instant::now(),
            last_usage,
            first_instant: Instant::now(),
            first_usage: unsafe { std::mem::zeroed() }
        }
    }

    // Very similar code to iperf3, but with some modifications to rustify it
    fn get_cpu_util(&mut self, rusage: libc::rusage, instant: Instant) -> (f64, f64, f64) {
        let now = Instant::now();
        let mut current_usage = unsafe { std::mem::zeroed() };
        unsafe {
            getrusage(RUSAGE_SELF, &mut current_usage);
        }

        let timediff = now.duration_since(instant).as_micros() as f64;
        let userdiff = (current_usage.ru_utime.tv_sec as f64 * 1_000_000.0 + current_usage.ru_utime.tv_usec as f64)
            - (rusage.ru_utime.tv_sec as f64 * 1_000_000.0 + rusage.ru_utime.tv_usec as f64);
        let systemdiff = (current_usage.ru_stime.tv_sec as f64 * 1_000_000.0 + current_usage.ru_stime.tv_usec as f64)
            - (rusage.ru_stime.tv_sec as f64 * 1_000_000.0 + rusage.ru_stime.tv_usec as f64);
        // Calculate total CPU usage: Sum of user and system time differences
        let totaldiff = userdiff + systemdiff; 

        // Update last measurements
        self.last_instant = now;
        self.last_usage = current_usage;
        if self.first_usage.ru_utime.tv_sec == 0 && self.first_usage.ru_stime.tv_sec == 0 {
            self.first_instant = now;
            self.first_usage = current_usage;
        }

        // userspace, system, total cpu time
        ((userdiff / timediff) * 100.0, (systemdiff / timediff) * 100.0, (totaldiff / timediff) * 100.0)
    }

    pub fn get_relative_cpu_util(&mut self) -> (f64, f64, f64) {
        self.get_cpu_util(self.last_usage, self.last_instant)
    }

    pub fn get_absolut_cpu_util(&mut self) -> (f64, f64, f64) {
        self.get_cpu_util(self.first_usage, self.first_instant)
    }
}