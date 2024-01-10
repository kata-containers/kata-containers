# runtime-rs

## Wath's runtime-rs

`runtime-rs` is a new component introduced in Kata Containers 3.0, it is a Rust version of runtime(shim). It like [runtime](../runtime), but they have many difference:

- `runtime-rs` is written in Rust, and `runtime` is written in Go.
- `runtime` is the default shim in Kata Containers 3.0, `runtime-rs` is still under heavy development.
- `runtime-rs` has a completed different architecture than `runtime`, you can check at the [architecture overview](../../docs/design/architecture_3.0).

**Note**:

`runtime-rs` is still under heavy development, you should avoid using it in critical system.

## Architecture overview

Also, `runtime-rs` provides the following features:

- Turn key solution with builtin `Dragonball` Sandbox, all components in one process
- Async I/O to reduce resource consumption
- Extensible framework for multiple services, runtimes and hypervisors
- Lifecycle management for sandbox and container associated resources

See the [architecture overview](../../docs/design/architecture_3.0)
for details on the `runtime-rs` design.

`runtime-rs` is a runtime written in Rust, it is composed of several crates.

This picture shows the overview about the crates under this directory and the relation between crates.

![crates overview](docs/images/crate-overview.svg)

Not all the features have been implemented yet, for details please check the [roadmap](../../docs/design/architecture_3.0/README.md#roadmap).

## Crates

The `runtime-rs` directory contains some crates in the crates directory that compose the `containerd-shim-kata-v2`.

| Crate | Description |
|-|-|
| [`shim`](crates/shim)| containerd shimv2 implementation |
| [`service`](crates/service)| services for containers, includes task service |
| [`runtimes`](crates/runtimes)| container runtimes |
| [`resource`](crates/resource)| sandbox and container resources |
| [`hypervisor`](crates/hypervisor)| hypervisor that act as a sandbox |
| [`agent`](crates/agent)| library used to communicate with agent in the guest OS |
| [`persist`](crates/persist)| persist container state to disk |

### shim

`shim` is the entry point of the containerd shim process, it implements containerd shim's [binary protocol](https://github.com/containerd/containerd/tree/v1.6.8/runtime/v2#commands):

- start: start a new shim process
- delete: delete exist a shim process
- run: run ttRPC service in shim

containerd will launch a shim process and the shim process will serve as a ttRPC server to provide shim service through `TaskService` from `service` crate.

### service

The `runtime-rs` has an extensible framework, includes extension of services, runtimes, and hypervisors.

Currently, only containerd compatible `TaskService` is implemented.

`TaskService` has implemented the [containerd shim protocol](https://docs.rs/containerd-shim-protos/0.2.0/containerd_shim_protos/),
and interacts with runtimes through messages.

### runtimes

Runtime is a container runtime, the runtime handler handles messages from task services to manage containers.
Runtime handler and Runtime instance is used to deal with the operation for sandbox and container.

Currently, only `VirtContainer` has been implemented.

### resource

In `runtime-rs`, all networks/volumes/rootfs are abstracted as resources.

Resources are classified into two types:

- sandbox resources: network, share-fs
- container resources: rootfs, volume, cgroup

[Here](../../docs/design/architecture_3.0/README.md#resource-manager) is a detailed description of the resources.

### hypervisor

For `VirtContainer`, there will be more hypervisors to choose.

Currently, built-in `Dragonball` has been implemented. We have also added initial support for `cloud-hypervisor` with CI being added next.

### agent

`agent` is used to communicate with agent in the guest OS from the shim side. The only supported agent is `KataAgent`.

### persist

Persist defines traits and functions to help different components save state to disk and load state from disk.

### helper libraries

Some helper libraries are maintained in [the library directory](../libs) so that they can be shared with other rust components.

## Build and install

See the
[build from the source section of the rust runtime installation guide](../../docs/install/kata-containers-3.0-rust-runtime-installation-guide.md#build-from-source-installation).

## Configuration

`runtime-rs` has the same [configuration as `runtime`](../runtime/README.md#configuration) with some [limitations](#limitations).

## Logging

See the
[debugging section of the developer guide](../../docs/Developer-Guide.md#troubleshoot-kata-containers).

## Debugging

See the
[debugging section of the developer guide](../../docs/Developer-Guide.md#troubleshoot-kata-containers).

An [experimental alternative binary](crates/shim-ctl/README.md) is available that removes containerd dependencies and makes it easier to run the shim proper outside of the runtime's usual deployment environment (i.e. on a developer machine).

## Limitations

For Kata Containers limitations, see the
[limitations file](../../docs/Limitations.md)
for further details.

`runtime-rs` is under heavy developments, and doesn't support all features as the Golang version [`runtime`](../runtime), check the [roadmap](../../docs/design/architecture_3.0/README.md#roadmap) for details.
