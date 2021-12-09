# Trace Forwarder

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
>   the agent will log an error (signalling that it cannot generate trace
>   spans), but continue to work as normal.
>
> - The trace forwarder requires a trace collector (such as Jaeger) to be
>   running before it is started. If a collector is not running, the trace
>   forwarder will exit with an error.

## Quick start

1. Start the OpenTelemetry collector (such as Jaeger).
1. [Start the trace forwarder](#run).
1. Ensure agent tracing is enabled in the Kata configuration file.
1. Create a Kata container as usual.

## Run

The way the trace forwarder is run depends on the configured hypervisor.

### Determine configured hypervisor

To identify which hypervisor Kata is configured to use, either look in the
configuration file, or run:

```bash
$ kata-runtime env --json|jq '.Hypervisor.Path'
```

### QEMU

Since QEMU supports VSOCK sockets in the standard way, if you are using QEMU
simply run the trace forwarder using the default options:

#### Run the forwarder

```bash
$ cargo run
```

You can now proceed to create a Kata container as normal.

### Cloud Hypervisor and Firecracker

Cloud Hypervisor and Firecracker both use "hybrid VSOCK" which uses a local
UNIX socket rather than the host kernel to handle communication with the
guest. As such, you need to specify the path to the UNIX socket.

Since the trace forwarder needs to be run before the VM (sandbox) is started
and since the socket path is sandbox-specific, you need to run the `env`
command to determine the "template path". This path includes a `{ID}` tag that
represents the real sandbox ID or name.

### Examples

#### Configured hypervisor is Cloud Hypervisor

```bash
$ socket_path_template=$(sudo kata-runtime env --json | jq '.Hypervisor.SocketPath')
$ echo "$socket_path_template"
"/run/vc/vm/{ID}/clh.sock"
```

#### Configured hypervisor is Firecracker

```bash
$ socket_path_template=$(sudo kata-runtime env --json | jq '.Hypervisor.SocketPath')
$ echo "$socket_path_template"
"/run/vc/firecracker/{ID}/root/kata.hvsock"
```

> **Note:**
>
> Do not rely on the paths shown above: you should run the command yourself
> as these paths _may_ change.

Once you have determined the template path, build and install the forwarder,
create the sandbox directory and then run the trace forwarder.

#### Build and install

If you are using the [QEMU hypervisor](#qemu), this step is not necessary.

If you are using Cloud Hypervisor of Firecracker, using the tool is simpler if
it has been installed.

##### Build

```bash
$ make
```

##### Install

```bash
$ cargo install --path .
$ sudo install -o root -g root -m 0755 ~/.cargo/bin/kata-trace-forwarder /usr/local/bin
```

#### Create sandbox directory

You will need to change the `sandbox_id` variable below to match the name of
the container (sandbox) you plan to create _after_ starting the trace
forwarder.

```bash
$ sandbox_id="foo"
$ socket_path=$(echo "$socket_path_template" | sed "s/{ID}/${sandbox_id}/g" | tr -d '"')
$ sudo mkdir -p $(dirname "$socket_path")
```

> **Note:** The `socket_path_template` variable was set in the
> [Cloud Hypervisor and Firecracker](#cloud-hypervisor-and-firecracker) section.

#### Run the forwarder specifying socket path

```bash
$ sudo kata-trace-forwarder --socket-path "$socket_path"
```

You can now proceed as normal to create the "foo" Kata container.

> **Note:**
>
> Since the trace forwarder needs to create the socket in the sandbox
> directory, and since that directory is owned by the `root` user, the trace
> forwarder must also be run as `root`. This requirement is unique to
> hypervisors that use hybrid VSOCK: QEMU does not require special privileges
> to run the trace forwarder. To reduce the impact of this, once the forwarder
> is running it drops privileges to run as user `nobody`.

## Full details

For further information on how to run the trace forwarder, run:

```bash
$ cargo run -- --help
```

[agent-tracing]: /docs/tracing.md
[jaeger-tracing]: https://www.jaegertracing.io
[opentelemetry]: https://opentelemetry.io
[vsock]: https://wiki.qemu.org/Features/VirtioVsock
