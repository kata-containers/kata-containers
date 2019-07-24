# Kata Containers Architecture

* [Overview](#overview)
* [Hypervisor](#hypervisor)
  * [Assets](#assets)
    * [Guest kernel](#guest-kernel)
    * [Root filesystem image](#root-filesystem-image)
* [Agent](#agent)
* [Runtime](#runtime)
    * [Configuration](#configuration)
    * [Significant OCI commands](#significant-oci-commands)
        * [create](#create)
        * [start](#start)
        * [exec](#exec)
        * [kill](#kill)
        * [delete](#delete)
* [Proxy](#proxy)
* [Shim](#shim)
* [Networking](#networking)
* [Storage](#storage)
* [Kubernetes Support](#kubernetes-support)
    * [Problem Statement](#problem-statement)
    * [Containerd](#containerd)
    * [CRI-O](#cri-o)
        * [OCI Annotations](#oci-annotations)
        * [Mixing VM based and namespace based runtimes](#mixing-vm-based-and-namespace-based-runtimes)
* [Appendices](#appendices)
    * [DAX](#dax)

## Overview

This is an architectural overview of Kata Containers, based on the 1.5.0 release.

The two primary deliverables of the Kata Containers project are a container runtime
and a CRI friendly shim. There is also a CRI friendly library API behind them.

The [Kata Containers runtime (`kata-runtime`)](https://github.com/kata-containers/runtime)
is compatible with the [OCI](https://github.com/opencontainers) [runtime specification](https://github.com/opencontainers/runtime-spec)
and therefore works seamlessly with the
[Docker\* Engine](https://www.docker.com/products/docker-engine) pluggable runtime
architecture. It also supports the [Kubernetes\* Container Runtime Interface (CRI)](https://github.com/kubernetes/community/blob/master/contributors/devel/sig-node/container-runtime-interface.md)
through the [CRI-O\*](https://github.com/kubernetes-incubator/cri-o) and
[Containerd CRI Plugin\*](https://github.com/containerd/cri) implementation. In other words, you can transparently
select between the [default Docker and CRI shim runtime (runc)](https://github.com/opencontainers/runc)
and `kata-runtime`.

`kata-runtime` creates a QEMU\*/KVM virtual machine for each container or pod,
the Docker engine or `kubelet` (Kubernetes) creates respectively.

![Docker and Kata Containers](arch-images/docker-kata.png)

The [`containerd-shim-kata-v2` (shown as `shimv2` from this point onwards)](https://github.com/kata-containers/runtime/tree/master/containerd-shim-v2) 
is another Kata Containers entrypoint, which 
implements the [Containerd Runtime V2 (Shim API)](https://github.com/containerd/containerd/tree/master/runtime/v2) for Kata.
With `shimv2`, Kubernetes can launch Pod and OCI compatible containers with one shim (the `shimv2`) per Pod instead
of `2N+1` shims (a `containerd-shim` and a `kata-shim` for each container and the Pod sandbox itself), and no standalone
`kata-proxy` process even if no VSOCK is available.

![Kubernetes integration with shimv2](arch-images/shimv2.svg)

The container process is then spawned by
[agent](https://github.com/kata-containers/agent), an agent process running
as a daemon inside the virtual machine. `kata-agent` runs a gRPC server in
the guest using a VIRTIO serial or VSOCK interface which QEMU exposes as a socket
file on the host. `kata-runtime` uses a gRPC protocol to communicate with
the agent. This protocol allows the runtime to send container management
commands to the agent. The protocol is also used to carry the I/O streams (stdout,
stderr, stdin) between the containers and the manage engines (e.g. Docker Engine).

For any given container, both the init process and all potentially executed
commands within that container, together with their related I/O streams, need
to go through the VIRTIO serial or VSOCK interface exported by QEMU. 
In the VIRTIO serial case, a [Kata Containers
proxy (`kata-proxy`)](https://github.com/kata-containers/proxy) instance is
launched for each virtual machine to handle multiplexing and demultiplexing
those commands and streams.

On the host, each container process's removal is handled by a reaper in the higher
layers of the container stack. In the case of Docker or containerd it is handled by `containerd-shim`.
In the case of CRI-O it is handled by `conmon`. For clarity, for the remainder
of this document the term "container process reaper" will be used to refer to
either reaper. As Kata Containers processes run inside their own  virtual machines,
the container process reaper cannot monitor, control
or reap them. `kata-runtime` fixes that issue by creating an [additional shim process
(`kata-shim`)](https://github.com/kata-containers/shim) between the container process
reaper and `kata-proxy`. A `kata-shim` instance will both forward signals and `stdin`
streams to the container process on the guest and pass the container `stdout`
and `stderr` streams back up the stack to the CRI shim or Docker via the container process
reaper. `kata-runtime` creates a `kata-shim` daemon for each container and for each
OCI command received to run within an already running container (example, `docker
exec`).

Since Kata Containers version 1.5, the new introduced `shimv2` has integrated the
functionalities of the reaper, the `kata-runtime`, the `kata-shim`, and the `kata-proxy`.
As a result, there will not be any of the additional processes previously listed.

The container workload, that is, the actual OCI bundle rootfs, is exported from the
host to the virtual machine.  In the case where a block-based graph driver is
configured, `virtio-scsi` will be used. In all other cases a 9pfs VIRTIO mount point
will be used. `kata-agent` uses this mount point as the root filesystem for the
container processes.

## Hypervisor

Kata Containers is designed to support multiple hypervisors.  For the 1.0 release,
Kata Containers uses just [QEMU](http://www.qemu-project.org/)/[KVM](http://www.linux-kvm.org/page/Main_Page)
to create virtual machines where containers will run:

![QEMU/KVM](arch-images/qemu.png)

### QEMU/KVM

Depending on the host architecture, Kata Containers supports various machine types,
for example `pc` and `q35` on x86 systems, `virt` on ARM systems and `pseries` on IBM Power systems. The default Kata Containers
machine type is `pc`. The default machine type and its [`Machine accelerators`](#machine-accelerators) can
be changed by editing the runtime [`configuration`](#configuration) file.

The following QEMU features are used in Kata Containers to manage resource constraints, improve
boot time and reduce memory footprint:

- Machine accelerators.
- Hot plug devices.

Each feature is documented below.

#### Machine accelerators

Machine accelerators are architecture specific and can be used to improve the performance
and enable specific features of the machine types. The following machine accelerators
are used in Kata Containers:

- NVDIMM: This machine accelerator is x86 specific and only supported by `pc` and
`q35` machine types. `nvdimm` is used to provide the root filesystem as a persistent
memory device to the Virtual Machine.

Although Kata Containers can run with any recent QEMU release, Kata Containers
boot time, memory footprint and 9p IO are significantly optimized by using a specific
QEMU version called [`qemu-lite`](https://github.com/kata-containers/qemu/tree/qemu-lite-2.11.0) and
custom machine accelerators that are not available in the upstream version of QEMU.
These custom machine accelerators are described below.

- `nofw`: this machine accelerator is x86 specific and only supported by `pc` and `q35`
machine types. `nofw` is used to boot an ELF format kernel by skipping the BIOS/firmware
in the guest. This custom machine accelerator improves boot time significantly.
- `static-prt`: this machine accelerator is x86 specific and only supported by `pc`
and `q35` machine types. `static-prt` is used to reduce the interpretation burden
for guest ACPI component.

#### Hot plug devices

The Kata Containers VM starts with a minimum amount of resources, allowing for faster boot time and a reduction in memory footprint.  As the container launch progresses, devices are hotplugged to the VM. For example, when a CPU constraint is specified which includes additional CPUs, they can be hot added.  Kata Containers has support for hot-adding the following devices:
- Virtio block
- Virtio SCSI
- VFIO
- CPU

### Assets

The hypervisor will launch a virtual machine which includes a minimal guest kernel
and a guest image.

#### Guest kernel

The guest kernel is passed to the hypervisor and used to boot the virtual
machine. The default kernel provided in Kata Containers is highly optimized for
kernel boot time and minimal memory footprint, providing only those services
required by a container workload. This is based on a very current upstream Linux
kernel.

#### Guest image

Kata Containers supports both an `initrd` and `rootfs` based minimal guest image.

##### Root filesystem image

The default packaged root filesystem image, sometimes referred to as the "mini O/S", is a
highly optimized container bootstrap system based on [Clear Linux](https://clearlinux.org/). It provides an extremely minimal environment and
has a highly optimized boot path.

The only services running in the context of the mini O/S are the init daemon
(`systemd`) and the [Agent](#agent). The real workload the user wishes to run
is created using libcontainer, creating a container in the same manner that is done
by `runc`.

For example, when `docker run -ti ubuntu date` is run:

- The hypervisor will boot the mini-OS image using the guest kernel.
- `systemd`, running inside the mini-OS context, will launch the `kata-agent` in
  the same context.
- The agent will create a new confined context to run the specified command in
  (`date` in this example).
- The agent will then execute the command (`date` in this example) inside this
  new context, first setting the root filesystem to the expected Ubuntu\* root
  filesystem.

##### Initrd image

placeholder

## Agent

[`kata-agent`](https://github.com/kata-containers/agent) is a process running in the
guest as a supervisor for managing containers and processes running within
those containers.

The `kata-agent` execution unit is the sandbox. A `kata-agent` sandbox is a container sandbox defined by a set of namespaces (NS, UTS, IPC and PID). `kata-runtime` can
run several containers per VM to support container engines that require multiple
containers running inside a pod. In the case of docker, `kata-runtime` creates a
single container per pod.

`kata-agent` communicates with the other Kata components over gRPC.
It also runs a [`yamux`](https://github.com/hashicorp/yamux) server on the same gRPC URL.

The `kata-agent` makes use of [`libcontainer`](https://github.com/opencontainers/runc/tree/master/libcontainer)
to manage the lifecycle of the container. This way the `kata-agent` reuses most
of the code used by [`runc`](https://github.com/opencontainers/runc).

### Agent gRPC protocol

placeholder

## Runtime

`kata-runtime` is an OCI compatible container runtime and is responsible for handling
all commands specified by
[the OCI runtime specification](https://github.com/opencontainers/runtime-spec)
and launching `kata-shim` instances.

`kata-runtime` heavily utilizes the
[virtcontainers project](https://github.com/containers/virtcontainers), which
provides a generic, runtime-specification agnostic, hardware-virtualized containers
library.

### Configuration

The runtime uses a TOML format configuration file called `configuration.toml`. By
default this file is installed in the `/usr/share/defaults/kata-containers`
directory and contains various settings such as the paths to the hypervisor,
the guest kernel and the mini-OS image.

Most users will not need to modify the configuration file.

The file is well commented and provides a few "knobs" that can be used to modify
the behavior of the runtime.

The configuration file is also used to enable runtime [debug output](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md#enable-full-debug).

### Significant OCI commands

Here we describe how `kata-runtime` handles the most important OCI commands.

#### `create`

When handling the OCI
[`create`](https://github.com/kata-containers/runtime/blob/master/cli/create.go)
command, `kata-runtime` goes through the following steps:

1. Create the network namespace where we will spawn VM and shims processes.
2. Call into the pre-start hooks. One of them should be responsible for creating
the `veth` network pair between the host network namespace and the network namespace
freshly created.
3. Scan the network from the new network namespace, and create a MACVTAP connection
 between the `veth` interface and a `tap` interface into the VM.
4. Start the VM inside the network namespace by providing the `tap` interface
 previously created.
5. Wait for the VM to be ready.
6. Start `kata-proxy`, which will connect to the created VM. The `kata-proxy` process
will take care of proxying all communications with the VM. Kata has a single proxy
per VM.
7. Communicate with `kata-agent` (through the proxy) to configure the sandbox
 inside the VM.
8. Communicate with `kata-agent` to create the container, relying on the OCI
configuration file `config.json` initially provided to `kata-runtime`. This
spawns the container process inside the VM, leveraging the `libcontainer` package.
9. Start `kata-shim`, which will connect to the gRPC server socket provided by the `kata-proxy`. `kata-shim`  will spawn a few Go routines to parallelize blocking calls `ReadStdout()` , `ReadStderr()` and `WaitProcess()`. Both `ReadStdout()` and `ReadStderr()` are run through infinite loops since `kata-shim` wants the output of those until the container process terminates. `WaitProcess()` is a unique call which returns the exit code of the container process when it terminates inside the VM. Note that `kata-shim` is started inside the network namespace, to allow upper layers to determine which network namespace has been created and by checking the `kata-shim` process. It also creates a new PID namespace by entering into it. This ensures that all `kata-shim` processes belonging to the same container will get killed when the `kata-shim` representing the container process terminates.

At this point the container process is running inside of the VM, and it is represented
on the host system by the `kata-shim` process.

![`kata-oci-create`](arch-images/kata-oci-create.svg)

#### `start`

With traditional containers, [`start`](https://github.com/kata-containers/runtime/blob/master/cli/start.go) launches a container process in its own set of namespaces. With Kata Containers, the main task of `kata-runtime` is to ask [`kata-agent`](#agent) to start the container workload inside the virtual machine. `kata-runtime` will run through the following steps:

1. Communicate with `kata-agent` (through the proxy) to start the container workload
 inside the VM. If, for example, the command to execute inside of the container is `top`,
 the `kata-shim`'s `ReadStdOut()` will start returning text output for top, and
  `WaitProcess()` will continue to block as long as the `top` process runs.
2. Call into the post-start hooks. Usually, this is a no-op since nothing is provided
  (this needs clarification)

![`kata-oci-start`](arch-images/kata-oci-start.svg)

#### `exec`

OCI [`exec`](https://github.com/kata-containers/runtime/blob/master/cli/exec.go) allows you to run an additional command within an already running
container.  In Kata Containers, this is handled as follows:

1. A request is sent to the `kata agent` (through the proxy) to start a new process
 inside an existing container running within the VM.
2. A new `kata-shim` is created within the same network and PID namespaces as the
 original `kata-shim` representing the container process. This new `kata-shim` is
 used for the new exec process.

Now the process started with `exec` is running within the VM, sharing `uts`, `pid`, `mnt` and `ipc` namespaces with the container process.

![`kata-oci-exec`](arch-images/kata-oci-exec.svg)

#### `kill`

When sending the OCI [`kill`](https://github.com/kata-containers/runtime/blob/master/cli/kill.go) command, the container runtime should send a
[UNIX signal](https://en.wikipedia.org/wiki/Unix_signal) to the container process.
A `kill` sending a termination signal such as `SIGKILL` or `SIGTERM` is expected
to terminate the container process.  In the context of a traditional container,
this means stopping the container.  For `kata-runtime`, this translates to stopping
the container and the VM associated with it.

1. Send a request to kill the container process to the `kata-agent` (through the proxy).
2. Wait for `kata-shim` process to exit.
3. Force kill the container process if `kata-shim` process didn't return after a
 timeout. This is done by communicating with `kata-agent` (connecting the proxy),
 sending `SIGKILL` signal to the container process inside the VM.
4. Wait for `kata-shim` process to exit, and return an error if we reach the
 timeout again.
5. Communicate with `kata-agent` (through the proxy) to remove the container
 configuration from the VM.
6. Communicate with `kata-agent` (through the proxy) to destroy the sandbox
 configuration from the VM.
7. Stop the VM.
8. Remove all network configurations inside the network namespace and delete the
 namespace.
9. Execute post-stop hooks.

If `kill` was invoked with a non-termination signal, this simply signals the container process. Otherwise, everything has been torn down, and the VM has been removed.

#### `delete`

[`delete`](https://github.com/kata-containers/runtime/blob/master/cli/delete.go) removes all internal resources related to a container. A running container
cannot be deleted unless the OCI runtime is explicitly being asked to, by using
`--force` flag.

If the sandbox is not stopped, but the particular container process returned on
its own already, the `kata-runtime` will first go through most of the steps a `kill`
would go through for a termination signal. After this process, or if the `sandboxID` was already stopped to begin with, then `kata-runtime` will:

1. Remove container resources. Every file kept under `/var/{lib,run}/virtcontainers/sandboxes/<sandboxID>/<containerID>`.
2. Remove sandbox resources. Every file kept under `/var/{lib,run}/virtcontainers/sandboxes/<sandboxID>`.

At this point, everything related to the container should have been removed from the host system, and no related process should be running.

#### `state`

[`state`](https://github.com/kata-containers/runtime/blob/master/cli/state.go)
returns the status of the container. For `kata-runtime`, this means being
able to detect if the container is still running by looking at the state of `kata-shim`
process representing this container process.

1. Ask the container status by checking information stored on disk. (clarification needed)
2. Check `kata-shim` process representing the container.
3. In case the container status on disk was supposed to be `ready` or `running`,
 and the `kata-shim` process no longer exists, this involves the detection of a
 stopped container. This means that before returning the container status,
 the container has to be properly stopped. Here are the steps involved in this detection:
	1. Wait for `kata-shim` process to exit.
	2. Force kill the container process if `kata-shim` process didn't return after a timeout. This is done by communicating with `kata-agent` (connecting the proxy), sending `SIGKILL` signal to the container process inside the VM.
	3. Wait for `kata-shim` process to exit, and return an error if we reach the timeout again.
	4. Communicate with `kata-agent` (connecting the proxy) to remove the container configuration from the VM.
4. Return container status.

## Proxy

Communication with the VM can be achieved by either `virtio-serial` or, if the host
kernel is newer than v4.8, a virtual socket, `vsock` can be used. The default is `virtio-serial`.

The VM will likely be running multiple container processes.  In the event `virtio-serial`
is used, the I/O streams associated with each process needs to be multiplexed and demultiplexed on the host. On systems with `vsock` support, this component becomes optional.

`kata-proxy` is a process offering access to the VM [`kata-agent`](https://github.com/kata-containers/agent)
to multiple `kata-shim` and `kata-runtime` clients associated with the VM. Its
main role is to route the I/O streams and signals between each `kata-shim`
instance and the `kata-agent`.
`kata-proxy` connects to `kata-agent` on a Unix domain socket that `kata-runtime` provides
while spawning `kata-proxy`.
`kata-proxy` uses [`yamux`](https://github.com/hashicorp/yamux) to multiplex gRPC
requests on its connection to the `kata-agent`.

When proxy type is configured as `proxyBuiltIn`, we do not spawn a separate
process to proxy gRPC connections. Instead a built-in Yamux gRPC dialer is used to connect
directly to `kata-agent`. This is used by CRI container runtime server `frakti` which
calls directly into `kata-runtime`.

## Shim

A container process reaper, such as Docker's `containerd-shim` or CRI-O's `conmon`,
is designed around the assumption that it can monitor and reap the actual container
process. As the container process reaper runs on the host, it cannot directly
monitor a process running within a virtual machine. At most it can see the QEMU
process, but that is not enough. With Kata Containers, `kata-shim` acts as the
container process that the container process reaper can monitor. Therefore
`kata-shim` needs to handle all container I/O streams (`stdout`, `stdin` and `stderr`)
and forward all signals the container process reaper decides to send to the container
process.

`kata-shim` has an implicit knowledge about which VM agent will handle those streams
and signals and thus acts as an encapsulation layer between the container process
reaper and the `kata-agent`. `kata-shim`:

- Connects to `kata-proxy` on a Unix domain socket. The socket URL is passed from
  `kata-runtime` to `kata-shim` when the former spawns the latter along with a
  `containerID` and `execID`. The `containerID` and `execID` are used to identify
  the true container process that the shim process will be shadowing or representing.
- Forwards the standard input stream from the container process reaper into
 `kata-proxy` using gRPC `WriteStdin` gRPC API.
- Reads the standard output/error from the container process.
- Forwards signals it receives from the container process reaper to `kata-proxy`
  using `SignalProcessRequest` API.
- Monitors terminal changes and forwards them to `kata-proxy` using gRPC `TtyWinResize`
  API.


## Networking

Containers will typically live in their own, possibly shared, networking namespace.
At some point in a container lifecycle, container engines will set up that namespace
to add the container to a network which is isolated from the host network, but
which is shared between containers

In order to do so, container engines will usually add one end of a virtual
ethernet (`veth`) pair into the container networking namespace. The other end of
the `veth` pair is added to the host networking namespace.

This is a very namespace-centric approach as many hypervisors (in particular QEMU)
cannot handle `veth` interfaces. Typically, `TAP` interfaces are created for VM
connectivity.

To overcome incompatibility between typical container engines expectations
and virtual machines, `kata-runtime` networking transparently connects `veth`
interfaces with `TAP` ones using MACVTAP:

![Kata Containers networking](arch-images/network.png)

 Kata Containers supports both
[CNM](https://github.com/docker/libnetwork/blob/master/docs/design.md#the-container-network-model)
and [CNI](https://github.com/containernetworking/cni) for networking management.

### CNM

![High-level CNM Diagram](arch-images/CNM_overall_diagram.png)

__CNM lifecycle__

1.  `RequestPool`

2.  `CreateNetwork`

3.  `RequestAddress`

4.  `CreateEndPoint`

5.  `CreateContainer`

6.  Create `config.json`

7.  Create PID and network namespace

8.  `ProcessExternalKey`

9.  `JoinEndPoint`

10. `LaunchContainer`

11. Launch

12. Run container

![Detailed CNM Diagram](arch-images/CNM_detailed_diagram.png)

__Runtime network setup with CNM__

1. Read `config.json`

2. Create the network namespace

3. Call the `prestart` hook (from inside the netns)

4. Scan network interfaces inside netns and get the name of the interface
  created by prestart hook

5. Create bridge, TAP, and link all together with network interface previously
  created

### Network Hotplug

Kata Containers has developed a set of network sub-commands and APIs to add, list and
remove a guest network endpoint and to manipulate the guest route table.

The following diagram illustrates the Kata Containers network hotplug workflow.

![Network Hotplug](arch-images/kata-containers-network-hotplug.png)

## Storage
Container workloads are shared with the virtualized environment through [9pfs](https://www.kernel.org/doc/Documentation/filesystems/9p.txt).
The devicemapper storage driver is a special case. The driver uses dedicated block
devices rather than formatted filesystems, and operates at the block level rather
than the file level. This knowledge is used to directly use the underlying block
device instead of the overlay file system for the container root file system. The
block device maps to the top read-write layer for the overlay. This approach gives
much better I/O performance compared to using 9pfs to share the container file system.

The approach above does introduce a limitation in terms of dynamic file copy
in/out of the container using the `docker cp` operations. The copy operation from
host to container accesses the mounted file system on the host-side. This is
not expected to work and may lead to inconsistencies as the block device will
be simultaneously written to from two different mounts. The copy operation from
container to host will work, provided the user calls `sync(1)` from within the
container prior to the copy to make sure any outstanding cached data is written
to the block device.

```
docker cp [OPTIONS] CONTAINER:SRC_PATH HOST:DEST_PATH
docker cp [OPTIONS] HOST:SRC_PATH CONTAINER:DEST_PATH
```

Kata Containers has the ability to hotplug and remove block devices, which makes it
possible to use block devices for containers started after the VM has been launched.

Users can check to see if the container uses the devicemapper block device as its
rootfs by calling `mount(8)` within the container.  If the devicemapper block device
is used, `/` will be mounted on `/dev/vda`.  Users can disable direct mounting
of the underlying block device through the runtime configuration.

## Kubernetes support

[Kubernetes\*](https://github.com/kubernetes/kubernetes/) is a popular open source
container orchestration engine. In Kubernetes, a set of containers sharing resources
such as networking, storage, mount, PID, etc. is called a
[Pod](https://kubernetes.io/docs/user-guide/pods/).
A node can have multiple pods, but at a minimum, a node within a Kubernetes cluster
only needs to run a container runtime and a container agent (called a
[Kubelet](https://kubernetes.io/docs/admin/kubelet/)).

A Kubernetes cluster runs a control plane where a scheduler (typically running on a
dedicated master node) calls into a compute Kubelet. This Kubelet instance is
responsible for managing the lifecycle of pods within the nodes and eventually relies
on a container runtime to handle execution. The Kubelet architecture decouples
lifecycle management from container execution through the dedicated
`gRPC` based [Container Runtime Interface (CRI)](https://github.com/kubernetes/community/blob/master/contributors/design-proposals/node/container-runtime-interface-v1.md).

In other words, a Kubelet is a CRI client and expects a CRI implementation to
handle the server side of the interface.
[CRI-O\*](https://github.com/kubernetes-incubator/cri-o) and [Containerd CRI Plugin\*](https://github.com/containerd/cri) are CRI implementations that rely on [OCI](https://github.com/opencontainers/runtime-spec)
compatible runtimes for managing container instances.

Kata Containers is an officially supported CRI-O and Containerd CRI Plugin runtime. It is OCI compatible and therefore aligns with project's architecture and requirements.
However, due to the fact that Kubernetes execution units are sets of containers (also
known as pods) rather than single containers, the Kata Containers runtime needs to
get extra information to seamlessly integrate with Kubernetes.

### Problem statement

The Kubernetes\* execution unit is a pod that has specifications detailing constraints
such as namespaces, groups, hardware resources, security contents, *etc* shared by all
the containers within that pod.
By default the Kubelet will send a container creation request to its CRI runtime for
each pod and container creation. Without additional metadata from the CRI runtime,
the Kata Containers runtime will thus create one virtual machine for each pod and for
each containers within a pod. However the task of providing the Kubernetes pod semantics
when creating one virtual machine for each container within the same pod is complex given
the resources of these virtual machines (such as networking or PID) need to be shared.

The challenge with Kata Containers when working as a Kubernetes\* runtime is thus to know
when to create a full virtual machine (for pods) and when to create a new container inside
a previously created virtual machine. In both cases it will get called with very similar
arguments, so it needs the help of the Kubernetes CRI runtime to be able to distinguish a
pod creation request from a container one.

### Containerd

As of Kata Containers 1.5, using `shimv2` with containerd 1.2.0 or above is the preferred
way to run Kata Containers with Kubernetes ([see the howto](https://github.com/kata-containers/documentation/blob/master/how-to/how-to-use-k8s-with-cri-containerd-and-kata.md#configure-containerd-to-use-kata-containers)).
The CRI-O will catch up soon ([`kubernetes-sigs/cri-o#2024`](https://github.com/kubernetes-sigs/cri-o/issues/2024)).

Refer to the following how-to guides:

- [How to use Kata Containers and Containerd](/how-to/containerd-kata.md)
- [How to use Kata Containers and CRI (containerd plugin) with Kubernetes](/how-to/how-to-use-k8s-with-cri-containerd-and-kata.md)

### CRI-O

####  OCI annotations

In order for the Kata Containers runtime (or any virtual machine  based OCI compatible
runtime) to be able to understand if it needs to create a full virtual machine or if it
has to create a new container inside an existing pod's virtual machine, CRI-O adds
specific annotations to the OCI configuration file (`config.json`) which is passed to
the OCI compatible runtime.

Before calling its runtime, CRI-O will always add a `io.kubernetes.cri-o.ContainerType`
annotation to the `config.json` configuration file it produces from the Kubelet CRI
request. The `io.kubernetes.cri-o.ContainerType` annotation can either be set to `sandbox`
or `container`. Kata Containers will then use this annotation to decide if it needs to
respectively create a virtual machine or a container inside a virtual machine associated
with a Kubernetes pod:

```Go
	containerType, err := ociSpec.ContainerType()
	if err != nil {
		return err
	}

	handleFactory(ctx, runtimeConfig)

	disableOutput := noNeedForOutput(detach, ociSpec.Process.Terminal)

	var process vc.Process
	switch containerType {
	case vc.PodSandbox:
		process, err = createSandbox(ctx, ociSpec, runtimeConfig, containerID, bundlePath, console, disableOutput, systemdCgroup)
		if err != nil {
			return err
		}
	case vc.PodContainer:
		process, err = createContainer(ctx, ociSpec, containerID, bundlePath, console, disableOutput)
		if err != nil {
			return err
		}
	}

```

#### Mixing VM based and namespace based runtimes

> **Note:** Since Kubernetes 1.12, the [`Kubernetes RuntimeClass`](/how-to/containerd-kata.md#kubernetes-runtimeclass)
> has been supported and the user can specify runtime without the non-standardized annotations.

One interesting evolution of the CRI-O support for `kata-runtime` is the ability
to run virtual machine based pods alongside namespace ones. With CRI-O and Kata
Containers, one can introduce the concept of workload trust inside a Kubernetes
cluster.

A cluster operator can now tag (through Kubernetes annotations) container workloads
as `trusted` or `untrusted`. The former labels known to be safe workloads while
the latter describes potentially malicious or misbehaving workloads that need the
highest degree of isolation. In a software development context, an example of a `trusted` workload would be a containerized continuous integration engine whereas all
developers applications would be `untrusted` by default. Developers workloads can
be buggy, unstable or even include malicious code and thus from a security perspective
it makes sense to tag them as `untrusted`. A CRI-O and Kata Containers based
Kubernetes cluster handles this use case transparently as long as the deployed
containers are properly tagged. All `untrusted` containers will be handled by Kata Containers and thus run in a hardware virtualized secure sandbox while `runc`, for
example, could  handle the `trusted` ones.

CRI-O's default behavior is to trust all pods, except when they're annotated with
`io.kubernetes.cri-o.TrustedSandbox` set to `false`. The default CRI-O trust level
is set through its `configuration.toml` configuration file. Generally speaking,
the CRI-O runtime selection between its trusted runtime (typically `runc`) and its untrusted one (`kata-runtime`) is a function of the pod `Privileged` setting, the `io.kubernetes.cri-o.TrustedSandbox` annotation value, and the default CRI-O trust
level. When a pod is `Privileged`, the runtime will always be `runc`. However, when
a pod is **not** `Privileged` the runtime selection is done as follows:

|                                        | `io.kubernetes.cri-o.TrustedSandbox` not set   | `io.kubernetes.cri-o.TrustedSandbox` = `true` | `io.kubernetes.cri-o.TrustedSandbox` = `false` |
| :---                                   |     :---:                                      |     :---:                                     |     :---:                                             |
| Default CRI-O trust level: `trusted`   | runc                                           | runc                                          | Kata Containers |
| Default CRI-O trust level: `untrusted` | Kata Containers                               | Kata Containers                              |  Kata Containers |


# Appendices

## DAX

Kata Containers utilizes the Linux kernel DAX [(Direct Access filesystem)](https://git.kernel.org/cgit/linux/kernel/git/torvalds/linux.git/tree/Documentation/filesystems/dax.txt)
feature to efficiently map some host-side files into the guest VM space.
In particular, Kata Containers uses the QEMU NVDIMM feature to provide a
memory-mapped virtual device that can be used to DAX map the virtual machine's
root filesystem into the guest memory address space.

Mapping files using DAX provides a number of benefits over more traditional VM
file and device mapping mechanisms:

- Mapping as a direct access devices allows the guest to directly access
  the host memory pages (such as via Execute In Place (XIP)), bypassing the guest
  page cache. This provides both time and space optimizations.
- Mapping as a direct access device inside the VM allows pages from the
  host to be demand loaded using page faults, rather than having to make requests
  via a virtualized device (causing expensive VM exits/hypercalls), thus providing
  a speed optimization.
- Utilizing `MAP_SHARED` shared memory on the host allows the host to efficiently
  share pages.

Kata Containers uses the following steps to set up the DAX mappings:
1. QEMU is configured with an NVDIMM memory device, with a memory file
  backend to map in the host-side file into the virtual NVDIMM space.
2. The guest kernel command line mounts this NVDIMM device with the DAX
  feature enabled, allowing direct page mapping and access, thus bypassing the
  guest page cache.

![DAX](arch-images/DAX.png)

Information on the use of NVDIMM via QEMU is available in the [QEMU source code](http://git.qemu-project.org/?p=qemu.git;a=blob;f=docs/nvdimm.txt;hb=HEAD)
