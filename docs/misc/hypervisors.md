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

| Hypervisor | Written in | Architectures | Type |
|-|-|-|-|
|[Cloud Hypervisor] | rust | `aarch64`, `x86_64` | Type 2 ([KVM]) |
|[Firecracker] | rust | `aarch64`, `x86_64` | Type 2 ([KVM]) |
|[QEMU] | C | all | Type 2 ([KVM]) | `configuration-qemu.toml` |
|[`Dragonball`] | rust | `aarch64`, `x86_64` | Type 2 ([KVM]) |
|[StratoVirt] | rust | `aarch64`, `x86_64` | Type 2 ([KVM]) |

## Determine currently configured hypervisor

```bash
$ kata-runtime kata-env | awk -v RS= '/\[Hypervisor\]/' | grep Path
```

## Choose a Hypervisor

The table below provides a brief summary of some of the differences between
the hypervisors:

| Hypervisor | Summary | Features | Limitations | Container Creation speed | Memory density | Use cases | Comment |
|-|-|-|-|-|-|-|-|
|[Cloud Hypervisor] | Low latency, small memory footprint, small attack surface | Minimal | | excellent | excellent | High performance modern cloud workloads | |
|[Firecracker] | Very slimline | Extremely minimal | Doesn't support all device types | excellent | excellent | Serverless / FaaS | |
|[QEMU] | Lots of features | Lots | | good | good | Good option for most users | |
|[`Dragonball`] | Built-in VMM,  low CPU and memory overhead| Minimal | | excellent | excellent | Optimized for most container workloads | `out-of-the-box` Kata Containers experience |
|[StratoVirt] | Unified architecture supporting three scenarios: VM, container, and serverless | Extremely minimal(`MicroVM`) to Lots(`StandardVM`) | | excellent | excellent | Common container workloads | `StandardVM` type of StratoVirt for Kata is under development |

For further details, see the [Virtualization in Kata Containers](design/virtualization.md) document and the official documentation for each hypervisor.

## Hypervisor configuration files

Since each hypervisor offers different features and options, Kata Containers
provides a separate
[configuration file](../src/runtime/README.md#configuration)
for each. The configuration files contain comments explaining which options
are available, their default values and how each setting can be used.

| Hypervisor | Golang runtime config file | golang runtime short name | golang runtime default | rust runtime config file | rust runtime short name | rust runtime default |
|-|-|-|-|-|-|-|
| [Cloud Hypervisor] | [`configuration-clh.toml`](../src/runtime/config/configuration-clh.toml.in) | `clh` | | [`configuration-cloud-hypervisor.toml`](../src/runtime-rs/config/configuration-cloud-hypervisor.toml.in) | `cloud-hypervisor` | |
| [Firecracker] | [`configuration-fc.toml`](../src/runtime/config/configuration-fc.toml.in) | `fc` | | | | |
| [QEMU] | [`configuration-qemu.toml`](../src/runtime/config/configuration-qemu.toml.in) | `qemu` | yes | [`configuration-qemu.toml`](../src/runtime-rs/config/configuration-qemu-runtime-rs.toml.in) | `qemu` | |
| [`Dragonball`] | | | | [`configuration-dragonball.toml`](../src/runtime-rs/config/configuration-dragonball.toml.in) | `dragonball` | yes |
| [StratoVirt] | [`configuration-stratovirt.toml`](../src/runtime/config/configuration-stratovirt.toml.in) | `stratovirt` | | | | |

> **Notes:**
>
> - The short names specified are used by the [`kata-manager`](../utils/README.md) tool.
> - As shown by the default columns, each runtime type has its own default hypervisor.
> - The [golang runtime](../src/runtime) is the current default runtime.
> - The [rust runtime](../src/runtime-rs), also known as `runtime-rs`,
>   is the newer runtime written in the rust language.
> - See the [Configuration](../README.md#configuration) for further details.
> - The configuration file links in the table link to the "source"
>   versions: these are not usable configuration files as they contain
>   variables that need to be expanded:
>   - The links are provided for reference only.
>   - The final (installed) versions, where all variables have been
>     expanded, are built from these source configuration files.
> - The pristine configuration files are usually installed in the
>   `/opt/kata/share/defaults/kata-containers/` or
>   `/usr/share/defaults/kata-containers/` directories.
> - Some hypervisors may have the same name for both golang and rust
>   runtimes, but the file contents may differ.
> - If there is no configuration file listed for the golang or
>   rust runtimes, this either means the hypervisor cannot be run with
>   a particular runtime, or that a driver has not yet been made
>   available for that runtime.

## Switch configured hypervisor

To switch the configured hypervisor, you only need to run a single command.
See [the `kata-manager` documentation](../utils/README.md#choose-a-hypervisor) for further details.

[Cloud Hypervisor]: https://github.com/cloud-hypervisor/cloud-hypervisor
[Firecracker]: https://github.com/firecracker-microvm/firecracker
[KVM]: https://en.wikipedia.org/wiki/Kernel-based_Virtual_Machine
[QEMU]: http://www.qemu.org
[`Dragonball`]: https://github.com/kata-containers/kata-containers/blob/main/src/dragonball
[StratoVirt]: https://gitee.com/openeuler/stratovirt
