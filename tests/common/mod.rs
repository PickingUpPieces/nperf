use std::{process::Command, thread};
use assert_cmd::prelude::*;

#[allow(dead_code)]
pub fn start_nperf_server(args: Option<Vec<String>>) {
    // Start server as a separate thread
    thread::spawn(|| {
        let mut cmd = Command::cargo_bin("nperf").unwrap();
        cmd.arg("server");
        for arg in args.unwrap_or_default() {
            cmd.arg(arg);
        }
        cmd.output().unwrap();
    });

    std::thread::sleep(std::time::Duration::from_secs(1)); // Wait for server to start
}

#[allow(dead_code)]
pub fn start_nperf_client(args: Option<Vec<String>>) {
    // Start server as a separate thread
    thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(3)); // Wait for server to start
        let mut cmd = Command::cargo_bin("nperf").unwrap();
        cmd.arg("client");
        for arg in args.unwrap_or_default() {
            cmd.arg(arg);
        }
        cmd.output().unwrap();
    });

}