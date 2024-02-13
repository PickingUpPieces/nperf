use std::{process::Command, thread};
use assert_cmd::prelude::*;
#[allow(dead_code)]
pub fn start_server() {
    // Start server as a separate thread
    thread::spawn(|| {
        let mut cmd = Command::cargo_bin("nperf").unwrap();
        cmd.arg("server").output().unwrap();
    });

    std::thread::sleep(std::time::Duration::from_secs(1)); // Wait for server to start
}

#[allow(dead_code)]
pub fn start_client() {
    todo!()
}

#[allow(dead_code)]
pub fn get_client_config() {
    todo!()
}