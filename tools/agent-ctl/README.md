# Agent Control tool

* [Overview](#overview)
* [Audience and environment](#audience-and-environment)
* [Full details](#full-details)
* [Code summary](#code-summary)
* [Running the tool](#running-the-tool)
    * [Prerequisites](#prerequisites)
    * [Connect to a real Kata Container](#connect-to-a-real-kata-container)
    * [Run the tool and the agent in the same environment](#run-the-tool-and-the-agent-in-the-same-environment)

## Overview

The Kata Containers agent control tool (`kata-agent-ctl`) is a low-level test
tool. It allows basic interaction with the Kata Containers agent,
`kata-agent`, that runs inside the virtual machine.

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
| Protocol buffers definition of the Kata Containers Agent API protocol | [`agent.proto`](../../src/agent/protocols/protos/agent.proto) | `CreateContainer` | API to create a Kata container. |
| Agent Control (client) API calls | [`src/client.rs`](src/client.rs) | `agent_cmd_container_create()` | Agent Control tool function that calls the `CreateContainer` API. |
| Agent (server) API implementations | [`rpc.rs`](../../src/agent/src/rpc.rs) | `create_container()` | Server function that implements the `CreateContainers` API. |

## Running the tool

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

### Connect to a real Kata Container

1. Start a Kata Container

1. Establish the VSOCK guest CID number for the virtual machine:

   Assuming you are running a single QEMU based Kata Container, you can look
   at the program arguments to find the (randomly-generated) `guest-cid=` option
   value:

   ```sh
   $ guest_cid=$(ps -ef | grep qemu-system-x86_64 | egrep -o "guest-cid=[^,][^,]*" | cut -d= -f2)
   ```

1. Run the tool to connect to the agent:

   ```sh
   $ cargo run -- -l debug connect --bundle-dir "${bundle_dir}" --server-address "vsock://${guest_cid}:1024" -c Check -c GetGuestDetails
   ```

   This examples makes two API calls:

   - It runs `Check` to see if the agent's RPC server is serving.
   - It then runs `GetGuestDetails` to establish some details of the
     environment the agent is running in.

### Run the tool and the agent in the same environment

> **Warnings:**
>
> - This method is **only** for testing and development!
> - Only continue if you are using a non-critical system
>   (such as a freshly installed VM environment).

1. Start the agent, specifying a local socket for it to communicate on:

   ```sh
   $ sudo KATA_AGENT_SERVER_ADDR=unix:///tmp/foo.socket target/x86_64-unknown-linux-musl/release/kata-agent
   ```

1. Run the tool in the same environment:

   ```sh
   $ cargo run -- -l debug connect --server-address "unix://@/tmp/foo.socket" --bundle-dir "$bundle_dir" -c Check -c GetGuestDetails
   ```

   > **Note:**
   >
   > The `@` in the server address is required - it denotes an abstract
   > socket which the agent requires (see `unix(7)`).
