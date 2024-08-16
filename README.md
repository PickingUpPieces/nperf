# nPerf
nPerf is a network performance measurement tool solely for measuring UDP throughput. Several kernel features, such as GSO, GRO, or io_uring, can be compared along with several different features.

## Dependencies
- libhwloc-dev
- libudev-dev

# Command-Line Help for `nperf`
**Command Overview:**

* [`nperf`↴](#nperf)

## `nperf`

A network performance measurement tool

**Usage:** `nperf [OPTIONS] [MODE]`

###### **Arguments:**

* `<MODE>` — Mode of operation: sender or receiver

  Default value: `receiver`

  Possible values: `receiver`, `sender`


###### **Options:**

* `-a`, `--ip <IP>` — IP address to measure against/listen on

  Default value: `0.0.0.0`
* `-p`, `--port <PORT>` — Port number for sender to measure against and receiver to listen on

  Default value: `45001`
* `-s`, `--sender-port <SENDER_PORT>` — Port number the sender uses to send packets

  Default value: `46001`
* `--parallel <PARALLEL>` — Start multiple sender/receiver threads in parallel. The port number is incremented automatically for every thread

  Default value: `1`
* `-r`, `--run-infinite` — Do not finish the execution after the first measurement

  Default value: `false`

  Possible values: `true`, `false`

* `-i`, `--interval <INTERVAL>` — Interval printouts of the statistic in seconds (0 to disable). WARNING: Interval statistics are printed at the end of the test, not at the interval!

  Default value: `0`
* `-l`, `--datagram-size <DATAGRAM_SIZE>` — Length of single datagram (Without IP and UDP headers)

  Default value: `1472`
* `-t`, `--time <TIME>` — Amount of seconds to run the test for

  Default value: `10`
* `--with-core-affinity` — Pin each thread to an individual core. The receiver threads start from the last core downwards, while the sender threads are pinned from the first core upwards

  Default value: `false`

  Possible values: `true`, `false`

* `--with-numa-affinity` — Pin sender/receiver threads alternating to the available NUMA nodes

  Default value: `false`

  Possible values: `true`, `false`

* `--with-gsro` — Enable GSO or GRO for the sender/receiver. The gso_size is set with --with-gso-buffer

  Default value: `false`

  Possible values: `true`, `false`

* `--bandwidth <BANDWIDTH>` — Use kernel pacing to ensure a send bandwidth in total (not per thread) in Mbit/s (0 for disabled)

  Default value: `0`
* `--with-gso-buffer <WITH_GSO_BUFFER>` — Set GSO buffer size which overwrites the MSS by default if GSO/GRO is enabled

  Default value: `64768`
* `--with-mss <WITH_MSS>` — Set the transmit buffer size. Multiple smaller datagrams can be send with one packet of MSS size. The MSS is the size of the packets sent out by nPerf. Gets overwritten by GSO/GRO buffer size if GSO/GRO is enabled

  Default value: `1472`
* `--with-ip-frag` — Enable IP fragmentation on sending socket

  Default value: `false`

  Possible values: `true`, `false`

* `--without-non-blocking` — Disable non-blocking socket

  Default value: `false`

  Possible values: `true`, `false`

* `--with-socket-buffer <WITH_SOCKET_BUFFER>` — Setting socket buffer size (in multiple of default size 212992 Byte)

  Default value: `1`
* `--exchange-function <EXCHANGE_FUNCTION>` — Exchange function to use: normal (send/recv), msg (sendmsg/recvmsg), mmsg (sendmmsg/recvmmsg)

  Default value: `msg`

  Possible values: `normal`, `msg`, `mmsg`

* `--with-mmsg-amount <WITH_MMSG_AMOUNT>` — Size of msgvec when using sendmmsg/recvmmsg

  Default value: `1`
* `--io-model <IO_MODEL>` — Select the IO model to use

  Default value: `select`

  Possible values: `select`, `poll`, `busy-waiting`, `io-uring`

* `--output-format <OUTPUT_FORMAT>` — Define the type the output

  Default value: `text`

  Possible values: `text`, `json`, `file`

* `--output-file-path <OUTPUT_FILE_PATH>` — Define the path in which the results file should be saved. Make sure the path exists and the application has the rights to write in it

  Default value: `nperf-output.csv`
* `--label-test <LABEL_TEST>` — Test label which appears in the output file, if multiple tests are run in parallel. Useful for benchmark automation

  Default value: `nperf-test`
* `--label-run <LABEL_RUN>` — Run label which appears in the output file, to differentiate between multiple different runs which are executed within a single test. Useful for benchmark automation

  Default value: `run-nperf`
* `--repetition-id <REPETITION_ID>` — Repetition label which appears in the output file, to differentiate between multiple different repetitions which are executed for a single run. Useful for benchmark automation

  Default value: `1`
* `--multiplex-port <MULTIPLEX_PORT>` — Configure if all threads should use different ports, share a port or use port sharding

  Default value: `individual`

  Possible values: `individual`, `sharing`, `sharding`

* `--multiplex-port-receiver <MULTIPLEX_PORT_RECEIVER>` — Same as for multiplex_port, but for the receiver

  Default value: `individual`

  Possible values: `individual`, `sharing`, `sharding`

* `--simulate-connection <SIMULATE_CONNECTION>` — CURRENTLY IGNORED. Simulate a single QUIC connection or one QUIC connection per thread

  Default value: `multiple`

  Possible values: `single`, `multiple`

* `--uring-mode <URING_MODE>` — io_uring: Which mode to use

  Default value: `normal`

  Possible values: `normal`, `zerocopy`, `provided-buffer`, `multishot`

* `--uring-sqpoll` — io_uring: Use a SQ_POLL thread per executing thread, pinned to CPU 0

  Default value: `false`

  Possible values: `true`, `false`

* `--uring-sqpoll-shared` — io_uring: Share the SQ_POLL thread between all executing threads

  Default value: `false`

  Possible values: `true`, `false`

* `--uring-burst-size <URING_BURST_SIZE>` — io_uring: Amount of recvmsg/sendmsg operations are submitted/completed in one go

  Default value: `64`
* `--uring-ring-size <URING_RING_SIZE>` — io_uring: Size of the SQ ring buffer

  Default value: `256`
* `--uring-sq-mode <URING_SQ_MODE>` — io_uring: Event loop strategy

  Default value: `topup`

  Possible values: `topup`, `topup-no-wait`, `syscall`

* `--uring-task-work <URING_TASK_WORK>` — io_uring: Set the operation mode of task_work

  Default value: `default`

  Possible values: `default`, `coop`, `defer`, `coop-defer`

* `--uring-record-utilization` — io_uring: Record utilization of SQ, CQ and inflight counter

  Default value: `false`

  Possible values: `true`, `false`

* `--markdown-help` — Show help in markdown format

  Possible values: `true`, `false`




<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

