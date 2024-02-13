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

###### **Options:**

* `-a`, `--ip <IP>` — IP address to measure against/listen on

  Default value: `0.0.0.0`
* `-p`, `--port <PORT>` — Port number to measure against/listen on. If port is defined with parallel mode, all client threads will measure against the same port

  Default value: `45001`
* `--parallel <PARALLEL>` — Start multiple client/server threads in parallel. The port number will be incremented automatically

  Default value: `1`
* `-r`, `--run-infinite` — Don't stop the node after the first measurement

  Default value: `false`

  Possible values: `true`, `false`

* `-l`, `--datagram-size <DATAGRAM_SIZE>` — Set length of single datagram (Without IP and UDP headers)

  Default value: `1472`
* `-t`, `--time <TIME>` — Time to run the test

  Default value: `10`
* `--with-gso` — Enable GSO on sending socket

  Default value: `false`

  Possible values: `true`, `false`

* `--with-gso-buffer <WITH_GSO_BUFFER>` — Set GSO buffer size which overwrites the MSS by default if GSO/GRO is enabled

  Default value: `65507`
* `--with-mss <WITH_MSS>` — Set transmit buffer size. Gets overwritten by GSO/GRO buffer size if GSO/GRO is enabled

  Default value: `1472`
* `--with-gro` — Enable GRO on receiving socket

  Default value: `false`

  Possible values: `true`, `false`

* `--with-ip-frag` — Disable fragmentation on sending socket

  Default value: `false`

  Possible values: `true`, `false`

* `--with-msg` — Use sendmsg/recvmsg method for sending data

  Default value: `false`

  Possible values: `true`, `false`

* `--with-mmsg` — Use sendmmsg/recvmmsg method for sending data

  Default value: `false`

  Possible values: `true`, `false`

* `--with-mmsg-amount <WITH_MMSG_AMOUNT>` — Amount of message packs of gso_buffers to send when using sendmmsg

  Default value: `1024`
* `--without-non-blocking` — Enable non-blocking socket

  Default value: `false`

  Possible values: `true`, `false`

* `--io-model <IO_MODEL>` — Select the IO model to use: busy-waiting, select, poll

  Default value: `select`
* `--json` — Enable json output of statistics

  Default value: `false`

  Possible values: `true`, `false`

* `--markdown-help`

  Possible values: `true`, `false`




<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

