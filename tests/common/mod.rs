use std::{process::Command, thread::{self, JoinHandle}};
use assert_cmd::prelude::*;

#[allow(dead_code)]
pub fn start_nperf_receiver(args: Option<Vec<String>>) -> JoinHandle<()> {
    let handle = thread::spawn(|| {
        let mut cmd = Command::cargo_bin("nperf").unwrap();
        cmd.arg("receiver");
        for arg in args.unwrap_or_default() {
            cmd.arg(arg);
        }
        cmd.assert().success();
    });

    std::thread::sleep(std::time::Duration::from_secs(2)); // Wait for receiver to start
    handle
}

#[allow(dead_code)]
pub fn start_nperf_sender(args: Option<Vec<String>>) -> JoinHandle<()> {
    thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(3)); // Wait for receiver to start
        let mut cmd = Command::cargo_bin("nperf").unwrap();
        cmd.arg("sender");
        for arg in args.unwrap_or_default() {
            cmd.arg(arg);
        }
        cmd.output().unwrap();
    })
}