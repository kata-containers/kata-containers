# Overview

A [Kata Container](https://github.com/kata-containers) utilizes a Virtual Machine (VM) to enhance security and
isolation of container workloads. As a result, the system has a number of differences
and limitations when compared with the default [Docker*](https://www.docker.com/) runtime,
[`runc`](https://github.com/opencontainers/runc).

Some of these limitations have potential solutions, whereas others exist
due to fundamental architectural differences generally related to the
use of VMs.

The [Kata Container runtime](../src/runtime)
launches each container within its own hardware isolated VM, and each VM has
its own kernel. Due to this higher degree of isolation, certain container
capabilities cannot be supported or are implicitly enabled through the VM.

# Definition of a limitation

The [Open Container Initiative](https://www.opencontainers.org/)
[Runtime Specification](https://github.com/opencontainers/runtime-spec) ("OCI spec")
defines the minimum specifications a runtime must support to interoperate with
container managers such as Docker. If a runtime does not support some aspect
of the OCI spec, it is by definition a limitation.

However, the OCI runtime reference implementation (`runc`) does not perfectly
align with the OCI spec itself.

Further, since the default OCI runtime used by Docker is `runc`, Docker
expects runtimes to behave as `runc` does. This implies that another form of
limitation arises if the behavior of a runtime implementation does not align
with that of `runc`. Having two standards complicates the challenge of
supporting a Docker environment since a runtime must support the official OCI
spec and the non-standard extensions provided by `runc`.

# Scope

Each known limitation is captured in a separate GitHub issue that contains
detailed information about the issue. These issues are tagged with the
`limitation` label. This document is a curated summary of important known
limitations and provides links to the relevant GitHub issues.

The following link shows the latest list of limitations:

- https://github.com/pulls?utf8=%E2%9C%93&q=is%3Aopen+label%3Alimitation+org%3Akata-containers

# Contributing

If you would like to work on resolving a limitation, please refer to the
[contributors guide](https://github.com/kata-containers/community/blob/main/CONTRIBUTING.md).
If you wish to raise an issue for a new limitation, either
[raise an issue directly on the runtime](https://github.com/kata-containers/kata-containers/issues/new)
or see the
[project table of contents](https://github.com/kata-containers/kata-containers)
for advice on which repository to raise the issue against.

# Pending items

This section lists items that might be possible to fix.

## OCI CLI commands

### Docker and Podman support
Currently Kata Containers does not support Podman.

See issue https://github.com/kata-containers/kata-containers/issues/722 for more information.

Docker supports Kata Containers since 22.06:

```bash
$ sudo docker run --runtime io.containerd.kata.v2
```

Kata Containers works perfectly with containerd, we recommend to use
containerd's Docker-style command line tool [`nerdctl`](https://github.com/containerd/nerdctl).

## Runtime commands

### checkpoint and restore

The runtime does not provide `checkpoint` and `restore` commands. There
are discussions about using VM save and restore to give us a
[`criu`](https://github.com/checkpoint-restore/criu)-like functionality,
which might provide a solution.

Note that the OCI standard does not specify `checkpoint` and `restore`
commands.

See issue https://github.com/kata-containers/runtime/issues/184 for more information.

### events command

The runtime does not fully implement the `events` command. `OOM` notifications and `Intel RDT` stats are not fully supported.

Note that the OCI standard does not specify an `events` command.

See issue https://github.com/kata-containers/runtime/issues/308 and https://github.com/kata-containers/runtime/issues/309 for more information.

### update command

Currently, only block I/O weight is not supported.
All other configurations are supported and are working properly.

## Networking

### Host network

Host network (`nerdctl/docker run --net=host`or [Kubernetes `HostNetwork`](https://kubernetes.io/docs/reference/kubernetes-api/workload-resources/pod-v1/#hosts-namespaces)) is not supported.
It is not possible to directly access the host networking configuration
from within the VM.

The `--net=host` option can still be used with `runc` containers and
inter-mixed with running Kata Containers, thus enabling use of `--net=host`
when necessary.

It should be noted, currently passing the `--net=host` option into a
Kata Container may result in the Kata Container networking setup
modifying, re-configuring and therefore possibly breaking the host
networking setup. Do not use `--net=host` with Kata Containers.

### Support for joining an existing VM network

Docker supports the ability for containers to join another containers
namespace with the `docker run --net=containers` syntax. This allows
multiple containers to share a common network namespace and the network
interfaces placed in the network namespace. Kata Containers does not
support network namespace sharing. If a Kata Container is setup to
share the network namespace of a `runc` container, the runtime
effectively takes over all the network interfaces assigned to the
namespace and binds them to the VM. Consequently, the `runc` container loses
its network connectivity.

### docker run --link

The runtime does not support the `docker run --link` command. This
command is now deprecated by docker and we have no intention of adding support.
Equivalent functionality can be achieved with the newer docker networking commands.

See more documentation at
[docs.docker.com](https://docs.docker.com/network/links/).

## Resource management

Due to the way VMs differ in their CPU and memory allocation, and sharing
across the host system, the implementation of an equivalent method for
these commands is potentially challenging.

See issue https://github.com/clearcontainers/runtime/issues/341 and [the constraints challenge](#the-constraints-challenge) for more information.

For CPUs resource management see
[CPU constraints(in runtime-go)](design/vcpu-handling-runtime-go.md).
[CPU constraints(in runtime-rs)](design/vcpu-handling-runtime-rs.md).

# Architectural limitations

This section lists items that might not be fixed due to fundamental
architectural differences between "soft containers" (i.e. traditional Linux*
containers) and those based on VMs.

## Storage limitations

### Kubernetes `volumeMounts.subPaths`

Kubernetes `volumeMount.subPath` is not supported by Kata Containers at the
moment.

See [this issue](https://github.com/kata-containers/runtime/issues/2812) for more details.
[Another issue](https://github.com/kata-containers/kata-containers/issues/1728) focuses on the case of `emptyDir`.

### Kubernetes [hostPath][k8s-hostpath] volumes

In Kata, Kubernetes hostPath volumes can mount host directories and
regular files into the guest VM via filesystem sharing, if it is enabled
through the `shared_fs` [configuration][runtime-config] flag.

By default:

 - Non-TEE environment: Filesystem sharing is used to mount host files.
 - TEE environment: Filesystem sharing is disabled. Instead, host files
   are copied into the guest VM when the container starts, and file
   changes are *not* synchronized between the host and the guest.

In some cases, the behavior of hostPath volumes in Kata is further
different compared to `runc` containers:

**Mounting host block devices**: When a hostPath volume is of type
[`BlockDevice`][k8s-blockdevice], Kata hotplugs the host block device
into the guest and exposes it directly to the container.

**Mounting guest devices**: When the source path of a hostPath volume is
under `/dev`, and the path either corresponds to a host device or is not
accessible by the Kata shim, the Kata agent bind mounts the source path
directly from the *guest* filesystem into the container.

[runtime-config]: /src/runtime/README.md#configuration
[k8s-hostpath]: https://kubernetes.io/docs/concepts/storage/volumes/#hostpath
[k8s-blockdevice]: https://kubernetes.io/docs/concepts/storage/volumes/#hostpath-volume-types

### Mounting `procfs` and `sysfs`

For security reasons, the following mounts are disallowed:

| Type              | Source    | Destination                      | Rationale      |
|-------------------|-----------|----------------------------------|----------------|
| `bind`            | `!= proc` | `/proc`                          | CVE-2019-16884 |
| `bind`            | `*`       | `/proc/*` (see exceptions below) | CVE-2019-16884 |
| `proc \|\| sysfs` | `*`       | not a directory (e.g. symlink)   | CVE-2019-19921 |

For bind mounts under /proc, these destinations are allowed:
	
 * `/proc/cpuinfo`
 * `/proc/diskstats`
 * `/proc/meminfo`
 * `/proc/stat`
 * `/proc/swaps`
 * `/proc/uptime`
 * `/proc/loadavg`
 * `/proc/net/dev`

## Privileged containers

Privileged support in Kata is essentially different from `runc` containers.
The container runs with elevated capabilities within the guest.
This is also true with using `securityContext privileged=true` with Kubernetes.

Importantly, the default behavior to pass the host devices to a
privileged container is not supported in Kata Containers and needs to be
disabled, see [Privileged Kata Containers](how-to/privileged.md).

# Appendices

## The constraints challenge

Applying resource constraints such as cgroup, CPU, memory, and storage to a workload is not always straightforward with a VM based system. A Kata Container runs in an isolated environment inside a virtual machine. This, coupled with the architecture of Kata Containers, offers many more possibilities than are available to traditional Linux containers due to the various layers and contexts.

In some cases it might be necessary to apply the constraints to multiple levels. In other cases, the hardware isolated VM provides equivalent functionality to the the requested constraint.

The following examples outline some of the various areas constraints can be applied:

- Inside the VM

  Constrain the guest kernel. This can be achieved by passing particular values through the kernel command line used to boot the guest kernel. Alternatively, sysctl values can be applied at early boot.

- Inside the container

  Constrain the container created inside the VM.

- Outside the VM:

  - Constrain the hypervisor process by applying host-level constraints.

  - Constrain all processes running inside the hypervisor.

    This can be achieved by specifying particular hypervisor configuration options.


Note that in some circumstances it might be necessary to apply particular constraints
to more than one of the previous areas to achieve the desired level of isolation and resource control.
