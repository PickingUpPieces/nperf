mod common;

#[test]
fn test_client_send() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--port".to_string(), "45001".to_string()]));

    let args = vec!["client", "--port", "45001"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn test_client_sendmsg() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--port".to_string(), "45002".to_string()]));

    let args = vec!["client", "--with-msg", "--port", "45002"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        println!("{:?}", x);
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn test_client_sendmmsg() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--port".to_string(), "45003".to_string()]));

    let args = vec!["client", "--with-mmsg", "--with-mmsg-amount=20", "--port", "45003"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}