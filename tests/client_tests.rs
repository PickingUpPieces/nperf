mod common;

// TODO: Test client send 
// TODO: Test client sendmsg
// TODO: Test client sendmmsg

#[test]
fn test_client_send() -> Result<(), Box<dyn std::error::Error>>{
    common::start_server();

    let args = vec!["client", "--with-msg"];
    if let Some(x) = nperf::nPerf::new().set_args(args).exec() {
        assert!(x.amount_datagrams > 10000);
    };

    Ok(())
}