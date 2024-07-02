mod common;

#[test]
fn multiple_clients_one_receiver() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_receiver(Some(vec!["--port=45001".to_string()]));

    let args = vec!["client", "--parallel=2", "--port=45001", "--multiplex-port-receiver=sharding"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn multiple_clients_multiple_receiver() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_receiver(Some(vec!["--port=45101".to_string(), "--parallel=2".to_string()]));

    let args = vec!["client", "--parallel=2", "--port=45101"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}
