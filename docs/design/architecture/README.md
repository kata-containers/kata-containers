# Kata Containers Architecture

## Overview

Kata Containers is an open source community working to build a secure
container [runtime](#runtime) with lightweight virtual machines (VM's)
that feel and perform like standard Linux containers, but provide
stronger [workload](#workload) isolation using hardware
[virtualization](#virtualization) technology as a second layer of
defence.

Kata Containers runs on [multiple architectures](../../../src/runtime/README.md#platform-support)
and supports [multiple hypervisors](../../hypervisors.md).

This document is a summary of the Kata Containers architecture.

## Background knowledge

This document assumes the reader understands a number of concepts
related to containers and file systems. The
[background](background.md) document explains these concepts.

## Example command

This document makes use of a particular [example
command](example-command.md) throughout the text to illustrate certain
concepts.

## Virtualization

For details on how Kata Containers maps container concepts to VM
technologies, and how this is realized in the multiple hypervisors and
VMMs that Kata supports see the
[virtualization documentation](../virtualization.md).

## Compatibility

The [Kata Containers runtime](../../../src/runtime) is compatible with
the [OCI](https://github.com/opencontainers)
[runtime specification](https://github.com/opencontainers/runtime-spec)
and therefore works seamlessly with the
[Kubernetes Container Runtime Interface (CRI)](https://github.com/kubernetes/community/blob/master/contributors/devel/sig-node/container-runtime-interface.md)
through the [CRI-O](https://github.com/kubernetes-incubator/cri-o)
and [containerd](https://github.com/containerd/containerd)
implementations.

Kata Containers provides a ["shimv2"](#shim-v2-architecture) compatible runtime.

## Shim v2 architecture

The Kata Containers runtime is shim v2 ("shimv2") compatible. This
section explains what this means.

> **Note:**
>
> For a comparison with the Kata 1.x architecture, see
> [the architectural history document](history.md).

The
[containerd runtime shimv2 architecture](https://github.com/containerd/containerd/tree/main/core/runtime/v2)
or _shim API_ architecture resolves the issues with the old
architecture by defining a set of shimv2 APIs that a compatible
runtime implementation must supply. Rather than calling the runtime
binary multiple times for each new container, the shimv2 architecture
runs a single instance of the runtime binary (for any number of
containers). This improves performance and resolves the state handling
issue.

The shimv2 API is similar to the
[OCI runtime](https://github.com/opencontainers/runtime-spec)
API in terms of the way the container lifecycle is split into
different verbs. Rather than calling the runtime multiple times, the
container manager creates a socket and passes it to the shimv2
runtime. The socket is a bi-directional communication channel that
uses a gRPC based protocol to allow the container manager to send API
calls to the runtime, which returns the result to the container
manager using the same channel.

The shimv2 architecture allows running several containers per VM to
support container engines that require multiple containers running
inside a pod.

With the new architecture [Kubernetes](kubernetes.md) can
launch both Pod and OCI compatible containers with a single
[runtime](#runtime) shim per Pod, rather than `2N+1` shims. No stand
alone `kata-proxy` process is required, even if VSOCK is not
available.

## Workload

The workload is the command the user requested to run in the
container and is specified in the [OCI bundle](background.md#oci-bundle)'s
configuration file.

In our [example](example-command.md), the workload is the `sh(1)` command.

### Workload root filesystem

For details of how the [runtime](#runtime) makes the
[container image](background.md#container-image) chosen by the user available to
the workload process, see the
[Container creation](#container-creation) and [storage](#storage) sections.

Note that the workload is isolated from the [guest VM](#environments) environment by its
surrounding [container environment](#environments). The guest VM
environment where the container runs in is also isolated from the _outer_
[host environment](#environments) where the container manager runs.

## System overview

### Environments

The following terminology is used to describe the different or
environments (or contexts) various processes run in. It is necessary
to study this table closely to make sense of what follows:

| Type | Name | Virtualized | Containerized | rootfs | Rootfs device type | Mount type | Description |
|-|-|-|-|-|-|-|-|
| Host | Host | no `[1]` | no | Host specific | Host specific | Host specific | The environment provided by a standard, physical non virtualized system. |
| VM root | Guest VM | yes | no | rootfs inside the [guest image](guest-assets.md#guest-image) | Hypervisor specific `[2]` | `ext4` | The first (or top) level VM environment created on a host system. |
| VM container root | Container | yes | yes | rootfs type requested by user ([`ubuntu` in the example](example-command.md)) | `kataShared` | [virtio FS](storage.md#virtio-fs) | The first (or top) level container environment created inside the VM. Based on the [OCI bundle](background.md#oci-bundle). |

**Key:**

- `[1]`: For simplicity, this document assumes the host environment
  runs on physical hardware.

- `[2]`: See the [DAX](#dax) section.

> **Notes:**
>
> - The word "root" is used to mean _top level_ here in a similar
>   manner to the term [rootfs](background.md#root-filesystem).
>
> - The term "first level" prefix used above is important since it implies
>   that it is possible to create multi level systems. However, they do
>   not form part of a standard Kata Containers environment so will not
>   be considered in this document.

The reasons for containerizing the [workload](#workload) inside the VM
are:

- Isolates the workload entirely from the VM environment.
- Provides better isolation between containers in a [pod](kubernetes.md).
- Allows the workload to be managed and monitored through its cgroup
  confinement.

### Container creation

The steps below show at a high level how a Kata Containers container is
created using the containerd container manager:

1. The user requests the creation of a container by running a command
   like the [example command](example-command.md).
1. The container manager daemon runs a single instance of the Kata
   [runtime](#runtime).
1. The Kata runtime loads its [configuration file](#configuration).
1. The container manager calls a set of shimv2 API functions on the runtime.
1. The Kata runtime launches the configured [hypervisor](#hypervisor).
1. The hypervisor creates and starts (_boots_) a VM using the
   [guest assets](guest-assets.md#guest-assets):

   - The hypervisor [DAX](#dax) shares the
     [guest image](guest-assets.md#guest-image)
     into the VM to become the VM [rootfs](background.md#root-filesystem) (mounted on a `/dev/pmem*` device),
     which is known as the [VM root environment](#environments).
   - The hypervisor mounts the [OCI bundle](background.md#oci-bundle), using [virtio FS](storage.md#virtio-fs),
     into a container specific directory inside the VM's rootfs.

     This container specific directory will become the
     [container rootfs](#environments), known as the
     [container environment](#environments).

1. The [agent](#agent) is started as part of the VM boot.

1. The runtime calls the agent's `CreateSandbox` API to request the
   agent create a container:

   1. The agent creates a [container environment](#environments)
      in the container specific directory that contains the [container rootfs](#environments).

      The container environment hosts the [workload](#workload) in the
      [container rootfs](#environments) directory.

   1. The agent spawns the workload inside the container environment.

   > **Notes:**
   >
   > - The container environment created by the agent is equivalent to
   >   a container environment created by the
   >   [`runc`](https://github.com/opencontainers/runc) OCI runtime;
   >   Linux cgroups and namespaces are created inside the VM by the
   >   [guest kernel](guest-assets.md#guest-kernel) to isolate the
   >   workload from the VM environment the container is created in.
   >   See the [Environments](#environments) section for an
   >   explanation of why this is done.
   >
   > - See the [guest image](guest-assets.md#guest-image) section for
   >   details of exactly how the agent is started.

1. The container manager returns control of the container to the
   user running the `ctr` command.

> **Note:**
>
> At this point, the container is running and:
>
> - The [workload](#workload) process ([`sh(1)` in the example](example-command.md))
>   is running in the [container environment](#environments).
> - The user is now able to interact with the workload
>   (using the [`ctr` command in the example](example-command.md)).
> - The [agent](#agent), running inside the VM is monitoring the
>   [workload](#workload) process.
> - The [runtime](#runtime) is waiting for the agent's `WaitProcess` API
>   call to complete.

Further details of these steps are provided in the sections below.

### Container shutdown

There are two possible ways for the container environment to be
terminated:

- When the [workload](#workload) exits.

  This is the standard, or _graceful_ shutdown method.

- When the container manager forces the container to be deleted.

#### Workload exit

The [agent](#agent) will detect when the [workload](#workload) process
exits, capture its exit status (see `wait(2)`) and return that value
to the [runtime](#runtime) by specifying it as the response to the
`WaitProcess` agent API call made by the [runtime](#runtime).

The runtime then passes the value back to the container manager by the
`Wait` [shimv2 API](#shim-v2-architecture) call.

Once the workload has fully exited, the VM is no longer needed and the
runtime cleans up the environment (which includes terminating the
[hypervisor](#hypervisor) process).

> **Note:**
>
> When [agent tracing is enabled](../../tracing.md#agent-shutdown-behaviour),
> the shutdown behaviour is different.

#### Container manager requested shutdown

If the container manager requests the container be deleted, the
[runtime](#runtime) will signal the agent by sending it a
`DestroySandbox` [ttRPC API](../../../src/libs/protocols/protos/agent.proto) request.

## Guest assets

The guest assets comprise a guest image and a guest kernel that are
used by the [hypervisor](#hypervisor).

See the [guest assets](guest-assets.md) document for further
information.

## Hypervisor

The [hypervisor](../../hypervisors.md) specified in the
[configuration file](#configuration) creates a VM to host the
[agent](#agent) and the [workload](#workload) inside the
[container environment](#environments).

> **Note:**
>
> The hypervisor process runs inside an environment slightly different
> to the host environment:
>
> - It is run in a different cgroup environment to the host.
> - It is given a separate network namespace from the host.
> - If the [OCI configuration specifies a SELinux label](https://github.com/opencontainers/runtime-spec/blob/main/config.md#linux-process),
>   the hypervisor process will run with that label (*not* the workload running inside the hypervisor's VM).

## Agent

The Kata Containers agent ([`kata-agent`](../../../src/agent)), written
in the [Rust programming language](https://www.rust-lang.org), is a
long running process that runs inside the VM. It acts as the
supervisor for managing the containers and the [workload](#workload)
running within those containers. Only a single agent process is run
for each VM created.

### Agent communications protocol

The agent communicates with the other Kata components (primarily the
[runtime](#runtime)) using a
[`ttRPC`](https://github.com/containerd/ttrpc-rust) based
[protocol](../../../src/libs/protocols/protos).

> **Note:**
>
> If you wish to learn more about this protocol, a practical way to do
> so is to experiment with the
> [agent control tool](#agent-control-tool) on a test system.
> This tool is for test and development purposes only and can send
> arbitrary ttRPC agent API commands to the [agent](#agent).

## Runtime

The Kata Containers runtime (the [`containerd-shim-kata-v2`](../../../src/runtime/cmd/containerd-shim-kata-v2
) binary) is a [shimv2](#shim-v2-architecture) compatible runtime.

> **Note:**
>
> The Kata Containers runtime is sometimes referred to as the Kata
> _shim_. Both terms are correct since the `containerd-shim-kata-v2`
> is a container runtime, and that runtime implements the containerd
> shim v2 API.

The runtime makes heavy use of the [`virtcontainers`
package](../../../src/runtime/virtcontainers), which provides a generic,
runtime-specification agnostic, hardware-virtualized containers
library.

The runtime is responsible for starting the [hypervisor](#hypervisor)
and it's VM, and communicating with the [agent](#agent) using a
[ttRPC based protocol](#agent-communications-protocol) over a VSOCK
socket that provides a communications link between the VM and the
host. 

This protocol allows the runtime to send container management commands
to the agent. The protocol is also used to carry the standard I/O
streams (`stdout`, `stderr`, `stdin`) between the containers and
container managers (such as CRI-O or containerd).

## Utility program

The `kata-runtime` binary is a utility program that provides
administrative commands to manipulate and query a Kata Containers
installation.

> **Note:**
>
> In Kata 1.x, this program also acted as the main
> [runtime](#runtime), but this is no longer required due to the
> improved shimv2 architecture.

### exec command

The `exec` command allows an administrator or developer to enter the
[VM root environment](#environments) which is not accessible by the container
[workload](#workload).

See [the developer guide](../../Developer-Guide.md#connect-to-debug-console) for further details.

### policy command

The `policy set` command allows an administrator or developer to set the policy
to [VM root environment](#environments). In this way, we can enable/disable
kata-agent API through policy.
The command is: `kata-runtime policy set policy.rego --sandbox-id XXXXXXXX`

Please refer to [`genpolicy tool`](../../../src/tools/genpolicy/README.md) to see how to generate `policy.rego` mentioned above.
And more about policy itself can be found at [Policy Details](../../../src/tools/genpolicy/genpolicy-auto-generated-policy-details.md).

### Configuration

See the [configuration file details](../../../src/runtime/README.md#configuration).

The configuration file is also used to enable runtime [debug output](../../Developer-Guide.md#enable-full-debug).

## Process overview

The table below shows an example of the main processes running in the
different [environments](#environments) when a Kata Container is
created with containerd using our [example command](example-command.md):

| Description | Host | VM root environment | VM container environment |
|-|-|-|-|
| Container manager | `containerd` | |
| Kata Containers | [runtime](#runtime), [`virtiofsd`](storage.md#virtio-fs), [hypervisor](#hypervisor) | [agent](#agent) |
| User [workload](#workload) | | | [`ubuntu sh`](example-command.md) |

## Networking

See the [networking document](networking.md).

## Storage

See the [storage document](storage.md).

## Kubernetes support

See the [Kubernetes document](kubernetes.md).

####  OCI annotations

In order for the Kata Containers [runtime](#runtime) (or any VM based OCI compatible
runtime) to be able to understand if it needs to create a full VM or if it
has to create a new container inside an existing pod's VM, CRI-O adds
specific annotations to the OCI configuration file (`config.json`) which is passed to
the OCI compatible runtime.

Before calling its runtime, CRI-O will always add a `io.kubernetes.cri-o.ContainerType`
annotation to the `config.json` configuration file it produces from the Kubelet CRI
request. The `io.kubernetes.cri-o.ContainerType` annotation can either be set to `sandbox`
or `container`. Kata Containers will then use this annotation to decide if it needs to
respectively create a virtual machine or a container inside a virtual machine associated
with a Kubernetes pod:

| Annotation value | Kata VM created? | Kata container created? |
|-|-|-|
| `sandbox` | yes | yes (inside new VM) |
| `container`| no | yes (in existing VM) |

#### Mixing VM based and namespace based runtimes

> **Note:** Since Kubernetes 1.12, the [`Kubernetes RuntimeClass`](https://kubernetes.io/docs/concepts/containers/runtime-class/)
> has been supported and the user can specify runtime without the non-standardized annotations.

With `RuntimeClass`, users can define Kata Containers as a
`RuntimeClass` and then explicitly specify that a pod must be created
as a Kata Containers pod. For details, please refer to [How to use
Kata Containers and containerd](../../../docs/how-to/containerd-kata.md).

## Tracing

The [tracing document](../../tracing.md) provides details on the tracing
architecture.

# Appendices

## DAX

Kata Containers utilizes the Linux kernel DAX
[(Direct Access filesystem)](https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/Documentation/filesystems/dax.rst?h=v5.14)
feature to efficiently map the [guest image](guest-assets.md#guest-image) in the
[host environment](#environments) into the
[guest VM environment](#environments) to become the VM's
[rootfs](background.md#root-filesystem).

If the [configured](#configuration) [hypervisor](#hypervisor) is set
to either QEMU or Cloud Hypervisor, DAX is used with the feature shown
in the table below:

| Hypervisor | Feature used | rootfs device type |
|-|-|-|
| Cloud Hypervisor (CH) | `dax` `FsConfig` configuration option | PMEM (emulated Persistent Memory device) |
| QEMU | NVDIMM memory device with a memory file backend | NVDIMM (emulated Non-Volatile Dual In-line Memory Module device) |

The features in the table above are equivalent in that they provide a memory-mapped
virtual device which is used to DAX map the VM's
[rootfs](background.md#root-filesystem) into the [VM guest](#environments) memory
address space.

The VM is then booted, specifying the `root=` kernel parameter to make
the [guest kernel](guest-assets.md#guest-kernel) use the appropriate emulated device
as its rootfs.

### DAX advantages

Mapping files using [DAX](#dax) provides a number of benefits over
more traditional VM file and device mapping mechanisms:

- Mapping as a direct access device allows the guest to directly
  access the host memory pages (such as via Execute In Place (XIP)),
  bypassing the [guest kernel](guest-assets.md#guest-kernel)'s page cache. This
  zero copy provides both time and space optimizations.

- Mapping as a direct access device inside the VM allows pages from the
  host to be demand loaded using page faults, rather than having to make requests
  via a virtualized device (causing expensive VM exits/hypercalls), thus providing
  a speed optimization.

- Utilizing `mmap(2)`'s `MAP_SHARED` shared memory option on the host
  allows the host to efficiently share pages.

![DAX](../arch-images/DAX.png)

For further details of the use of NVDIMM with QEMU, see the [QEMU
project documentation](https://www.qemu.org).

## Agent control tool

The [agent control tool](../../../src/tools/agent-ctl) is a test and
development tool that can be used to learn more about a Kata
Containers system.

## Terminology

See the [project glossary](../../../Glossary.md).
