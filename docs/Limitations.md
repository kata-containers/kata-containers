* [Overview](#overview)
* [Definition of a limitation](#definition-of-a-limitation)
* [Scope](#scope)
* [Contributing](#contributing)
* [Pending items](#pending-items)
    * [Runtime commands](#runtime-commands)
        * [checkpoint and restore](#checkpoint-and-restore)
        * [events command](#events-command)
        * [update command](#update-command)
    * [Networking](#networking)
        * [Docker swarm and compose support](#docker-swarm-and-compose-support)
    * [Resource management](#resource-management)
        * [docker run and shared memory](#docker-run-and-shared-memory)
        * [docker run and sysctl](#docker-run-and-sysctl)
    * [Docker daemon features](#docker-daemon-features)
        * [SELinux support](#selinux-support)
* [Architectural limitations](#architectural-limitations)
    * [Networking limitations](#networking-limitations)
        * [Support for joining an existing VM network](#support-for-joining-an-existing-vm-network)
        * [docker --net=host](#docker---nethost)
        * [docker run --link](#docker-run---link)
    * [Storage limitations](#storage-limitations)
        * [Kubernetes `volumeMounts.subPaths`](#kubernetes-volumemountssubpaths)
    * [Host resource sharing](#host-resource-sharing)
        * [docker run --privileged](#docker-run---privileged)
* [Miscellaneous](#miscellaneous)
    * [Docker --security-opt option partially supported](#docker---security-opt-option-partially-supported)
* [Appendices](#appendices)
    * [The constraints challenge](#the-constraints-challenge)

***

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
[contributors guide](https://github.com/kata-containers/community/blob/master/CONTRIBUTING.md).
If you wish to raise an issue for a new limitation, either
[raise an issue directly on the runtime](https://github.com/kata-containers/kata-containers/issues/new)
or see the
[project table of contents](https://github.com/kata-containers/kata-containers)
for advice on which repository to raise the issue against.

# Pending items

This section lists items that might be possible to fix.

## Runtime commands

### checkpoint and restore

The runtime does not provide `checkpoint` and `restore` commands. There
are discussions about using VM save and restore to give us a
`[criu](https://github.com/checkpoint-restore/criu)`-like functionality,
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

### Docker swarm and compose support

The newest version of Docker supported is specified by the
`externals.docker.version` variable in the
[versions database](https://github.com/kata-containers/runtime/blob/master/versions.yaml).

Basic Docker swarm support works. However, if you want to use custom networks
with Docker's swarm, an older version of Docker is required. This is specified
by the `externals.docker.meta.swarm-version` variable in the
[versions database](https://github.com/kata-containers/runtime/blob/master/versions.yaml).

See issue https://github.com/kata-containers/runtime/issues/175 for more information.

Docker compose normally uses custom networks, so also has the same limitations.

## Resource management

Due to the way VMs differ in their CPU and memory allocation, and sharing
across the host system, the implementation of an equivalent method for
these commands is potentially challenging.

See issue https://github.com/clearcontainers/runtime/issues/341 and [the constraints challenge](#the-constraints-challenge) for more information.

For CPUs resource management see
[CPU constraints](design/vcpu-handling.md).

### docker run and shared memory

The runtime does not implement the `docker run --shm-size` command to
set the size of the `/dev/shm tmpfs` within the container. It is possible to pass this configuration value into the VM container so the appropriate mount command happens at launch time.

See issue https://github.com/kata-containers/kata-containers/issues/21 for more information.

### docker run and sysctl

The `docker run --sysctl` feature is not implemented. At the runtime
level, this equates to the `linux.sysctl` OCI configuration. Docker
allows configuring the sysctl settings that support namespacing. From a security and isolation point of view, it might make sense to set them in the VM, which isolates sysctl settings. Also, given that each Kata Container has its own kernel, we can support setting of sysctl settings that are not namespaced. In some cases, we might need to support configuring some of the settings on both the host side Kata Container namespace and the Kata Containers kernel.

See issue https://github.com/kata-containers/runtime/issues/185 for more information.

## Docker daemon features

Some features enabled or implemented via the
[`dockerd` daemon](https://docs.docker.com/config/daemon/) configuration are not yet
implemented.

### SELinux support

The `dockerd` configuration option `"selinux-enabled": true` is not presently implemented
in Kata Containers. Enabling this option causes an OCI runtime error.

See issue https://github.com/kata-containers/runtime/issues/784 for more information.

The consequence of this is that the [Docker --security-opt is only partially supported](#docker---security-opt-option-partially-supported).

Kubernetes [SELinux labels](https://kubernetes.io/docs/tasks/configure-pod-container/security-context/#assign-selinux-labels-to-a-container) will also not be applied.

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

## Storage limitations

### Kubernetes `volumeMounts.subPaths`

Kubernetes `volumeMount.subPath` is not supported by Kata Containers at the
moment.

See [this issue](https://github.com/kata-containers/runtime/issues/2812) for more details.
[Another issue](https://github.com/kata-containers/kata-containers/issues/1728) focuses on the case of `emptyDir`.


## Host resource sharing

### docker run --privileged

Privileged support in Kata is essentially different from `runc` containers.
Kata does support `docker run --privileged` command, but in this case full access
to the guest VM is provided in addition to some host access.

The container runs with elevated capabilities within the guest and is granted
access to guest devices instead of the host devices.
This is also true with using `securityContext privileged=true` with Kubernetes.

The container may also be granted full access to a subset of host devices
(https://github.com/kata-containers/runtime/issues/1568).

See [Privileged Kata Containers](how-to/privileged.md) for how to configure some of this behavior.

# Miscellaneous

This section lists limitations where the possible solutions are uncertain.

## Docker --security-opt option partially supported

The `--security-opt=` option used by Docker is partially supported.
We only support `--security-opt=no-new-privileges` and `--security-opt seccomp=/path/to/seccomp/profile.json`
option as of today.

Note: The `--security-opt apparmor=your_profile` is not yet supported. See https://github.com/kata-containers/runtime/issues/707.
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
