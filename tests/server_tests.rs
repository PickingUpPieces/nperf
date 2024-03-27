mod common;

#[test]
fn test_server_send() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_client(Some(vec!["--exchange-function=normal".to_string(), "--port=45001".to_string()]));

    let args = vec!["server", "--exchange-function=normal", "--port=45001"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn test_server_sendmsg() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_client(Some(vec!["--exchange-function=normal".to_string(), "--port=45002".to_string()]));

    let args = vec!["server", "--port=45002"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn test_server_sendmmsg() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_client(Some(vec!["--exchange-function=normal".to_string(), "--port=45003".to_string()]));

    let args = vec!["server", "--exchange-function=mmsg", "--with-mmsg-amount=20", "--port=45003"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}