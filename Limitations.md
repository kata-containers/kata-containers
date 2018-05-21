* [Overview](#overview)
* [Definiton of a limitation](#definiton-of-a-limitation)
* [Scope](#scope)
* [Contributing](#contributing)
* [Pending items](#pending-items)
    * [Runtime commands](#runtime-commands)
        * [checkpoint and restore](#checkpoint-and-restore)
        * [events command](#events-command)
        * [ps command](#ps-command)
        * [update command](#update-command)
    * [Networking](#networking)
        * [Adding networks dynamically](#adding-networks-dynamically)
        * [Docker swarm support](#docker-swarm-support)
    * [Resource management](#resource-management)
        * [docker run and kernel memory](#docker-run-and-kernel-memory)
        * [docker run and shared memory](#docker-run-and-shared-memory)
        * [docker run and sysctl](#docker-run-and-sysctl)
* [Architectural limitations](#architectural-limitations)
    * [Networking limitations](#networking-limitations)
        * [Support for joining an existing VM network](#support-for-joining-an-existing-vm-network)
        * [docker --net=host](#docker---net=host)
        * [docker run --link](#docker-run---link)
    * [Host resource sharing](#host-resource-sharing)
        * [docker run --privileged](#docker-run---privileged)
* [Miscellaneous](#miscellaneous)
    * [Docker ramdisk not supported](#docker-ramdisk-not-supported)
* [Appendices](#appendices)
    * [The constraints challenge](#the-constraints-challenge)

---

# Overview

A [Kata Container](https://github.com/kata-containers) utilizes a Virtual Machine (VM) to enhance security and
isolation of container workloads. As a result, the system has a number of differences
and limitations when compared with the default [Docker*](https://www.docker.com/) runtime,
[`runc`](https://github.com/opencontainers/runc).

Some of these limitations have potential solutions, whereas others exist
due to fundamental architectural differences generally related to the
use of VMs.

The [Kata Container runtime](https://github.com/kata-containers/runtime)
launches each container within its own hardware isolated VM, and each VM has
its own kernel. Due to this higher degree of isolation, certain container
capabilities cannot be supported or are implicitly enabled through the VM.

# Definiton of a limitation

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

Each known limitation is captured in a separate github issue that contains
detailed information about the issue. These issues are tagged with the
`limitation` label. This document is a curated summary of important known
limitations and provides links to the relevant github issues.

The following link shows the latest list of limitations:

- https://github.com/pulls?utf8=%E2%9C%93&q=is%3Aopen+label%3Alimitation+org%3Akata-containers

# Contributing

If you would like to work on resolving a limitation, please refer to the
[contributers guide](https://github.com/kata-containers/community/blob/master/CONTRIBUTING.md).
If you wish to raise an issue for a new limitation, either
[raise an issue directly on the runtime](https://github.com/kata-containers/runtime/issues/new)
or see the
[project table of contents](https://github.com/kata-containers/kata-containers)
for advice on which repository to raise the issue against.

# Pending items

This section lists items that might be possible to fix.

## Runtime commands

### checkpoint and restore

The runtime does not provide `checkpoint` and `restore` commands. There
are discussions about using VM save and restore to give [`criu`](https://github.com/checkpoint-restore/criu)-like functionality, which might provide a solution.

Note that the OCI standard does not specify `checkpoint` and `restore`
commands.

See issue https://github.com/kata-containers/runtime/issues/184 for more information.

### events command

The runtime does not fully implement the `events` command. `OOM` notifications and `Intel RDT` stats are not fully supported.

Note that the OCI standard does not specify an `events` command.

See issue https://github.com/kata-containers/runtime/issues/308 and https://github.com/kata-containers/runtime/issues/309 for more information.

### ps command

The Kata Containers runtime does not currently support the `ps` command.

Note that this is *not* the same as the `docker ps` command. The runtime `ps`
command lists the processes running within a container. The `docker ps`
command lists the containers themselves. The runtime `ps` command is
invoked from `docker top`.

Note that the OCI standard does not specify a `ps` command.

See issue https://github.com/kata-containers/runtime/issues/129 for more information.

### update command

The runtime does not currently implement the update command, hence
does not support some of the `docker update` functionality. Much of the
`update` functionality is based around cgroup configurations.

It might be possible to implement some of the update functionality by adjusting cgroups either around the VM or inside the container VM, or by some other VM functional equivalent. See [the constraints challenge](#the-constraints-challenge) section for further information on how to handle constraints.

Note that the OCI standard does not specify an `update` command.

See issue https://github.com/kata-containers/runtime/issues/189 for more information.

## Networking

### Adding networks dynamically

The runtime does not currently support adding networks to an already
running container (`docker network connect`).

The VM network configuration is set up with what is defined by the CNM
plugin at startup time. Although it is possible to watch the networking namespace on the host to discover and propagate new networks at runtime, it is currently not implemented.

See https://github.com/kata-containers/runtime/issues/113 for more information.

### Docker swarm support

The newest version of Docker supported is specified by the
`externals.docker.version` variable in the
[versions database](https://github.com/kata-containers/runtime/blob/master/versions.yaml).

Basic Docker swarm support works. However, if you want to use custom networks
with Docker's swarm, an older version of Docker is required. This is specified
by the `externals.docker.meta.swarm-version` variable in the
[versions database](https://github.com/kata-containers/runtime/blob/master/versions.yaml).

See issue https://github.com/kata-containers/runtime/issues/175 for more information.

## Resource management

Due to the way VMs differ in their CPU and memory allocation, and sharing
across the host system, the implementation of an equivalent method for
these commands is potentially challenging.

See issue https://github.com/clearcontainers/runtime/issues/341 and [the constraints challenge](#the-constraints-challenge) for more information.

For CPUs resource management see
[cpu-constraints](https://github.com/kata-containers/runtime/blob/master/docs/cpu-constraints.md).

### docker run and kernel memory

The `docker run --kernel-memory=` option is not currently implemented.
It should be possible to pass this information through to the QEMU
command line CPU configuration options to gain a similar effect.

See issue https://github.com/kata-containers/runtime/issues/187 for more information.

### docker run and shared memory

The runtime does not implement the `docker run --shm-size` command to
set the size of the `/dev/shm tmpfs` within the container. It is possible to pass this configuration value into the VM container so the appropriate mount command happens at launch time.

See issue https://github.com/kata-containers/kata-containers/issues/21 for more information.

### docker run and sysctl

The `docker run --sysctl` feature is not implemented. At the runtime
level, this equates to the `linux.sysctl` OCI configuration. Docker
allows configuring the sysctl settings that support namespacing. From a security and isolation point of view, it might make sense to set them in the VM, which isolates sysctl settings. Also, given that each Kata Container has its own kernel, we can support setting of sysctl settings that are not namespaced. In some cases, we might need to support configuring some of the settings on both the host side Kata Container namespace and the Kata Containers kernel.

See issue https://github.com/kata-containers/runtime/issues/185 for more information.

# Architectural limitations

This section lists items that might not be fixed due to fundamental
architectural differences between "soft containers" (i.e. traditional Linux*
containers) and those based on VMs.

## Networking limitations

### Support for joining an existing VM network

Docker supports the ability for containers to join another containers
namespace with the `docker run --net=containers` syntax. This allows
multiple containers to share a common network namespace and the network
interfaces placed in the network namespace.  Kata Containers does not
support network namespace sharing. If a Kata Container is setup to
share the network namespace of a `runc` container, the runtime
effectively takes over all the network interfaces assigned to the
namespace and binds them to the VM. Consequently, the `runc` container loses
its network connectivity.

### docker --net=host

Docker host network support (`docker --net=host run`) is not supported.
It is not possible to directly access the host networking configuration
from within the VM.

The `--net=host` option can still be used with `runc` containers and
inter-mixed with running Kata Containers, thus enabling use of `--net=host`
when necessary.

It should be noted, currently passing the `--net=host` option into a
Kata Container may result in the Kata Container networking setup
modifying, re-configuring and therefore possibly breaking the host
networking setup. Do not use `--net=host` with Kata Containers.

### docker run --link

The runtime does not support the `docker run --link` command. This
command is now deprecated by docker and we have no intention of adding support.
Equivalent functionality can be achieved with the newer docker networking commands.

See more documentation at
[docs.docker.com](https://docs.docker.com/engine/userguide/networking/default_network/dockerlinks/).

## Host resource sharing

### docker run --privileged

The `docker run --privileged` command is not supported in the runtime.
There is no simple way to grant the VM access to all of the host devices that this command needs to be complete.

The `--privileged` option can be used with `runc` containers and inter-mixed with running Kata Containers. This enables use of `--privileged` when necessary.

# Miscellaneous

This section lists limitations where the possible solutions are uncertain.

## Docker ramdisk not supported

The `DOCKER_RAMDISK=true` environment variable used by Docker to force the
container to run entirely on a RAM disk is not supported.

See https://github.com/kata-containers/runtime/issues/134 for more information.

# Appendices

## The constraints challenge

Applying resource constraints such as cgroup, cpu, memory, and storage to a workload is not always straightforward with a VM based system. A Kata Container runs in an isolated environment inside a virtual machine. This, coupled with the architecture of Kata Containers, offers many more possibilities than are available to traditional Linux containers due to the various layers and contexts.

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

  - Constrain the [shim](https://github.com/kata-containers/shim) process.

    This process represents the container workload running inside the VM.

  - Constrain the [proxy](https://github.com/kata-containers/proxy) process.

Note that in some circumstances it might be necessary to apply particular constraints
to more than one of the previous areas to achieve the desired level of isolation and resource control.
