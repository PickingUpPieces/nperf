
mod common;

#[test]
fn client_sharding_server_individual() -> Result<(), Box<dyn std::error::Error>>{
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
fn client_sharding_server_sharing() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--port=45401".to_string(), "--parallel=2".to_string(), "--multiplex-port-server=sharing".to_string()]));

    let args = vec!["client", "--port=45401", "--parallel=2", "--multiplex-port=sharding", "--multiplex-port-server=sharing"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn client_sharing_server_sharing() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--port=45101".to_string(), "--parallel=2".to_string(), "--multiplex-port-server=sharing".to_string()]));

    let args = vec!["client",  "--parallel=2", "--port=45101", "--client-port=46101", "--multiplex-port=sharing", "--multiplex-port-server=sharing"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn client_sharing_server_individual() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--port=45201".to_string(), "--parallel=2".to_string()]));

    let args = vec!["client",  "--parallel=2", "--port=45201", "--client-port=46201", "--multiplex-port=sharing"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn client_individual_server_sharing() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--port=45301".to_string(), "--parallel=2".to_string(), "--multiplex-port-server=sharing".to_string()]));

    let args = vec!["client",  "--parallel=2", "--port=45301", "--client-port=46301", "--multiplex-port-server=sharing"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn client_individual_server_sharding() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--port=48501".to_string(), "--parallel=2".to_string(), "--multiplex-port-server=sharding".to_string()]));

    let args = vec!["client",  "--parallel=2", "--port=48501", "--client-port=46501", "--multiplex-port-server=sharding"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

