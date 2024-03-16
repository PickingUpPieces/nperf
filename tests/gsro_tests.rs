mod common;

// Test client sendmsg/sendmmsg with server sendmsg/sendmmsg in different combinations
#[test]
fn gro_no_gso() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--with-gsro".to_string(), "--port=45001".to_string()]));

    let args = vec!["client",  "--port=45001"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn gso_no_gro() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--port=45002".to_string()]));

    let args = vec!["client", "--with-gsro", "--port=45002"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}

#[test]
fn gso_gro() -> Result<(), Box<dyn std::error::Error>>{
    let handle = common::start_nperf_server(Some(vec!["--with-gsro".to_string(), "--port=45003".to_string()]));

    let args = vec!["client", "--with-gsro", "--port=45003"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    handle.join().unwrap();
    Ok(())
}
