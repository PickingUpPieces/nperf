# Command-Line Help for `nperf`

This document contains the help content for the `nperf` command-line program.

**Command Overview:**

* [`nperf`↴](#nperf)

## `nperf`

A network performance measurement tool

**Usage:** `nperf [OPTIONS] [MODE]`

###### **Arguments:**

* `<MODE>` — Mode of operation: client or server

  Default value: `server`

  Possible values: `server`, `client`


###### **Options:**

* `-a`, `--ip <IP>` — IP address to measure against/listen on

  Default value: `0.0.0.0`
* `-p`, `--port <PORT>` — Port number to measure against, server listen on

  Default value: `45001`
* `-c`, `--client-port <CLIENT_PORT>` — Port number clients send from

  Default value: `46001`
* `--parallel <PARALLEL>` — Start multiple client/server threads in parallel. The port number will be incremented automatically

  Default value: `1`
* `-r`, `--run-infinite` — Don't stop the node after the first measurement

  Default value: `false`

  Possible values: `true`, `false`

* `-i`, `--interval <INTERVAL>` — Interval printouts of the statistic in seconds (0 to disable)

  Default value: `0`
* `-l`, `--datagram-size <DATAGRAM_SIZE>` — Set length of single datagram (Without IP and UDP headers)

  Default value: `1472`
* `-t`, `--time <TIME>` — Amount of seconds to run the test for

  Default value: `10`
* `--with-core-affinity` — Pin each thread to an individual core. The server threads start from the last core, the client threads from the second core. This way each server/client pair should operate on the same NUMA core

  Default value: `false`

  Possible values: `true`, `false`

* `--with-numa-affinity` — Pin client/server threads to different NUMA nodes

  Default value: `false`

  Possible values: `true`, `false`

* `--with-gsro` — Enable GSO/GRO on socket

  Default value: `false`

  Possible values: `true`, `false`

* `--with-gso-buffer <WITH_GSO_BUFFER>` — Set GSO buffer size which overwrites the MSS by default if GSO/GRO is enabled

  Default value: `64768`
* `--with-mss <WITH_MSS>` — Set transmit buffer size. Gets overwritten by GSO/GRO buffer size if GSO/GRO is enabled

  Default value: `1472`
* `--with-ip-frag` — Disable fragmentation on sending socket

  Default value: `false`

  Possible values: `true`, `false`

* `--without-non-blocking` — Disable non-blocking socket

  Default value: `false`

  Possible values: `true`, `false`

* `--with-socket-buffer` — Enable setting udp socket buffer size

  Default value: `false`

  Possible values: `true`, `false`

* `--exchange-function <EXCHANGE_FUNCTION>` — Exchange function to use: normal (send/recv), sendmsg/recvmsg, sendmmsg/recvmmsg

  Default value: `msg`

  Possible values: `normal`, `msg`, `mmsg`

* `--with-mmsg-amount <WITH_MMSG_AMOUNT>` — Amount of message packs of gso_buffers to send when using sendmmsg

  Default value: `1`
* `--io-model <IO_MODEL>` — Select the IO model to use: busy-waiting, select, poll

  Default value: `poll`

  Possible values: `poll`, `busy-waiting`, `select`, `io-uring`

* `--output-format <OUTPUT_FORMAT>` — Define the type the output

  Default value: `text`

  Possible values: `text`, `json`, `file`

* `--output-file-path <OUTPUT_FILE_PATH>` — Define the path in which the results file should be saved. Make sure the path exists and the application has the rights to write in it

  Default value: `nperf-output.csv`
* `--label-test <LABEL_TEST>` — Test label which appears in the output file, if multiple tests are run in parallel

  Default value: `nperf-test`
* `--label-run <LABEL_RUN>` — Run label which appears in the output file, to differentiate between multiple different runs which are executed within a single test

  Default value: `run-nperf`
* `--multiplex-port <MULTIPLEX_PORT>` — Use different port number for each client thread, share a single port or shard a single port with reuseport

  Default value: `individual`

  Possible values: `individual`, `sharing`, `sharding`

* `--multiplex-port-server <MULTIPLEX_PORT_SERVER>` — Same as for multiplex_port, but for the server

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

* `--uring-burst-size <URING_BURST_SIZE>` — io_uring: Amount of recvmsg/sendmsg requests are submitted/completed in one go

  Default value: `64`
* `--uring-ring-size <URING_RING_SIZE>` — io_uring: Size of the ring buffer

  Default value: `256`
* `--uring-sq-mode <URING_SQ_MODE>` — io_uring: How the SQ is filled

  Default value: `topup`

  Possible values: `topup`, `topup-no-wait`, `syscall`

* `--uring-task-work <URING_TASK_WORK>` — io_uring: Set the operation mode of task_work

  Default value: `default`

  Possible values: `default`, `coop`, `defer`, `coop-defer`

* `--markdown-help` — Show help in markdown format

  Possible values: `true`, `false`




<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

