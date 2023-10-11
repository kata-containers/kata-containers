# Virtualization Reference Architecture

## Subject to Change | © 2022 by NVIDIA Corporation. All rights reserved. | For test and development only_

Before digging deeper into the virtualization reference architecture, let's
first look at the various GPUDirect use cases in the following table. We’re
distinguishing between two top-tier use cases where the devices are (1)
passthrough and (2) virtualized, where a VM gets assigned a virtual function
(VF) and not the physical function (PF). A combination of PF and VF would also
be possible.

| Device #1  (passthrough)  | Device #2 (passthrough) | P2P Compatibility and Mode                   |
| ------------------------- | ----------------------- | -------------------------------------------- |
| GPU PF                    | GPU PF                  | GPUDirect P2P                                |
| GPU PF                    | NIC PF                  | GPUDirect RDMA                               |
| MIG-slice                 | MIG-slice               | _No GPUDirect P2P_                           |
| MIG-slice                 | NIC PF                  | GPUDirect RDMA                               |
| **PDevice #1  (virtualized)** | **Device #2 (virtualized)** | **P2P Compatibility and   Mode**     |
| Time-slice vGPU VF        | Time-slice vGPU VF      | _No GPUDirect P2P  but NVLINK P2P available_ |
| Time-slice vGPU VF        | NIC VF                  | GPUDirect RDMA                               |
| MIG-slice vGPU            | MIG-slice vGPU          | _No GPUDirect P2P_                           |
| MIG-slice vGPU            | NIC VF                  | GPUDirect RDMA                               |

In a virtualized environment we have several distinct features that may prevent
Peer-to-peer (P2P) communication of two endpoints in a PCI Express topology. The
IOMMU translates IO virtual addresses (IOVA) to physical addresses (PA). Each
device behind an IOMMU has its own IOVA memory space, usually, no two devices
share the same IOVA memory space but it’s up to the hypervisor or OS how it
chooses to map devices to IOVA spaces.  Any PCI Express DMA transactions will
use IOVAs, which the IOMMU must translate. By default, all the traffic is routed
to the root complex and not issued directly to the peer device.

An IOMMU can be used to isolate and protect devices even if virtualization is
not used; since devices can only access memory regions that are mapped for it, a
DMA from one device to another is not possible. DPDK uses the IOMMU to have
better isolation between devices, another benefit is that IOVA space can be
represented as a contiguous memory even if the PA space is heavily scattered.

In the case of virtualization, the IOMMU is responsible for isolating the device
and memory between VMs for safe device assignment without compromising the host
and other guest OSes. Without an IOMMU, any device can access the entire system
and perform DMA transactions _anywhere_.

The second feature is ACS (Access Control Services), which controls which
devices are allowed to communicate with one another and thus avoids improper
routing of packets `irrespectively` of whether IOMMU is enabled or not.

When IOMMU is enabled, ACS is normally configured to force all PCI Express DMA
to go through the root complex so IOMMU can translate it, impacting performance
between peers with higher latency and reduced bandwidth.

A way to avoid the performance hit is to enable Address Translation Services
(ATS). ATS-capable endpoints can prefetch IOVA -> PA translations from the IOMMU
and then perform DMA transactions directly to another endpoint. Hypervisors
enable this by enabling ATS in such endpoints, configuring ACS to enable Direct
Translated P2P, and configuring the IOMMU to allow Address Translation requests.

Another important factor is that the NVIDIA driver stack will use the PCI
Express topology of the system it is running on to determine whether the
hardware is capable of supporting P2P. The driver stack qualifies specific
chipsets, and PCI Express switches for use with GPUDirect P2P. In virtual
environments, the PCI Express topology is flattened and obfuscated to present a
uniform environment to the software inside the VM, which breaks the GPUDirect
P2P use case.

On a bare metal machine, the driver stack groups GPUs into cliques that can
perform GPUDirect P2P communication, excluding peer mappings where P2P
communication is not possible, prominently if GPUs are attached to multiple CPU
sockets.  

CPUs and local memory banks are referred to as NUMA nodes. In a two-socket
server, each of the CPUs has a local memory bank for a total of two NUMA nodes.
Some servers provide the ability to configure additional NUMA nodes per CPU,
which means a CPU socket can have two NUMA nodes  (some servers support four
NUMA nodes per socket) with local memory banks and L3 NUMA domains for improved
performance.

One of the current solutions is that the hypervisor provides additional topology
information that the driver stack can pick up and enable GPUDirect P2P between
GPUs, even if the virtualized environment does not directly expose it. The PCI
Express virtual P2P approval capability structure in the PCI configuration space
is entirely emulated by the hypervisor of passthrough GPU devices.

A clique ID is provided where GPUs with the same clique ID belong to a group of
GPUs capable of P2P communication

On vSphere, Azure, and other CPSs,  the hypervisor lays down a `topologies.xml`
which NCCL can pick up and deduce the right P2P level[^1]. NCCL is leveraging
Infiniband (IB) and/or Unified Communication X (UCX) for communication, and
GPUDirect P2P and GPUDirect RDMA should just work in this case. The only culprit
is that software or applications that do not use the XML file to deduce the
topology will fail and not enable GPUDirect ( [`nccl-p2p-level`](https://docs.nvidia.com/deeplearning/nccl/user-guide/docs/env.html#nccl-p2p-level) )

## Hypervisor PCI Express Topology

To enable every part of the accelerator stack, we propose a virtualized
reference architecture to enable GPUDirect P2P and GPUDirect RDMA for any
hypervisor. The idea is split into two parts to enable the right PCI Express
topology. The first part builds upon extending the PCI Express virtual P2P
approval capability structure to every device that wants to do P2P in some way
and groups devices by clique ID. The other part involves replicating a subset of
the host topology so that applications running in the VM do not need to read
additional information and enable the P2P capability like in the bare-metal use
case described above. The driver stack can then deduce automatically if the
topology presented in the VM is capable of P2P communication.

We will work with the following host topology for the following sections. It is
a system with two converged DPUs, each having an `A100X` GPU and two `ConnectX-6`
network ports connected to the downstream ports of a PCI Express switch.

```sh
+-00.0-[d8-df]----00.0-[d9-df]--+-00.0-[da-db]--+-00.0  Mellanox Tech MT42822 BlueField-2 integrated ConnectX-6 Dx network
                                |               +-00.1  Mellanox Tech MT42822 BlueField-2 integrated ConnectX-6 Dx network
                                |               \-00.2  Mellanox Tech MT42822 BlueField-2 SoC Management Interface
                                 \-01.0-[dc-df]----00.0-[dd-df]----08.0-[de-df]----00.0  NVIDIA Corporation GA100 [A100X]

+-00.0-[3b-42]----00.0-[3c-42]--+-00.0-[3d-3e]--+-00.0  Mellanox Tech MT42822 BlueField-2 integrated ConnectX-6 Dx network
                                |               +-00.1  Mellanox Tech MT42822 BlueField-2 integrated ConnectX-6 Dx network
                                |               \-00.2  Mellanox Tech MT42822 BlueField-2 SoC Management Interface
                                 \-01.0-[3f-42]----00.0-[40-42]----08.0-[41-42]----00.0  NVIDIA Corporation GA100 [A100X]
```

The green path highlighted above is the optimal and preferred path for
efficient P2P communication.

## PCI Express Virtual P2P Approval Capability

Most of the time, the PCI Express topology is flattened and obfuscated to ensure
easy migration of the VM image between different physical hardware `topologies`.
In Kata, we can configure the hypervisor to use PCI Express root ports to
hotplug the VFIO  devices one is passing through. A user can select how many PCI
Express root ports to allocate depending on how many devices are passed through.
A recent addition to Kata will detect the right amount of PCI Express devices
that need hotplugging and bail out if the number of root ports is insufficient.
In Kata, we do not automatically increase the number of root ports, we want the
user to be in full control of the topology.

```toml
# /etc/kata-containers/configuration.toml

# VFIO devices are hotplugged on a bridge by default.
# Enable hot-plugging on the root bus. This may be required for devices with
# a large PCI bar, as this is a current limitation with hot-plugging on
# a bridge.
# Default “bridge-port”
hotplug_vfio = "root-port"

# Before hot plugging a PCIe device, you need to add a pcie_root_port device.
# Use this parameter when using some large PCI bar devices, such as NVIDIA GPU
# The value means the number of pcie_root_port
# This value is valid when hotplug_vfio_on_root_bus is true and machine_type is "q35"
# Default 0
pcie_root_port = 8
```

VFIO devices are hotplugged on a PCIe-PCI bridge by default. Hotplug of PCI
Express devices is only supported on PCI Express root or downstream ports. With
this configuration set, if we start up a Kata container, we can inspect our
topology and see the allocated PCI Express root ports and the hotplugged
devices.

```sh
$ lspci -tv
 -[0000:00]-+-00.0  Intel Corporation 82G33/G31/P35/P31 Express DRAM Controller
           +-01.0  Red Hat, Inc. Virtio console
           +-02.0  Red Hat, Inc. Virtio SCSI
           +-03.0  Red Hat, Inc. Virtio RNG
           +-04.0-[01]----00.0  Mellanox Technologies MT42822 BlueField-2 integrated ConnectX-6
           +-05.0-[02]----00.0  Mellanox Technologies MT42822 BlueField-2 integrated ConnectX-6
           +-06.0-[03]----00.0  NVIDIA Corporation Device 20b8
           +-07.0-[04]----00.0  NVIDIA Corporation Device 20b8
           +-08.0-[05]--
           +-09.0-[06]--
           +-0a.0-[07]--
           +-0b.0-[08]--
           +-0c.0  Red Hat, Inc. Virtio socket
           +-0d.0  Red Hat, Inc. Virtio file system
           +-1f.0  Intel Corporation 82801IB (ICH9) LPC Interface Controller
           +-1f.2  Intel Corporation 82801IR/IO/IH (ICH9R/DO/DH) 6 port SATA Controller
           \-1f.3  Intel Corporation 82801I (ICH9 Family) SMBus Controller
```

For devices with huge BARs (Base Address Registers) like the GPU (we need to
configure the PCI Express root port properly and allocate enough memory for
mapping), we have added a heuristic to Kata to deduce the right settings. Hence,
the BARs can be mapped correctly. This functionality is added to
[`nvidia/go-nvlib1](https://gitlab.com/nvidia/cloud-native/go-nvlib) which is part
of Kata now.

```sh
$ sudo dmesg | grep BAR
[    0.179960] pci 0000:00:04.0: BAR 7: assigned [io  0x1000-0x1fff]
[    0.179962] pci 0000:00:05.0: BAR 7: assigned [io  0x2000-0x2fff]
[    0.179963] pci 0000:00:06.0: BAR 7: assigned [io  0x3000-0x3fff]
[    0.179964] pci 0000:00:07.0: BAR 7: assigned [io  0x4000-0x4fff]
[    0.179966] pci 0000:00:08.0: BAR 7: assigned [io  0x5000-0x5fff]
[    0.179967] pci 0000:00:09.0: BAR 7: assigned [io  0x6000-0x6fff]
[    0.179968] pci 0000:00:0a.0: BAR 7: assigned [io  0x7000-0x7fff]
[    0.179969] pci 0000:00:0b.0: BAR 7: assigned [io  0x8000-0x8fff]
[    2.115912] pci 0000:01:00.0: BAR 0: assigned [mem 0x13000000000-0x13001ffffff 64bit pref]
[    2.116203] pci 0000:01:00.0: BAR 2: assigned [mem 0x13002000000-0x130027fffff 64bit pref]
[    2.683132] pci 0000:02:00.0: BAR 0: assigned [mem 0x12000000000-0x12001ffffff 64bit pref]
[    2.683419] pci 0000:02:00.0: BAR 2: assigned [mem 0x12002000000-0x120027fffff 64bit pref]
[    2.959155] pci 0000:03:00.0: BAR 1: assigned [mem 0x11000000000-0x117ffffffff 64bit pref]
[    2.959345] pci 0000:03:00.0: BAR 3: assigned [mem 0x11800000000-0x11801ffffff 64bit pref]
[    2.959523] pci 0000:03:00.0: BAR 0: assigned [mem 0xf9000000-0xf9ffffff]
[    2.966119] pci 0000:04:00.0: BAR 1: assigned [mem 0x10000000000-0x107ffffffff 64bit pref]
[    2.966295] pci 0000:04:00.0: BAR 3: assigned [mem 0x10800000000-0x10801ffffff 64bit pref]
[    2.966472] pci 0000:04:00.0: BAR 0: assigned [mem 0xf7000000-0xf7ffffff]
```

The NVIDIA driver stack in this case would refuse to do P2P communication since
(1) the topology is not what it expects, (2)  we do not have a qualified
chipset. Since our P2P devices are not connected to a PCI Express switch port,
we need to provide additional information to support the P2P functionality. One
way of providing such meta information would be to annotate the container; most
of the settings in Kata's configuration file can be overridden via annotations,
but this limits the flexibility, and a user would need to update all the
containers that he wants to run with Kata. The goal is to make such things as
transparent as possible, so we also introduced
[CDI](https://github.com/container-orchestrated-devices/container-device-interface)
(Container Device Interface) to Kata. CDI is a[
specification](https://github.com/container-orchestrated-devices/container-device-interface/blob/main/SPEC.md)
for container runtimes to support third-party devices.

As written before, we can provide a clique ID for the devices that belong
together and are capable of doing P2P. This information is provided to the
hypervisor, which will set up things in the VM accordingly. Let's suppose the
user wanted to do GPUDirect RDMA with the first GPU and the NIC that reside on
the same DPU, one could provide the specification telling the hypervisor that
they belong to the same clique.

```yaml
# /etc/cdi/nvidia.yaml
cdiVersion: 0.4.0
kind: nvidia.com/gpu
devices:
- name: gpu0
  annotations:
    bdf: “41:00.0”
    clique-id: “0”
  containerEdits:
    deviceNodes:
    - path: “/dev/vfio/71"

# /etc/cdi/mellanox.yaml
cdiVersion: 0.4.0
kind: mellanox.com/nic
devices:
- name: nic0
  annotations:
    bdf: “3d:00.0”
    clique-id: “0”
    attach-pci: “true”
  containerEdits:
    deviceNodes:
    - path: "/dev/vfio/66"
```

Since this setting is bound to the device and not the container we do not need
to alter the container just allocate the right resource and GPUDirect RDMA would
be set up correctly. Rather than exposing them separately, an idea would be to
expose a GPUDirect RDMA device via NFD (Node Feature Discovery) that combines
both of them; this way, we could make sure that the right pair is allocated and
used more on  Kubernetes deployment in the next section.

The GPU driver stack is leveraging the PCI Express virtual P2P approval
capability, but the NIC stack does not use this now. One of the action items is
to enable MOFED to read the P2P approval capability and enable ATS and ACS
settings as described above.

This way, we could enable GPUDirect P2P and GPUDirect RDMA on any topology
presented to the VM application. It is the responsibility of the administrator
or infrastructure engineer to provide the right information either via
annotations or a CDI specification.

## Host Topology Replication

The other way to represent the PCI Express topology in the VM is to replicate a
subset of the topology needed to support the P2P use case inside the VM. Similar
to the configuration for the root ports, we can easily configure the usage of
PCI Express switch ports to hotplug the devices.

```toml
# /etc/kata-containers/configuration.toml

# VFIO devices are hotplugged on a bridge by default.
# Enable hot plugging on the root bus. This may be required for devices with
# a large PCI bar, as this is a current limitation with hot plugging on
# a bridge.
# Default “bridge-port”
hotplug_vfio = "switch-port"

# Before hot plugging a PCIe device, you need to add a pcie_root_port device.
# Use this parameter when using some large PCI bar devices, such as Nvidia GPU
# The value means the number of pcie_root_port
# This value is valid when hotplug_vfio_on_root_bus is true and machine_type is "q35"
# Default 0
pcie_switch_port = 8
```

Each device that is passed through is attached to a PCI Express downstream port
as illustrated below. We can even replicate the host’s two DPUs `topologies` with
added metadata through the CDI. Most of the time, a container only needs one
pair of GPU and NIC for GPUDirect RDMA. This is more of a showcase of what we
can do with the power of Kata and CDI. One could even think of adding groups of
devices that support P2P, even from different CPU sockets or NUMA nodes, into
one container; indeed, the first group is NUMA node 0 (red), and the second
group is NUMA node 1 (green). Since they are grouped correctly, P2P would be
enabled naturally inside a group, aka clique ID.

```sh
$ lspci -tv
 -[0000:00]-+-00.0  Intel Corporation 82G33/G31/P35/P31 Express DRAM Controller
            +-01.0  Red Hat, Inc. Virtio console
            +-02.0  Red Hat, Inc. Virtio SCSI
            +-03.0  Red Hat, Inc. Virtio RNG
            +-04.0-[01-04]----00.0-[02-04]--+-00.0-[03]----00.0  NVIDIA Corporation Device 20b8
            |                               \-01.0-[04]----00.0  Mellanox Tech MT42822 BlueField-2 integrated ConnectX-6 Dx
            +-05.0-[05-08]----00.0-[06-08]--+-00.0-[07]----00.0  Mellanox Tech MT42822 BlueField-2 integrated ConnectX-6 Dx
            |                               \-01.0-[08]----00.0  NVIDIA Corporation Device 20b8
            +-06.0  Red Hat, Inc. Virtio socket
            +-07.0  Red Hat, Inc. Virtio file system
            +-1f.0  Intel Corporation 82801IB (ICH9) LPC Interface Controller
            +-1f.2  Intel Corporation 82801IR/IO/IH (ICH9R/DO/DH) 6 port SATA Controller [AHCI mode]
            \-1f.3  Intel Corporation 82801I (ICH9 Family) SMBus Controller
            \-1f.3  Intel Corporation 82801I (ICH9 Family) SMBus Controller
```

The configuration of using either the root port or switch port can be applied on
a per Container or Pod basis, meaning we can switch PCI Express `topologies` on
each run of an application.

## Hypervisor Resource Limits

Every hypervisor will have resource limits in terms of how many PCI Express root
ports, switch ports, or bridge ports can be created, especially with devices
that need to reserve a 4K IO range per PCI specification. Each instance of root
or switch port will consume 4K IO of very limited capacity, 64k is the maximum.

Simple math brings us to the conclusion that we can have a maximum of 16 PCI
Express root ports or 16 PCI Express switch ports in QEMU if devices with IO
BARs are used in the PCI Express hierarchy.

Additionally, one can have 32 slots on the PCI root bus and a maximum of 256
slots for the complete PCI(e) topology.

Per default, QEMU will attach a multi-function device in the last slot on the
PCI root bus,

```sh
 +-1f.0  Intel Corporation 82801IB (ICH9) LPC Interface Controller
 +-1f.2  Intel Corporation 82801IR/IO/IH (ICH9R/DO/DH) 6 port SATA Controller [AHCI mode]
 \-1f.3  Intel Corporation 82801I (ICH9 Family) SMBus Controller
```

Kata will additionally add `virtio-xxx-pci` devices consuming (5 slots) plus a
PCIe-PCI-bridge (1 slot) and a DRAM controller (1 slot), meaning per default, we
have already eight slots used. This leaves us 24 slots for adding other devices
to the root bus.

The problem that arises here is one use-case from a customer that uses recent
RTX GPUs with Kata. The user wanted to pass through eight of these GPUs into one
container and ran into issues. The problem is that those cards often consist of
four individual device nodes: GPU, Audio, and two USB controller devices (some
cards have a USB-C output).

These devices are grouped into one IOMMU group. Since one needs to pass through
the complete IOMMU group into the VM, we need to allocate 32 PCI Express root
ports or 32 PCI Express switch ports, which is technically impossible due to the
resource limits outlined above. Since all the devices appear as PCI Express
devices, we need to hotplug those into a root or switch port.

The solution to this problem is leveraging CDI. For each device, add the
information if it is going to be hotplugged as a PCI Express or PCI device,
which results in either using a PCI Express root/switch port or an ordinary PCI
bridge. PCI bridges are not affected by the limited IO range. This way, the GPU
is attached as a PCI Express device to a root/switch port and the other three
PCI devices to a PCI bridge, leaving enough resources to create the needed PCI
Express root/switch ports.  For example, we’re going to attach the GPUs to a PCI
Express root port and the NICs to a PCI bridge.

```jsonld
# /etc/cdi/mellanox.json
cdiVersion: 0.4.0
kind: mellanox.com/nic
devices:
- name: nic0
  annotations:
    bdf: “3d:00.0”
    clique-id: “0”
    attach-pci: “true”
  containerEdits:
    deviceNodes:
    - path: "/dev/vfio/66"
- name: nic1
  annotations:
    bdf: “3d:00.1”
    clique-id: “1”
    attach-pci: “true”
  containerEdits:
    deviceNodes:
    - path: "/dev/vfio/67”
```

The configuration is set to use eight root ports for the GPUs and attach the
NICs to a PCI bridge which is connected to a PCI Express-PCI bridge which is the
preferred way of introducing a PCI topology in a PCI Express machine.

```sh
$ lspci -tv
-[0000:00]-+-00.0  Intel Corporation 82G33/G31/P35/P31 Express DRAM Controller
           +-01.0  Red Hat, Inc. Virtio console
           +-02.0  Red Hat, Inc. Virtio SCSI
           +-03.0  Red Hat, Inc. Virtio RNG
           +-04.0-[01]----00.0  NVIDIA Corporation Device 20b8
           +-05.0-[02]----00.0  NVIDIA Corporation Device 20b8
           +-06.0-[03]--
           +-07.0-[04]--
           +-08.0-[05]--
           +-09.0-[06]--
           +-0a.0-[07]--
           +-0b.0-[08]--
           +-0c.0-[09-0a]----00.0-[0a]--+-00.0  Mellanox Tech MT42822 BlueField-2 ConnectX-6
           |                             \-01.0  Mellanox Tech MT42822 BlueField-2 ConnectX-6
           +-0d.0  Red Hat, Inc. Virtio socket
           +-0e.0  Red Hat, Inc. Virtio file system
           +-1f.0  Intel Corporation 82801IB (ICH9) LPC Interface Controller
           +-1f.2  Intel Corporation 82801IR/IO/IH (ICH9R/DO/DH) 6 port SATA Controller
           \-1f.3  Intel Corporation 82801I (ICH9 Family) SMBus Controller
```

The PCI devices will consume a slot of which we have 256 in the PCI(e) topology
and leave scarce resources for the needed PCI Express devices.
