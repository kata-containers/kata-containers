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

| Hypervisor | Written in | Architectures | GPU Support | Intel TDX | AMD SEV-SNP |
|-|-|-|-|-|-|
|[Cloud Hypervisor](#cloud-hypervisor) | rust | `aarch64`, `x86_64` | :x: | :x: | :x: |
|[Firecracker](#firecracker) | rust | `aarch64`, `x86_64` | :x: | :x: | :x: |
|[QEMU](#qemu) | C | all | :white_check_mark: | :white_check_mark: | :white_check_mark: |
|[Dragonball](#dragonball) | rust | `aarch64`, `x86_64` | :x: | :x: | :x: |
|StratoVirt | rust | `aarch64`, `x86_64` | :x: | :x: | :x: |

Each Kata runtime is configured for a specific hypervisor through the runtime's configuration file. For example:

```toml title="/opt/kata/share/defaults/kata-containers/configuration.toml"
[hypervisor.qemu]
path = "/opt/kata/bin/qemu-system-x86_64"
```

```toml title="/opt/kata/share/defaults/kata-containers/configuration-clh.toml"
[hypervisor.clh]
path = "/opt/kata/bin/cloud-hypervisor"
```

## Cloud Hypervisor

[Cloud Hypervisor](https://www.cloudhypervisor.org/) is a more modern hypervisor written in Rust.

## Firecracker

[Firecracker](https://firecracker-microvm.github.io/) is a minimal and lightweight hypervisor created for the AWS Lambda product.

## QEMU

QEMU is the best supported hypervisor for NVIDIA-based GPUs and for confidential computing use-cases (such as Intel TDX and AMD SEV-SNP). Runtimes that use this are normally named `kata-qemu-nvidia-gpu-*`. The Kata project focuses primarily on QEMU runtimes for GPU support.

## Dragonball

Dragonball is a special hypervisor created by the Ant Group that runs in the same process as the Rust-based containerd shim.
