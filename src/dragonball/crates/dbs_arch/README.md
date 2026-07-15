# dbs-arch

## Design

The `dbs-arch` crate is a collection of CPU architecture specific constants and utilities to hide CPU architecture details away from the Dragonball Sandbox or other VMMs.
Also, we have provided x86_64 CPUID support in this crate, for more details you could look at [this document](docs/x86_64_cpuid.md)

## Supported Architectures

- AMD64 (x86_64)
- ARM64 (aarch64)

## Submodule List

This repository contains the following submodules:
| Name | Arch| Description |
| --- | --- | --- |
| [x86_64::cpuid](src/x86_64/cpuid/) | x86_64 |Facilities to process CPUID information. |
| [x86_64::msr](src/x86_64/msr.rs) | x86_64 | Constants and functions for Model Specific Registers |
| [aarch64::gic](src/aarch64/gic) | aarch64 | Structures to manage GICv2/GICv3/ITS devices for ARM64 |
| [aarch64::regs](src/aarch64/regs.rs) | aarch64 | Constants and functions to configure and manage CPU registers |

## Acknowledgement

Part of the code is derived from the [Firecracker](https://github.com/firecracker-microvm/firecracker) project.

## License

This project is licensed under [Apache License](http://www.apache.org/licenses/LICENSE-2.0), Version 2.0.
