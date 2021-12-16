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
[containerd runtime shimv2 architecture](https://github.com/containerd/containerd/tree/main/runtime/v2)
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

With the new architecture [Kubernetes](#kubernetes-support) can
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
| VM root | Guest VM | yes | no | rootfs inside the [guest image](#guest-image) | Hypervisor specific `[2]` | `ext4` | The first (or top) level VM environment created on a host system. |
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
- Provides better isolation between containers in a [pod](#kubernetes-support).
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
   [guest assets](#guest-assets):

   - The hypervisor [DAX](#dax) shares the [guest image](#guest-image)
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
   >   [guest kernel](#guest-kernel) to isolate the workload from the
   >   VM environment the container is created in. See the
   >   [Environments](#environments) section for an explanation of why
   >   this is done.
   >
   > - See the [guest image](#guest-image) section for details of
   >   exactly how the agent is started.

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
`DestroySandbox` [ttRPC API](../../../src/agent/protocols/protos/agent.proto) request.

## Guest assets

Kata Containers creates a VM in which to run one or more containers. It
does this by launching a [hypervisor](#hypervisor) to create the VM.
The hypervisor needs two assets for this task: a Linux kernel and a
small root filesystem image to boot the VM.

### Guest kernel

The [guest kernel](../../../tools/packaging/kernel)
is passed to the hypervisor and used to boot the VM.
The default kernel provided in Kata Containers is highly optimized for
kernel boot time and minimal memory footprint, providing only those
services required by a container workload. It is based on the latest
Linux LTS (Long Term Support) [kernel](https://www.kernel.org).

### Guest image

The hypervisor uses an image file which provides a minimal root
filesystem used by the guest kernel to boot the VM and host the Kata
Container. Kata Containers supports both initrd and rootfs based
minimal guest images. The [default packages](../../install/) provide both
an image and an initrd, both of which are created using the
[`osbuilder`](../../../tools/osbuilder) tool.

> **Notes:**
>
> - Although initrd and rootfs based images are supported, not all
>   [hypervisors](#hypervisor) support both types of image.
>
> - The guest image is *unrelated* to the image used in a container
>   workload.
>
>   For example, if a user creates a container that runs a shell in a
>   BusyBox image, they will run that shell in a BusyBox environment.
>   However, the guest image running inside the VM that is used to
>   *host* that BusyBox image could be running Clear Linux, Ubuntu,
>   Fedora or any other distribution potentially.
>
>   The `osbuilder` tool provides
>   [configurations for various common Linux distributions](../../../tools/osbuilder/rootfs-builder)
>   which can be built into either initrd or rootfs guest images.
>
> - If you are using a [packaged version of Kata
>   Containers](../../install), you can see image details by running the
>   [`kata-collect-data.sh`](../../../src/runtime/data/kata-collect-data.sh.in)
>   script as `root` and looking at the "Image details" section of the
>   output.

#### Root filesystem image

The default packaged rootfs image, sometimes referred to as the _mini
O/S_, is a highly optimized container bootstrap system.

If this image type is [configured](#configuration), when the user runs
the [example command](example-command.md):

- The [runtime](#runtime) will launch the configured [hypervisor](#hypervisor).
- The hypervisor will boot the mini-OS image using the [guest kernel](#guest-kernel).
- The kernel will start the init daemon as PID 1 (`systemd`) inside the VM root environment.
- `systemd`, running inside the mini-OS context, will launch the [agent](#agent)
  in the root context of the VM.
- The agent will create a new container environment, setting its root
  filesystem to that requested by the user (Ubuntu in [the example](example-command.md)).
- The agent will then execute the command (`sh(1)` in [the example](example-command.md))
  inside the new container.

The table below summarises the default mini O/S showing the
environments that are created, the services running in those
environments (for all platforms) and the root filesystem used by
each service:

| Process | Environment | systemd service? | rootfs | User accessible | Notes |
|-|-|-|-|-|-|
| systemd | VM root | n/a | [VM guest image](#guest-image)| [debug console][debug-console] | The init daemon, running as PID 1 |
| [Agent](#agent) | VM root | yes | [VM guest image](#guest-image)| [debug console][debug-console] | Runs as a systemd service |
| `chronyd` | VM root | yes | [VM guest image](#guest-image)| [debug console][debug-console] | Used to synchronise the time with the host |
| container workload (`sh(1)` in [the example](example-command.md)) | VM container | no | User specified (Ubuntu in [the example](example-command.md)) | [exec command](#exec-command) | Managed by the agent |

See also the [process overview](#process-overview).

> **Notes:**
>
> - The "User accessible" column shows how an administrator can access
>   the environment.
>
> - The container workload is running inside a full container
>   environment which itself is running within a VM environment.
>
> - See the [configuration files for the `osbuilder` tool](../../../tools/osbuilder/rootfs-builder)
>   for details of the default distribution for platforms other than
>   Intel x86_64.

#### Initrd image

The initrd image is a compressed `cpio(1)` archive, created from a
rootfs which is loaded into memory and used as part of the Linux
startup process. During startup, the kernel unpacks it into a special
instance of a `tmpfs` mount that becomes the initial root filesystem.

If this image type is [configured](#configuration), when the user runs
the [example command](example-command.md):

- The [runtime](#runtime) will launch the configured [hypervisor](#hypervisor).
- The hypervisor will boot the mini-OS image using the [guest kernel](#guest-kernel).
- The kernel will start the init daemon as PID 1 (the [agent](#agent))
  inside the VM root environment.
- The [agent](#agent) will create a new container environment, setting its root
  filesystem to that requested by the user (`ubuntu` in
  [the example](example-command.md)).
- The agent will then execute the command (`sh(1)` in [the example](example-command.md))
  inside the new container.

The table below summarises the default mini O/S showing the environments that are created,
the processes running in those environments (for all platforms) and
the root filesystem used by each service:

| Process | Environment | rootfs | User accessible | Notes |
|-|-|-|-|-|
| [Agent](#agent) | VM root | [VM guest image](#guest-image) | [debug console][debug-console] | Runs as the init daemon (PID 1) |
| container workload | VM container | User specified (Ubuntu in this example) | [exec command](#exec-command) | Managed by the agent |

> **Notes:**
>
> - The "User accessible" column shows how an administrator can access
>   the environment.
>
> - It is possible to use a standard init daemon such as systemd with
>   an initrd image if this is desirable.

See also the [process overview](#process-overview).

#### Image summary

| Image type | Default distro | Init daemon | Reason | Notes |
|-|-|-|-|-|
| [image](background.md#root-filesystem-image) | [Clear Linux](https://clearlinux.org) (for x86_64 systems)| systemd | Minimal and highly optimized | systemd offers flexibility |
| [initrd](#initrd-image) | [Alpine Linux](https://alpinelinux.org) | Kata [agent](#agent) (as no systemd support) | Security hardened and tiny C library |

See also:

- The [osbuilder](../../../tools/osbuilder) tool

  This is used to build all default image types.

- The [versions database](../../../versions.yaml)

  The `default-image-name` and `default-initrd-name` options specify
  the default distributions for each image type.

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
[protocol](../../../src/agent/protocols/protos).

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

Containers will typically live in their own, possibly shared, networking namespace.
At some point in a container lifecycle, container engines will set up that namespace
to add the container to a network which is isolated from the host network, but
which is shared between containers

In order to do so, container engines will usually add one end of a virtual
ethernet (`veth`) pair into the container networking namespace. The other end of
the `veth` pair is added to the host networking namespace.

This is a very namespace-centric approach as many hypervisors or VM
Managers (VMMs) such as `virt-manager` cannot handle `veth`
interfaces. Typically, `TAP` interfaces are created for VM
connectivity.

To overcome incompatibility between typical container engines expectations
and virtual machines, Kata Containers networking transparently connects `veth`
interfaces with `TAP` ones using Traffic Control:

![Kata Containers networking](../arch-images/network.png)

With a TC filter in place, a redirection is created between the container network and the
virtual machine. As an example, the CNI may create a device, `eth0`, in the container's network
namespace, which is a VETH device. Kata Containers will create a tap device for the VM, `tap0_kata`,
and setup a TC redirection filter to mirror traffic from `eth0`'s ingress to `tap0_kata`'s egress,
and a second to mirror traffic from `tap0_kata`'s ingress to `eth0`'s egress.

Kata Containers maintains support for MACVTAP, which was an earlier implementation used in Kata. TC-filter
is the default because it allows for simpler configuration, better CNI plugin compatibility, and performance
on par with MACVTAP.

Kata Containers has deprecated support for bridge due to lacking performance relative to TC-filter and MACVTAP.

Kata Containers supports both
[CNM](https://github.com/docker/libnetwork/blob/master/docs/design.md#the-container-network-model)
and [CNI](https://github.com/containernetworking/cni) for networking management.

### Network Hotplug

Kata Containers has developed a set of network sub-commands and APIs to add, list and
remove a guest network endpoint and to manipulate the guest route table.

The following diagram illustrates the Kata Containers network hotplug workflow.

![Network Hotplug](../arch-images/kata-containers-network-hotplug.png)

## Storage

See the [storage document](storage.md).

## Kubernetes support

[Kubernetes](https://github.com/kubernetes/kubernetes/), or K8s, is a popular open source
container orchestration engine. In Kubernetes, a set of containers sharing resources
such as networking, storage, mount, PID, etc. is called a
[pod](https://kubernetes.io/docs/user-guide/pods/).

A node can have multiple pods, but at a minimum, a node within a Kubernetes cluster
only needs to run a container runtime and a container agent (called a
[Kubelet](https://kubernetes.io/docs/admin/kubelet/)).

Kata Containers represents a Kubelet pod as a VM.

A Kubernetes cluster runs a control plane where a scheduler (typically
running on a dedicated master node) calls into a compute Kubelet. This
Kubelet instance is responsible for managing the lifecycle of pods
within the nodes and eventually relies on a container runtime to
handle execution. The Kubelet architecture decouples lifecycle
management from container execution through a dedicated gRPC based
[Container Runtime Interface (CRI)](https://github.com/kubernetes/community/blob/master/contributors/design-proposals/node/container-runtime-interface-v1.md).

In other words, a Kubelet is a CRI client and expects a CRI
implementation to handle the server side of the interface.
[CRI-O](https://github.com/kubernetes-incubator/cri-o) and
[containerd](https://github.com/containerd/containerd/) are CRI
implementations that rely on
[OCI](https://github.com/opencontainers/runtime-spec) compatible
runtimes for managing container instances.

Kata Containers is an officially supported CRI-O and containerd
runtime. Refer to the following guides on how to set up Kata
Containers with Kubernetes:

- [How to use Kata Containers and containerd](../../how-to/containerd-kata.md)
- [Run Kata Containers with Kubernetes](../../how-to/run-kata-with-k8s.md)

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
feature to efficiently map the [guest image](#guest-image) in the
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
the [guest kernel](#guest-kernel) use the appropriate emulated device
as its rootfs.

### DAX advantages

Mapping files using [DAX](#dax) provides a number of benefits over
more traditional VM file and device mapping mechanisms:

- Mapping as a direct access device allows the guest to directly
  access the host memory pages (such as via Execute In Place (XIP)),
  bypassing the [guest kernel](#guest-kernel)'s page cache. This
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

[debug-console]: ../../Developer-Guide.md#connect-to-debug-console
