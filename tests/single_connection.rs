
mod common;

#[test]
fn test_client_reuseport() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--port=45001".to_string(), "--parallel=2".to_string()]));

    let args = vec!["client", "--port=45001", "--parallel=2", "--multiplex-port=sharding"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn multiple_clients_multiple_server_single_socket() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--parallel=2".to_string(), "--multiplex-port=sharing".to_string(), "--port=45003".to_string()]));

    let args = vec!["client",  "--parallel=2", "--port=45003", "--multiplex-port=sharing"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}