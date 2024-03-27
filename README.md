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

* `-l`, `--datagram-size <DATAGRAM_SIZE>` — Set length of single datagram (Without IP and UDP headers)

  Default value: `1472`
* `-t`, `--time <TIME>` — Time to run the test

  Default value: `10`
* `--with-gsro` — Enable GSO/GRO on socket

  Default value: `false`

  Possible values: `true`, `false`

* `--with-gso-buffer <WITH_GSO_BUFFER>` — Set GSO buffer size which overwrites the MSS by default if GSO/GRO is enabled

  Default value: `65507`
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

  Possible values: `poll`, `busy-waiting`, `select`

* `--output-format <OUTPUT_FORMAT>` — Define the data structure type the output

  Default value: `text`

  Possible values: `text`, `json`

* `--multiplex-port <MULTIPLEX_PORT>` — Use different port number for each client thread, share a single port or shard a single port with reuseport

  Default value: `individual`

  Possible values: `individual`, `sharing`, `sharding`

* `--multiplex-port-server <MULTIPLEX_PORT_SERVER>` — Same as for multiplex_port, but for the server

  Default value: `individual`

  Possible values: `individual`, `sharing`, `sharding`

* `--simulate-connection <SIMULATE_CONNECTION>` — CURRENTLY IGNORED. Simulate a single QUIC connection or one QUIC connection per thread

  Default value: `multiple`

  Possible values: `single`, `multiple`

* `--markdown-help` — Show help in markdown format

  Possible values: `true`, `false`




<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

