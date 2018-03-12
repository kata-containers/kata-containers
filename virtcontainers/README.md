[![Build Status](https://travis-ci.org/containers/virtcontainers.svg?branch=master)](https://travis-ci.org/containers/virtcontainers)
[![Build Status](http://cc-jenkins-ci.westus2.cloudapp.azure.com/job/virtcontainers-ubuntu-16-04-master/badge/icon)](http://cc-jenkins-ci.westus2.cloudapp.azure.com/job/virtcontainers-ubuntu-16-04-master)
[![Build Status](http://cc-jenkins-ci.westus2.cloudapp.azure.com/job/virtcontainers-ubuntu-17-04-master/badge/icon)](http://cc-jenkins-ci.westus2.cloudapp.azure.com/job/virtcontainers-ubuntu-17-04-master)
[![Build Status](http://cc-jenkins-ci.westus2.cloudapp.azure.com/job/virtcontainers-fedora-26-master/badge/icon)](http://cc-jenkins-ci.westus2.cloudapp.azure.com/job/virtcontainers-fedora-26-master)
[![Go Report Card](https://goreportcard.com/badge/github.com/containers/virtcontainers)](https://goreportcard.com/report/github.com/containers/virtcontainers)
[![Coverage Status](https://coveralls.io/repos/github/containers/virtcontainers/badge.svg?branch=master)](https://coveralls.io/github/containers/virtcontainers?branch=master)
[![GoDoc](https://godoc.org/github.com/containers/virtcontainers?status.svg)](https://godoc.org/github.com/containers/virtcontainers)

Table of Contents
=================

   * [What is it ?](#what-is-it-)
   * [Background](#background)
   * [Out of scope](#out-of-scope)
      * [virtcontainers and Kubernetes CRI](#virtcontainers-and-kubernetes-cri)
   * [Design](#design)
      * [Pods](#pods)
      * [Hypervisors](#hypervisors)
      * [Agents](#agents)
      * [Shim](#shim)
      * [Proxy](#proxy)
   * [API](#api)
      * [Pod API](#pod-api)
      * [Container API](#container-api)
   * [Networking](#networking)
      * [CNM](#cnm)
      * [CNI](#cni)
   * [Storage](#storage)
      * [How to check if container uses devicemapper block device as its rootfs](#how-to-check-if-container-uses-devicemapper-block-device-as-its-rootfs)
   * [Devices](#devices)
      * [How to pass a device using VFIO-passthrough](#how-to-pass-a-device-using-vfio-passthrough)
   * [Developers](#developers)

# What is it ?

`virtcontainers` is a Go library that can be used to build hardware-virtualized container
runtimes.

# Background

The few existing VM-based container runtimes (Clear Containers, runv, rkt's
kvm stage 1) all share the same hardware virtualization semantics but use different
code bases to implement them. `virtcontainers`'s goal is to factorize this code into
a common Go library.

Ideally, VM-based container runtime implementations would become translation
layers from the runtime specification they implement (e.g. the [OCI runtime-spec][oci]
or the [Kubernetes CRI][cri]) to the `virtcontainers` API.

`virtcontainers` is [Clear Containers][cc]'s runtime foundational package for their
[runtime][cc-runtime] implementation

[oci]: https://github.com/opencontainers/runtime-spec
[cri]: https://github.com/kubernetes/kubernetes/blob/master/docs/proposals/container-runtime-interface-v1.md
[cc]: https://github.com/clearcontainers/
[cc-runtime]: https://github.com/clearcontainers/runtime/

# Out of scope

Implementing a container runtime is out of scope for this project. Any
tools or executables in this repository are only provided for demonstration or
testing purposes.

## virtcontainers and Kubernetes CRI

`virtcontainers`'s API is loosely inspired by the Kubernetes [CRI][cri] because
we believe it provides the right level of abstractions for containerized pods.
However, despite the API similarities between the two projects, the goal of
`virtcontainers` is _not_ to build a CRI implementation, but instead to provide a
generic, runtime-specification agnostic, hardware-virtualized containers
library that other projects could leverage to implement CRI themselves.

# Design

## Pods

The `virtcontainers` execution unit is a _pod_, i.e. `virtcontainers` users start pods where
containers will be running.

`virtcontainers` creates a pod by starting a virtual machine and setting the pod
up within that environment. Starting a pod means launching all containers with
the VM pod runtime environment.

## Hypervisors

The `virtcontainers` package relies on hypervisors to start and stop virtual machine where
pods will be running. An hypervisor is defined by an Hypervisor interface implementation,
and the default implementation is the QEMU one.

## Agents

During the lifecycle of a container, the runtime running on the host needs to interact with
the virtual machine guest OS in order to start new commands to be executed as part of a given
container workload, set new networking routes or interfaces, fetch a container standard or
error output, and so on.
There are many existing and potential solutions to resolve that problem and `virtcontainers` abstracts
this through the Agent interface.

## Shim

In some cases the runtime will need a translation shim between the higher level container
stack (e.g. Docker) and the virtual machine holding the container workload. This is needed
for container stacks that make strong assumptions on the nature of the container they're
monitoring. In cases where they assume containers are simply regular host processes, a shim
layer is needed to translate host specific semantics into e.g. agent controlled virtual
machine ones.

## Proxy

When hardware virtualized containers have limited I/O multiplexing capabilities,
runtimes may decide to rely on an external host proxy to support cases where several
runtime instances are talking to the same container.

# API

The high level `virtcontainers` API is the following one:

## Pod API

* `CreatePod(podConfig PodConfig)` creates a Pod.
The virtual machine is started and the Pod is prepared.

* `DeletePod(podID string)` deletes a Pod.
The virtual machine is shut down and all information related to the Pod are removed.
The function will fail if the Pod is running. In that case `StopPod()` has to be called first.

* `StartPod(podID string)` starts an already created Pod.
The Pod and all its containers are started.

* `RunPod(podConfig PodConfig)` creates and starts a Pod.
This performs `CreatePod()` + `StartPod()`.

* `StopPod(podID string)` stops an already running Pod.
The Pod and all its containers are stopped.

* `PausePod(podID string)` pauses an existing Pod.

* `ResumePod(podID string)` resume a paused Pod.

* `StatusPod(podID string)` returns a detailed Pod status.

* `ListPod()` lists all Pods on the host.
It returns a detailed status for every Pod.

## Container API

* `CreateContainer(podID string, containerConfig ContainerConfig)` creates a Container on an existing Pod.

* `DeleteContainer(podID, containerID string)` deletes a Container from a Pod.
If the Container is running it has to be stopped first.

* `StartContainer(podID, containerID string)` starts an already created Container.
The Pod has to be running.

* `StopContainer(podID, containerID string)` stops an already running Container.

* `EnterContainer(podID, containerID string, cmd Cmd)` enters an already running Container and runs a given command.

* `StatusContainer(podID, containerID string)` returns a detailed Container status.

* `KillContainer(podID, containerID string, signal syscall.Signal, all bool)` sends a signal to all or one container inside a Pod.

An example tool using the `virtcontainers` API is provided in the `hack/virtc` package.

# Networking

`virtcontainers` supports the 2 major container networking models: the [Container Network Model (CNM)][cnm] and the [Container Network Interface (CNI)][cni].

Typically the former is the Docker default networking model while the later is used on Kubernetes deployments.

`virtcontainers` callers can select one or the other, on a per pod basis, by setting their `PodConfig`'s `NetworkModel` field properly.

[cnm]: https://github.com/docker/libnetwork/blob/master/docs/design.md
[cni]: https://github.com/containernetworking/cni/

## CNM

![High-level CNM Diagram](documentation/network/CNM_overall_diagram.png)

__CNM lifecycle__

1.  RequestPool

2.  CreateNetwork

3.  RequestAddress

4.  CreateEndPoint

5.  CreateContainer

6.  Create config.json

7.  Create PID and network namespace

8.  ProcessExternalKey

9.  JoinEndPoint

10. LaunchContainer

11. Launch

12. Run container

![Detailed CNM Diagram](documentation/network/CNM_detailed_diagram.png)

__Runtime network setup with CNM__

1. Read config.json

2. Create the network namespace ([code](https://github.com/containers/virtcontainers/blob/0.5.0/cnm.go#L108-L120))

3. Call the prestart hook (from inside the netns) ([code](https://github.com/containers/virtcontainers/blob/0.5.0/api.go#L46-L49))

4. Scan network interfaces inside netns and get the name of the interface created by prestart hook ([code](https://github.com/containers/virtcontainers/blob/0.5.0/cnm.go#L70-L106))

5. Create bridge, TAP, and link all together with network interface previously created ([code](https://github.com/containers/virtcontainers/blob/0.5.0/network.go#L123-L205))

6. Start VM inside the netns and start the container ([code](https://github.com/containers/virtcontainers/blob/0.5.0/api.go#L66-L70))

__Drawbacks of CNM__

There are three drawbacks about using CNM instead of CNI:
* The way we call into it is not very explicit: Have to re-exec dockerd binary so that it can accept parameters and execute the prestart hook related to network setup.
* Implicit way to designate the network namespace: Instead of explicitely giving the netns to dockerd, we give it the PID of our runtime so that it can find the netns from this PID. This means we have to make sure being in the right netns while calling the hook, otherwise the veth pair will be created with the wrong netns.
* No results are back from the hook: We have to scan the network interfaces to discover which one has been created inside the netns. This introduces more latency in the code because it forces us to scan the network in the CreatePod path, which is critical for starting the VM as quick as possible.


## CNI

![CNI Diagram](documentation/network/CNI_diagram.png)

__Runtime network setup with CNI__

1. Create the network namespace ([code](https://github.com/containers/virtcontainers/blob/0.5.0/cni.go#L64-L76))

2. Get CNI plugin information ([code](https://github.com/containers/virtcontainers/blob/0.5.0/cni.go#L29-L32))

3. Start the plugin (providing previously created netns) to add a network described into /etc/cni/net.d/ directory. At that time, the CNI plugin will create the cni0 network interface and a veth pair between the host and the created netns. It links cni0 to the veth pair before to exit. ([code](https://github.com/containers/virtcontainers/blob/0.5.0/cni.go#L34-L45))

4. Create bridge, TAP, and link all together with network interface previously created ([code](https://github.com/containers/virtcontainers/blob/0.5.0/network.go#L123-L205))

5. Start VM inside the netns and start the container ([code](https://github.com/containers/virtcontainers/blob/0.5.0/api.go#L66-L70))

# Storage

Container workloads are shared with the virtualized environment through 9pfs.
The devicemapper storage driver is a special case. The driver uses dedicated block devices rather than formatted filesystems, and operates at the block level rather than the file level. This knowledge has been used to directly use the underlying block device instead of the overlay file system for the container root file system. The block device maps to the top read-write layer for the overlay. This approach gives much better I/O performance compared to using 9pfs to share the container file system.

The approach above does introduce a limitation in terms of dynamic file copy in/out of the container via `docker cp` operations.
The copy operation from host to container accesses the mounted file system on the host side. This is not expected to work and may lead to inconsistencies as the block device will be simultaneously written to, from two different mounts.
The copy operation from container to host will work, provided the user calls `sync(1)` from within the container prior to the copy to make sure any outstanding cached data is written to the block device.

```
docker cp [OPTIONS] CONTAINER:SRC_PATH HOST:DEST_PATH
docker cp [OPTIONS] HOST:SRC_PATH CONTAINER:DEST_PATH
```

Ability to hotplug block devices has been added, which makes it possible to use block devices for containers started after the VM has been launched.

## How to check if container uses devicemapper block device as its rootfs

Start a container. Call mount(8) within the container. You should see '/' mounted on /dev/vda device.

# Devices

Support has been added to pass [VFIO](https://www.kernel.org/doc/Documentation/vfio.txt) 
assigned devices on the docker command line with --device.
Support for passing other devices including block devices with --device has
not been added added yet.

## How to pass a device using VFIO-passthrough

1. Requirements

IOMMU group represents the smallest set of devices for which the IOMMU has
visibility and which is isolated from other groups.  VFIO uses this information
to enforce safe ownership of devices for userspace. 

You will need Intel VT-d capable hardware. Check if IOMMU is enabled in your host
kernel by verifying `CONFIG_VFIO_NOIOMMU` is not in the kernel config. If it is set,
you will need to rebuild your kernel.

The following kernel configs need to be enabled:
```
CONFIG_VFIO_IOMMU_TYPE1=m 
CONFIG_VFIO=m
CONFIG_VFIO_PCI=m
```

In addition, you need to pass `intel_iommu=on` on the kernel command line.

2. Identify BDF(Bus-Device-Function) of the PCI device to be assigned.


```
$ lspci -D | grep -e Ethernet -e Network
0000:01:00.0 Ethernet controller: Intel Corporation Ethernet Controller 10-Gigabit X540-AT2 (rev 01)

$ BDF=0000:01:00.0
```

3. Find vendor and device id.

```
$ lspci -n -s $BDF
01:00.0 0200: 8086:1528 (rev 01)
```

4. Find IOMMU group.

```
$ readlink /sys/bus/pci/devices/$BDF/iommu_group
../../../../kernel/iommu_groups/16
```

5. Unbind the device from host driver.

```
$ echo $BDF | sudo tee /sys/bus/pci/devices/$BDF/driver/unbind
```

6. Bind the device to vfio-pci.

```
$ sudo modprobe vfio-pci
$ echo 8086 1528 | sudo tee /sys/bus/pci/drivers/vfio-pci/new_id
$ echo $BDF | sudo tee --append /sys/bus/pci/drivers/vfio-pci/bind
```

7. Check /dev/vfio

```
$ ls /dev/vfio
16 vfio
```

8. Start a Clear Containers container passing the VFIO group on the docker command line.

```
docker run -it --device=/dev/vfio/16 centos/tools bash
```

9. Running `lspci` within the container should show the device among the 
PCI devices. The driver for the device needs to be present within the
Clear Containers kernel. If the driver is missing,  you can add it to your
custom container kernel using the [osbuilder](https://github.com/clearcontainers/osbuilder)
tooling.

# Developers

For information on how to build, develop and test `virtcontainers`, see the
[developer documentation](documentation/Developers.md).
