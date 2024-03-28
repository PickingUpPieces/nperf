mod common;

// Test client send/sendmsg/sendmmsg with server send/sendmsg/sendmmsg in different combinations

#[test]
fn sendmsg_recvmsg() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--port=45001".to_string()]));

    let args = vec!["client", "--port=45001"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn sendmmsg_recvmsg() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--port=45101".to_string()]));

    let args = vec!["client", "--exchange-function=mmsg", "--port=45101"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn sendmmsg_recvmmsg() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--exchange-function=mmsg".to_string(), "--port=45201".to_string()]));

    let args = vec!["client", "--exchange-function=mmsg", "--port=45201"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn sendmsg_recvmmsg() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--exchange-function=mmsg".to_string(), "--port=45301".to_string()]));

    let args = vec!["client", "--port=45301"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

