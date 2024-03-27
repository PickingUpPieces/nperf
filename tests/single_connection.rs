
mod common;

#[test]
fn test_client_reuseport() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--port=45001".to_string(), "--parallel=2".to_string()]));

    let args = vec!["client", "--port=45001", "--parallel=2", "--multiplex-port=sharding"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn multiple_clients_multiple_server_single_socket() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--port=45003".to_string(), "--parallel=2".to_string(), "--multiplex-port-server=sharing".to_string()]));

    let args = vec!["client",  "--parallel=2", "--port=45003", "--client-port=47001", "--multiplex-port=sharing", "--multiplex-port-server=sharing"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}