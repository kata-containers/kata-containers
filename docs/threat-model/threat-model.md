# Kata Containers threat model

This document discusses threat models associated with the Kata Containers project.
Kata was designed to provide additional isolation of container workloads, protecting
the host infrastructure from potentially malicious container users or workloads. Since
Kata Containers adds a level of isolation on top of traditional containers, the focus
is on the additional layer provided, not on traditional container security.

This document provides a brief background on containers and layered security, describes
the interface to Kata from CRI runtimes, a review of utilized virtual machine interfaces, and then
a review of threats.

## Kata security objective

Kata seeks to prevent an untrusted container workload or user of that container workload to gain
control of, obtain information from, or tamper with the host infrastructure.

In our scenario, an asset is anything on the host system, or elsewhere in the cluster
infrastructure. The attacker is assumed to be either a malicious user or the workload itself
running within the container. The goal of Kata is to prevent attacks which would allow
any access to the defined assets.

## Background on containers, layered security

Traditional containers leverage several key Linux kernel features to provide isolation and
a view that the container workload is the only entity running on the host. Key features include
`Namespaces`, `cgroups`, `capablities`, `SELinux` and `seccomp`. The canonical runtime for creating such
a container is `runc`. In the remainder of the document, the term `traditional-container` will be used
to describe a container workload created by runc.

Kata Containers provides a second layer of isolation on top of those provided by traditional-containers.
The hardware virtualization interface is the basis of this additional layer. Kata launches a lightweight
virtual machine, and uses the guest’s Linux kernel to create a container workload, or workloads in the case
of multi-container pods. In Kubernetes and in the Kata implementation, the sandbox is carried out at the
pod level. In Kata, this sandbox is created using a virtual machine.

## Interface to Kata Containers: CRI, v2-shim, OCI

A typical Kata Containers deployment uses Kubernetes with a CRI implementation.
On every node, Kubelet will interact with a CRI implementor, which will in turn interface with
an OCI based runtime, such as Kata Containers. Typical CRI implementors are `cri-o` and `containerd`.

The CRI API, as defined at the Kubernetes [CRI-API repo](https://github.com/kubernetes/cri-api/),
results in a few constructs being supported by the CRI implementation, and ultimately in the OCI
runtime creating the workloads.

In order to run a container inside of the Kata sandbox, several virtual machine devices and interfaces
are required. Kata translates sandbox and container definitions to underlying virtualization technologies provided
by a set of virtual machine monitors (VMMs) and hypervisors. These devices and their underlying
implementations are discussed in detail in the following section.

## Interface to the Kata sandbox/virtual machine

In case of Kata, today the devices which we need in the guest are:
 - Storage: In the current design of Kata Containers, we are reliant on the CRI implementor to
 assist in image handling and volume management on the host. As a result, we need to support a way of passing to the sandbox the container rootfs, volumes requested
 by the workload, and any other volumes created to facilitate sharing of secrets and `configmaps` with the containers. Depending on how these are managed, a block based device or file-system
 sharing is required. Kata Containers does this by way of `virtio-blk` and/or `virtio-fs`.
 - Networking: A method for enabling network connectivity with the workload is required. Typically this will be done providing a `TAP` device
 to the VMM, and this will be exposed to the guest as a `virtio-net` device. It is feasible to pass in a NIC device directly, in which case `VFIO` is leveraged
 and the device itself will be exposed to the guest.
 - Control: In order to interact with the guest agent and retrieve `STDIO` from containers, a medium of communication is required.
 This is available via `virtio-vsock`.
 - Devices: `VFIO` is utilized when devices are passed directly to the virtual machine and exposed to the container.
- Dynamic Resource Management: `ACPI` is utilized to allow for dynamic VM resource management (for example: CPU, memory, device hotplug). This is required when containers are resized,
 or more generally when containers are added to a pod. 
 
How these devices are utilized varies depending on the VMM utilized. We clarify the default settings provided when integrating Kata
with the QEMU, Firecracker and Cloud Hypervisor VMMs in the following sections.

### Devices

Each virtio device is implemented by a backend, which may execute within userspace on the host (vhost-user), the VMM itself, or within the host kernel (vhost). While it may provide enhanced performance,
vhost devices are often seen as higher risk since an exploit would be already running within the kernel space. While VMM and vhost-user are both in userspace on the host, `vhost-user` generally allows for the back-end process to require less system calls and capabilities compared to a full VMM.

#### `virtio-blk` and `virtio-scsi`

The backend for `virtio-blk` and `virtio-scsi` are based in the VMM itself (ring3 in the context of x86) by default for Cloud Hypervisor, Firecracker and QEMU.
While `vhost` based back-ends are available for QEMU, it is not recommended. `vhost-user` back-ends are being added for Cloud Hypervisor, they are not utilized in Kata today.

#### `virtio-fs`

`virtio-fs` is supported in Cloud Hypervisor and QEMU. `virtio-fs`'s interaction with the host filesystem is done through a vhost-user daemon, `virtiofsd`.
The `virtio-fs` client, running in the guest, will generate requests to access files. `virtiofsd` will receive requests, open the file, and request the VMM
to `mmap` it into the guest. When DAX is utilized, the guest will access the host's page cache, avoiding the need for copy and duplication. DAX is still an experimental feature,
and is not enabled by default.

From the `virtiofsd` [documentation](https://qemu-project.gitlab.io/qemu/tools/virtiofsd.html): 
```This program must be run as the root user. Upon startup the program will switch into a new file system namespace with the shared directory tree as its root. This prevents “file system escapes” due to symlinks and other file system objects that might lead to files outside the shared directory. The program also sandboxes itself using seccomp(2) to prevent ptrace(2) and other vectors that could allow an attacker to compromise the system after gaining control of the virtiofsd process.```

DAX-less support for `virtio-fs` is available as of the 5.4 Linux kernel. QEMU VMM supports virtio-fs as of v4.2. Cloud Hypervisor
supports `virtio-fs`.

#### `virtio-net`

`virtio-net` has many options, depending on the VMM and Kata configurations.

##### QEMU networking

While QEMU has options for `vhost`, `virtio-net` and `vhost-user`, the `virtio-net` backend
for Kata defaults to `vhost-net` for performance reasons. The default configuration is being
reevaluated.

##### Firecracker networking

For Firecracker, the `virtio-net` backend is within Firecracker's VMM.

##### Cloud Hypervisor networking

For Cloud Hypervisor, the current backend default is within the VMM. `vhost-user-net` support
is being added (written in rust, Cloud Hypervisor specific).

#### virtio-vsock

##### QEMU vsock

In QEMU, vsock is backed by `vhost_vsock`, which runs within the kernel itself.

##### Firecracker and Cloud Hypervisor

In Firecracker and Cloud Hypervisor, vsock is backed by a unix-domain-socket in the hosts userspace.

#### VFIO

Utilizing VFIO, devices can be passed through to the virtual machine. We will assess this separately. Exposure to
host is limited to gaps in device pass-through handling. This is supported in QEMU and Cloud Hypervisor, but not
Firecracker.

#### ACPI

ACPI is necessary for hotplug of CPU, memory and devices. ACPI is available in QEMU and Cloud Hypervisor. Device, CPU and memory hotplug
are not available in Firecracker.

## Devices and threat model

![Threat model](threat-model-boundaries.svg "threat-model")

