mod common;

#[test]
fn uring_normal() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_sender(Some(vec!["--port=45001".to_string(), "--with-gsro".to_string()]));

    let args = vec!["receiver", "--io-model=io-uring", "--port=45001", "--uring-mode=normal"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn uring_multishot() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_sender(Some(vec!["--port=45002".to_string(), "--with-gsro".to_string()]));

    let args = vec!["receiver", "--io-model=io-uring", "--port=45002", "--uring-mode=multishot"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn uring_provided_buffer() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_sender(Some(vec!["--port=45003".to_string(), "--with-gsro".to_string()]));

    let args = vec!["receiver", "--io-model=io-uring", "--port=45003", "--uring-mode=provided-buffer"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}