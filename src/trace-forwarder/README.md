* [Overview](#overview)
* [Example](#example)
* [Full details](#full-details)

## Overview

The Kata Containers trace forwarder, `kata-trace-forwarder`, is a component
running on the host system which is used to support
[tracing the agent process][agent-tracing], which runs inside the Kata
Containers virtual machine (VM).

The trace forwarder, which must be started before the container, listens over
[`VSOCK`][vsock] for trace data sent by the agent running inside the VM. The
trace spans are exported to an [OpenTelemetry][opentelemetry] collector (such
as [Jaeger][jaeger-tracing]) running by default on the host.

> **Notes:**
>
> - If agent tracing is enabled but the forwarder is not running,
>   the agent will exit (panic) with an error message. Since the agent is not
>   directly visible by the user, it is recommended to enable debug to capture
>   this message in the logs.
>
> - The forwarder requires a trace collector (such as Jaeger) to be running
>   before it is started. If a collector is not running, the trace forwarder
>   will exit with an error.

## Example

Run with maximum verbosity:

```
$ cargo run -- -l trace
```

## Full details

Run:

```
$ cargo run -- --help
```

[agent-tracing]: https://github.com/kata-containers/kata-containers/blob/main/docs/tracing.md
[jaeger-tracing]: https://www.jaegertracing.io
[opentelemetry]: https://opentelemetry.io
[vsock]: https://www.qemu.org/Features/VirtioVsock
