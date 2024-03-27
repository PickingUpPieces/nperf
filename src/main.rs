use nperf::nPerf;

fn main() {
    let nperf = nPerf::new();

    let parameter = match nperf.parse_parameter() {
        Some(x) => x,
        None => { return },
    };

    nperf.exec(parameter);
}