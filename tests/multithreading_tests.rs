mod common;

#[test]
fn multiple_clients_one_server() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--port".to_string(), "45000".to_string()]));

    let args = vec!["client", "--with-msg", "--parallel", "2", "--port", "45000"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn multiple_clients_multiple_server() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--parallel".to_string(), "2".to_string()]));

    let args = vec!["client", "--with-msg", "--parallel", "2", "--port", "45001"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn multiple_clients_multiple_server_single_socket() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--parallel".to_string(), "2".to_string(), "--single-socket".to_string(), "--port".to_string(), "45003".to_string()]));

    let args = vec!["client", "--with-msg", "--parallel", "2", "--port", "45003", "--single-socket"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}