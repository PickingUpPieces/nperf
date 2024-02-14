mod common;

// Test client sendmsg/sendmmsg with server sendmsg/sendmmsg in different combinations
#[test]
fn gro_no_gso() -> Result<(), Box<dyn std::error::Error>>{
    common::start_nperf_server(Some(vec!["--with-msg".to_string(), "--with-gro".to_string(), "--port".to_string(), "45001".to_string()]));

    let args = vec!["client", "--with-msg", "--port", "45001"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    Ok(())
}

#[test]
fn gso_no_gro() -> Result<(), Box<dyn std::error::Error>>{
    common::start_nperf_server(Some(vec!["--with-msg".to_string(), "--port".to_string(), "45002".to_string()]));

    let args = vec!["client", "--with-msg", "--with-gso", "--port", "45002"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    Ok(())
}

#[test]
fn gso_gro() -> Result<(), Box<dyn std::error::Error>>{
    common::start_nperf_server(Some(vec!["--with-msg".to_string(), "--with-gro".to_string(), "--port".to_string(), "45003".to_string()]));

    let args = vec!["client", "--with-msg", "--with-gso", "--port", "45003"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    Ok(())
}

#[test]
fn gso_no_gro_with_sendmmsg() -> Result<(), Box<dyn std::error::Error>>{
    common::start_nperf_server(Some(vec!["--with-msg".to_string(), "--port".to_string(), "45004".to_string()]));

    let args = vec!["client", "--with-mmsg", "--with-gso", "--port", "45004"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    Ok(())
}
