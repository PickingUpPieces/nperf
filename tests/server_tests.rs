mod common;

#[test]
fn test_server_send() -> Result<(), Box<dyn std::error::Error>>{
    common::start_nperf_client(Some(vec!["--port".to_string(), "45001".to_string()]));

    let args = vec!["server", "--port", "45001"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    Ok(())
}

#[test]
fn test_server_sendmsg() -> Result<(), Box<dyn std::error::Error>>{
    common::start_nperf_client(Some(vec!["--port".to_string(), "45002".to_string()]));

    let args = vec!["server", "--with-msg", "--port", "45002"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    Ok(())
}

#[test]
fn test_server_sendmmsg() -> Result<(), Box<dyn std::error::Error>>{
    common::start_nperf_client(Some(vec!["--port".to_string(), "45003".to_string()]));

    let args = vec!["server", "--with-mmsg", "--with-mmsg-amount=20", "--port", "45003"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    Ok(())
}