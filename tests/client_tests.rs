use std::process::Command;
use assert_cmd::prelude::*; // Add methods on commands

mod common;

// TODO: Test client send 
// TODO: Test client sendmsg
// TODO: Test client sendmmsg

#[test]
fn test_client_send() -> Result<(), Box<dyn std::error::Error>>{
    common::start_server();

    let mut cmd = Command::cargo_bin("nperf").unwrap();
    cmd.arg("client").arg("--json");

    println!("{:?}", cmd.output().unwrap().stdout);
    // Parse json output
    Ok(())
}