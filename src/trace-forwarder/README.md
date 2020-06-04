# Trace Forwarder

* [Overview](#overview)
* [Full details](#full-details)

## Overview

The Kata Containers trace forwarder, `kata-trace-forwarder`, is a component
running on the host system which is used to support tracing the agent process
which runs inside the virtual machine.

The trace forwarder, which must be started before the agent, listens over
VSOCK for trace data sent by the agent running inside the virtual machine. The
trace spans are exported to an OpenTelemetry collector (such as Jaeger) running by
default on the host.

## Full details

Run:

```
$ cargo run -- --help
```
