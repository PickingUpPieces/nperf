mod common;

#[test]
fn test_client_send() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_receiver(Some(vec!["--port=45001".to_string()]));

    let args = vec!["client", "--exchange-function=normal", "--port=45001"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn test_client_sendmsg() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_receiver(Some(vec!["--port=45101".to_string()]));

    let args = vec!["client", "--port=45101"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn test_client_sendmmsg() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_receiver(Some(vec!["--port=45201".to_string()]));

    let args = vec!["client", "--exchange-function=mmsg", "--with-mmsg-amount=20", "--port=45201"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}