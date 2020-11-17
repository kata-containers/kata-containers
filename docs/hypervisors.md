# Hypervisors

* [Hypervisors](#hypervisors)
    * [Introduction](#introduction)
    * [Types](#types)
    * [Determine currently configured hypervisor](#determine-currently-configured-hypervisor)
    * [Choose a Hypervisor](#choose-a-hypervisor)

## Introduction

Kata Containers supports multiple hypervisors. This document provides a very
high level overview of the available hypervisors, giving suggestions as to
which hypervisors you may wish to investigate further.

> **Note:**
>
> This document is not prescriptive or authoritative:
>
> - It is up to you to decide which hypervisors may be most appropriate for
>   your use-case.
> - Refer to the official documentation for each hypervisor for further details.

## Types

Since each hypervisor offers different features and options, Kata Containers
provides a separate
[configuration file](/src/runtime/README.md#configuration)
for each. The configuration files contain comments explaining which options
are available, their default values and how each setting can be used.

> **Note:**
>
> The simplest way to switch between hypervisors is to create a symbolic link
> to the appropriate hypervisor-specific configuration file.

| Hypervisor | Written in | Architectures | Type | Configuration file |
|-|-|-|-|-|
[ACRN] | C | `x86_64` | Type 1 (bare metal) | `configuration-acrn.toml` |
[Cloud Hypervisor] | rust | `aarch64`, `x86_64` | Type 2 ([KVM]) | `configuration-clh.toml` |
[Firecracker] | rust | `aarch64`, `x86_64` | Type 2 ([KVM]) | `configuration-fc.toml` |
[QEMU] | C | all | Type 2 ([KVM]) | `configuration-qemu.toml` |

## Determine currently configured hypervisor

```bash
$ kata-runtime kata-env | awk -v RS= '/\[Hypervisor\]/' | grep Path
```

## Choose a Hypervisor

The table below provides a brief summary of some of the differences between
the hypervisors:


| Hypervisor | Summary | Features | Limitations | Container Creation speed | Memory density | Use cases | Comment |
|-|-|-|-|-|-|-|-|
[ACRN] | Safety critical and real-time workloads | | | excellent | excellent | Embedded and IOT systems | For advanced users |
[Cloud Hypervisor] | Low latency, small memory footprint, small attack surface | Minimal | | excellent | excellent | High performance modern cloud workloads | |
[Firecracker] | Very slimline | Extremely minimal | Doesn't support all device types | excellent | excellent | Serverless / FaaS | |
[QEMU] | Lots of features | Lots | | good | good | Good option for most users | | All users |

For further details, see the [Virtualization in Kata Containers](design/virtualization.md) document and the official documentation for each hypervisor.

[ACRN]: https://projectacrn.org
[Cloud Hypervisor]: https://github.com/cloud-hypervisor/cloud-hypervisor
[Firecracker]: https://github.com/firecracker-microvm/firecracker
[KVM]: https://en.wikipedia.org/wiki/Kernel-based_Virtual_Machine
[QEMU]: http://www.qemu-project.org
