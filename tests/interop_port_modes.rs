
mod common;

#[test]
fn sender_sharding_receiver_individual() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_receiver(Some(vec!["--port=45001".to_string(), "--parallel=2".to_string()]));

    let args = vec!["sender", "--port=45001", "--parallel=2", "--multiplex-port=sharding"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn sender_sharding_receiver_sharing() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_receiver(Some(vec!["--port=45401".to_string(), "--parallel=2".to_string(), "--multiplex-port-receiver=sharing".to_string()]));

    let args = vec!["sender", "--port=45401", "--parallel=2", "--multiplex-port=sharding", "--multiplex-port-receiver=sharing"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn sender_sharing_receiver_sharing() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_receiver(Some(vec!["--port=45101".to_string(), "--parallel=2".to_string(), "--multiplex-port-receiver=sharing".to_string()]));

    let args = vec!["sender",  "--parallel=2", "--port=45101", "--sender-port=46101", "--multiplex-port=sharing", "--multiplex-port-receiver=sharing"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn sender_sharing_receiver_individual() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_receiver(Some(vec!["--port=45201".to_string(), "--parallel=2".to_string()]));

    let args = vec!["sender",  "--parallel=2", "--port=45201", "--sender-port=46201", "--multiplex-port=sharing"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn sender_individual_receiver_sharing() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_receiver(Some(vec!["--port=45301".to_string(), "--parallel=2".to_string(), "--multiplex-port-receiver=sharing".to_string()]));

    let args = vec!["sender",  "--parallel=2", "--port=45301", "--sender-port=46301", "--multiplex-port-receiver=sharing"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn sender_individual_receiver_sharding() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_receiver(Some(vec!["--port=48501".to_string(), "--parallel=2".to_string(), "--multiplex-port-receiver=sharding".to_string()]));

    let args = vec!["sender",  "--parallel=2", "--port=48501", "--sender-port=46501", "--multiplex-port-receiver=sharding"];
    let nperf = nperf::nPerf::new().set_args(args);
    let arguments = nperf.parse_parameter().unwrap();
    if let Some(x) = nperf.exec(arguments) {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

