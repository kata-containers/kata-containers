# Agent Control tool

## Overview

The Kata Containers agent control tool (`kata-agent-ctl`) is a low-level test
tool. It allows basic interaction with the Kata Containers agent,
`kata-agent`, that runs inside the virtual machine (VM).

Unlike the Kata Runtime, which only ever makes sequences of correctly ordered
and valid agent API calls, this tool allows users to make arbitrary agent API
calls and to control their parameters.

## Audience and environment

> **Warning:**
>
> This tool is for *advanced* users familiar with the low-level agent API calls.
> Further, it is designed to be run on test and development systems **only**: since
> the tool can make arbitrary API calls, it is possible to easily confuse
> irrevocably other parts of the system or even kill a running container or
> sandbox.

## Full details

For a usage statement, run:

```sh
$ cargo run -- --help
```

To see some examples, run:

```sh
$ cargo run -- examples
```

## Code summary

The table below summarises where to look to learn more about both this tool,
the agent protocol and the client and server implementations.

| Description | File | Example RPC or function | Example summary |
|-|-|-|-|
| Protocol buffers definition of the Kata Containers Agent API protocol | [`agent.proto`](../../libs/protocols/protos/agent.proto) | `CreateContainer` | API to create a Kata container. |
| Agent Control (client) API calls | [`src/client.rs`](src/client.rs) | `agent_cmd_container_create()` | Agent Control tool function that calls the `CreateContainer` API. |
| Agent (server) API implementations | [`rpc.rs`](../../agent/src/rpc.rs) | `create_container()` | Server function that implements the `CreateContainers` API. |

## Run the tool

### Prerequisites

It is necessary to create an OCI bundle to use the tool. The simplest method
is:

```sh
$ bundle_dir="bundle"
$ rootfs_dir="$bundle_dir/rootfs"
$ image="busybox"
$ mkdir -p "$rootfs_dir" && (cd "$bundle_dir" && runc spec)
$ sudo docker export $(sudo docker create "$image") | tar -C "$rootfs_dir" -xvf -
```

### Specify API commands to run

The tool allows one or more API commands to be specified using the `-c` or
`--cmd` command-line options. At their simplest, these are just the name of
the API commands, which will make the API command using default values
(generally blank or empty) where possible. However, some API calls require
some basic value to be specified such as a sandbox ID or container ID. For
these calls, the tool will generate a value by default unless told not to.

If the user wishes to, they may specify these values as part of the command
using `name=value` syntax.

In addition to this, it is possible to specify either a complete or partial
set of values for the API call using JSON syntax, either directly on the
command-line or via a file URI.

The table below summarises the possible ways of specifying an API call to
make.

| CLI values | API Query |
|-|-|
| `-c 'SomeAPIName' -n` | Calls the API using the default values for all request options |
| `-c 'SomeAPIName'` | Calls the API specifying some values automatically if possible |
| `-c 'SomeAPIName foo=bar baz="hello world" x=3 y="a cat"'` | Calls the API specifying various values in name/value form |
| `-c 'SomeAPIName json://{}' -n` | Calls the API specifying empty values via an empty JSON document |
| `-c 'SomeAPIName json://{"foo": true, "bar": "hello world"}' -n` | Calls the API specifying _some_ values in JSON syntax |
| `-c 'SomeAPIName file:///foo.json' -n` | Calls the API passing the JSON values from the specified file |

#### JSON Example

An example showing how to specify the messages fields for an API call
(`GetGuestDetails`):

```sh
$ cargo run -- -l debug connect --server-address "unix://@/tmp/foo.socket" --bundle-dir "$bundle_dir" -c Check -c 'GetGuestDetails json://{"mem_block_size": true, "mem_hotplug_probe": true}'
```

> **Notes:**
>
> - For details of the names of the APIs to call and the available fields
>   in each API, see the [Code Summary](#code-summary) section.
> - For further examples, see the [Examples](#examples) section.

### Connect to a real Kata Container

The method used to connect to Kata Containers agent depends on the configured
hypervisor. Although by default the Kata Containers agent listens for API calls on a
VSOCK socket, the way that socket is exposed to the host depends on the
hypervisor.

#### QEMU

Since QEMU supports VSOCK sockets in the standard way, it is only necessary to
establish the VSOCK guest CID value to connect to the agent.

1. Start a Kata Container

1. Establish the VSOCK guest CID number for the VM:

   ```sh
   $ guest_cid=$(sudo ss -H --vsock | awk '{print $6}' | cut -d: -f1)
   ```

1. Run the tool to connect to the agent:

   ```sh
   # Default VSOCK port the agent listens on
   $ agent_vsock_port=1024

   $ cargo run -- -l debug connect --bundle-dir "${bundle_dir}" --server-address "vsock://${guest_cid}:${agent_vsock_port}" -c Check -c GetGuestDetails
   ```

   This examples makes two API calls:

   - It runs `Check` to see if the agent's RPC server is serving.
   - It then runs `GetGuestDetails` to establish some details of the
     environment the agent is running in.

#### Cloud Hypervisor and Firecracker

Cloud Hypervisor and Firecracker both use "hybrid VSOCK" which uses a local
UNIX socket rather than the host kernel to handle communication with the
guest. As such, you need to specify the path to the UNIX socket.

Since the UNIX socket path is sandbox-specific, you need to run the
`kata-runtime env` command to determine the socket's "template path". This
path includes a `{ID}` tag that represents the real sandbox ID or name.

Further, since the socket path is below the sandbox directory and since that
directory is `root` owned, it is necessary to run the tool as `root` when
using a Hybrid VSOCKS hypervisor.

##### Determine socket path template value

###### Configured hypervisor is Cloud Hypervisor

```bash
$ socket_path_template=$(sudo kata-runtime env --json | jq '.Hypervisor.SocketPath')
$ echo "$socket_path_template"
"/run/vc/vm/{ID}/clh.sock"
```

###### Configured hypervisor is Firecracker

```bash
$ socket_path_template=$(sudo kata-runtime env --json | jq '.Hypervisor.SocketPath')
$ echo "$socket_path_template"
"/run/vc/firecracker/{ID}/root/kata.hvsock"
```

> **Note:**
>
> Do not rely on the paths shown above: you should run the command yourself
> as these paths _may_ change.

Once you have determined the template path, build and install the tool to make
it easier to run as the `root` user.

##### Build and install

```bash
# Install for user
$ make install

# Install centrally
$ sudo install -o root -g root -m 0755 ~/.cargo/bin/kata-agent-ctl /usr/local/bin
```

1. Start a Kata Container

   Create a container called `foo`.

1. Run the tool

   ```bash
   # Name of container
   $ sandbox_id="foo"

   # Create actual socket path
   $ socket_path=$(echo "$socket_path_template" | sed "s/{ID}/${sandbox_id}/g" | tr -d '"')

   $ sudo kata-agent-ctl -l debug connect --bundle-dir "${bundle_dir}" --server-address "unix://${socket_path}" --hybrid-vsock -c Check -c GetGuestDetails
   ```

   > **Note:** The `socket_path_template` variable was set in the
   > [Determine socket path template value](#determine-socket-path-template-value) section.

### Run the tool and the agent in the same environment

> **Warnings:**
>
> - These methods are **only** for testing and development!
> - Only continue if you are using a non-critical system
>   (such as a freshly installed VM environment).

#### Use a Unix abstract domain socket

1. Start the agent, specifying a local socket for it to communicate on:

   ```sh
   $ sudo KATA_AGENT_SERVER_ADDR=unix://@/tmp/foo.socket target/x86_64-unknown-linux-musl/release/kata-agent
   ```

   > **Note:** This example assumes an Intel x86-64 system.

1. Run the tool in the same environment:

   ```sh
   $ cargo run -- -l debug connect --server-address "unix://@/tmp/foo.socket" --bundle-dir "$bundle_dir" -c Check -c GetGuestDetails
   ```

   > **Note:**
   >
   > The `@` in the server address is required - it denotes an abstract
   > socket which the agent requires (see `unix(7)`).

#### Use a VSOCK loopback socket

VSOCK supports a special CID value of `1` (known symbolically as
`VMADDR_CID_LOCAL`) which assumes that the VM is actually
the local environment. This is effectively a `localhost` or loopback
interface which does not require an actual VM to be
running.

1. Start the agent, specifying the local VSOCK socket for it to communicate on:

   ```sh
   $ vsock_loopback_cid=1
   $ agent_vsock_port=1024

   $ sudo KATA_AGENT_SERVER_ADDR="vsock://${vsock_loopback_cid}:${agent_vsock_port}" target/x86_64-unknown-linux-musl/release/kata-agent
   ```

   > **Note:** This example assumes an Intel x86-64 system.

1. Run the tool in the same environment:

   ```sh
   $ vsock_loopback_cid=1
   $ agent_vsock_port=1024

   $ cargo run -- -l debug connect --server-address "vsock://${vsock_loopback_cid}:${agent_vsock_port}" --bundle-dir "$bundle_dir" -c Check -c GetGuestDetails
   ```

## Examples

### QEMU examples

The following examples assume you have:

- Configure Kata Containers to use QEMU.
- Created a container using a command such as this:
  ```bash
  $ container_id="foo"
  $ sudo ctr --debug run --runtime "io.containerd.kata.v2" --rm -t "quay.io/prometheus/busybox:latest" "$container_id" sh
  ```

#### Check the agent is running with QEMU

```bash
$ cargo run -- -l trace connect --no-auto-values --bundle-dir "${bundle_dir}" --server-address "vsock://${guest_cid}:${agent_vsock_port}" -c Check
```

#### Show guest environment details with QEMU

```bash
$ cargo run -- -l trace connect --no-auto-values --bundle-dir "${bundle_dir}" --server-address "vsock://${guest_cid}:${agent_vsock_port}" -c GetGuestDetails
```

#### Show sandbox metrics with QEMU

```bash
$ cargo run -- -l trace connect --no-auto-values --bundle-dir "${bundle_dir}" --server-address "vsock://${guest_cid}:${agent_vsock_port}" -c GetMetrics
```

#### Show container statistics with QEMU

```bash
$ cargo run -- -l trace connect --no-auto-values --bundle-dir "${bundle_dir}" --server-address "vsock://${guest_cid}:${agent_vsock_port}" -c "StatsContainer json://{\"container_id\": \"${container_id}\"}"
```

#### Pause a running container with QEMU

```bash
$ cargo run -- -l trace connect --no-auto-values --bundle-dir "${bundle_dir}" --server-address "vsock://${guest_cid}:${agent_vsock_port}" -c "PauseContainer json://{\"container_id\": \"$container_id\"}"
```

#### Resume a paused container with QEMU

```bash
$ cargo run -- -l trace connect --no-auto-values --bundle-dir "${bundle_dir}" --server-address "vsock://${guest_cid}:${agent_vsock_port}" -c "ResumeContainer json://{\"container_id\": \"$container_id\"}"
```

#### Destroy a running container with QEMU

```bash
$ cargo run -- -l trace connect --no-auto-values --bundle-dir "${bundle_dir}" --server-address "vsock://${guest_cid}:${agent_vsock_port}" -c "RemoveContainer json://{\"container_id\": \"$container_id\"}"
```

> **Note:**
>
> Only run this on a test or development system!

#### Destroy a running sandbox with QEMU

```bash
$ cargo run -- -l trace connect --no-auto-values --bundle-dir "${bundle_dir}" --server-address "vsock://${guest_cid}:${agent_vsock_port}" -c 'DestroySandbox'
```

> **Notes:**
>
> - Only run this on a test or development system!
> - This will destroy the sandbox and all resources associated with it
>   (including all containers and the VM that hosts the agent).
> - You cannot create a sandbox this way since the Kata Containers
>   runtime and agent will already have created one inside the QEMU
>   VM.
>
>   See [Create a sandbox manually](#create-a-sandbox-manually).

### Local examples

> **Note:**
>
> These examples assume you have
> [already started the agent manually](#use-a-unix-abstract-domain-socket).

#### Create a sandbox manually

```bash
$ sandbox_id="foo"

# Critical to clear up from any previous run
$ sudo umount /run/sandbox-ns/*

$ server_addr='unix://@/tmp/foo.socket'

$ cargo run -- -l trace connect --no-auto-values --bundle-dir "${bundle_dir}" --server-address "${server_addr}" -c "CreateSandbox json://{\"sandbox_id\": \"$sandbox_id\"}"
```

> **Note:**
>
> Although it should be possible to create a container inside the
> sandbox, this is difficult since the JSON `CreateContainer` request
> must include an OCI configuration file specification in (quoted)
> JSON format.

#### Delete a sandbox manually

```bash
$ sandbox_id="foo"

$ server_addr='unix://@/tmp/foo.socket'

$ cargo run -- -l trace connect --no-auto-values --bundle-dir "${bundle_dir}" --server-address "${server_addr}" -c DestroySandbox
```

> **Note:**
>
> No parameters are required for the `DestroySandbox` API call
> (since only a single sandbox can be created).
