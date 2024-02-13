use std::{process::Command, thread};
use assert_cmd::prelude::*;
use serde::{Deserialize, Serialize}; // Add methods on commands

pub fn start_server() {
    // Start server as a separate thread
    thread::spawn(|| {
        let mut cmd = Command::cargo_bin("nperf").unwrap();
        cmd.arg("server").output().unwrap();
    });
}

pub fn start_client() {
    todo!()
}

pub fn get_client_config() {
    todo!()
}

pub fn deserialise_output(output: Vec<u8>) {
    // let statistic = serde_json::from_slice(&output).unwrap();
    todo!()
}