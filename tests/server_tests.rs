mod common;

#[test]
fn test_receiver_send() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_sender(Some(vec!["--exchange-function=normal".to_string(), "--port=45001".to_string()]));

    let args = vec!["receiver", "--exchange-function=normal", "--port=45001"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn test_receiver_sendmsg() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_sender(Some(vec!["--exchange-function=normal".to_string(), "--port=45101".to_string()]));

    let args = vec!["receiver", "--port=45101"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn test_receiver_sendmmsg() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_sender(Some(vec!["--exchange-function=normal".to_string(), "--port=45201".to_string()]));

    let args = vec!["receiver", "--exchange-function=mmsg", "--with-mmsg-amount=20", "--port=45201"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}