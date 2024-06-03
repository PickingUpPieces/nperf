mod common;

#[test]
fn uring_fillmode_topup() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_client(Some(vec!["--port=45001".to_string(), "--with-gsro".to_string()]));

    let args = vec!["server", "--io-model=io-uring", "--port=45001", "--uring-sq-mode=topup"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn uring_fillmode_syscall() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_client(Some(vec!["--port=45001".to_string(), "--with-gsro".to_string()]));

    let args = vec!["server", "--io-model=io-uring", "--port=45001", "--uring-sq-mode=syscall"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}
