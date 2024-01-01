# Hypervisors

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
|[ACRN] | C | `x86_64` | Type 1 (bare metal) | `configuration-acrn.toml` |
|[Cloud Hypervisor] | rust | `aarch64`, `x86_64` | Type 2 ([KVM]) | `configuration-clh.toml` |
|[Firecracker] | rust | `aarch64`, `x86_64` | Type 2 ([KVM]) | `configuration-fc.toml` |
|[QEMU] | C | all | Type 2 ([KVM]) | `configuration-qemu.toml` |
|[`Dragonball`] | rust | `aarch64`, `x86_64` | Type 2 ([KVM]) | `configuration-dragonball.toml` |
|[StratoVirt] | rust | `aarch64`, `x86_64` | Type 2 ([KVM]) | `configuration-stratovirt.toml` |

## Determine currently configured hypervisor

```bash
$ kata-runtime kata-env | awk -v RS= '/\[Hypervisor\]/' | grep Path
```

## Choose a Hypervisor

The table below provides a brief summary of some of the differences between
the hypervisors:


| Hypervisor | Summary | Features | Limitations | Container Creation speed | Memory density | Use cases | Comment |
|-|-|-|-|-|-|-|-|
|[ACRN] | Safety critical and real-time workloads | | | excellent | excellent | Embedded and IOT systems | For advanced users |
|[Cloud Hypervisor] | Low latency, small memory footprint, small attack surface | Minimal | | excellent | excellent | High performance modern cloud workloads | |
|[Firecracker] | Very slimline | Extremely minimal | Doesn't support all device types | excellent | excellent | Serverless / FaaS | |
|[QEMU] | Lots of features | Lots | | good | good | Good option for most users | |
|[`Dragonball`] | Built-in VMM,  low CPU and memory overhead| Minimal | | excellent | excellent | Optimized for most container workloads | `out-of-the-box` Kata Containers experience |
|[StratoVirt] | Unified architecture supporting three scenarios: VM, container, and serverless | Extremely minimal(`MicroVM`) to Lots(`StandardVM`) | | excellent | excellent | Common container workloads | `StandardVM` type of StratoVirt for Kata is under development |

For further details, see the [Virtualization in Kata Containers](design/virtualization.md) document and the official documentation for each hypervisor.

[ACRN]: https://projectacrn.org
[Cloud Hypervisor]: https://github.com/cloud-hypervisor/cloud-hypervisor
[Firecracker]: https://github.com/firecracker-microvm/firecracker
[KVM]: https://en.wikipedia.org/wiki/Kernel-based_Virtual_Machine
[QEMU]: http://www.qemu-project.org
[`Dragonball`]: https://github.com/kata-containers/kata-containers/blob/main/src/dragonball
[StratoVirt]: https://gitee.com/openeuler/stratovirt
