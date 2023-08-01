# dbs-boot

## Design

The `dbs-boot` crate is a collection of constants, structs and utilities used to boot virtual machines.

## Submodule List

This repository contains the following submodules:
| Name | Arch| Description |
| --- | --- | --- |
| [`bootparam`](src/x86_64/bootparam.rs) | x86_64 | Magic addresses externally used to lay out x86_64 VMs |
| [fdt](src/aarch64/fdt.rs) | aarch64| Create FDT for Aarch64 systems |
| [layout](src/x86_64/layout.rs) | x86_64 | x86_64 layout constants |
| [layout](src/aarch64/layout.rs/) | aarch64 | aarch64 layout constants |
| [mptable](src/x86_64/mptable.rs) | x86_64 | MP Table configurations used for defining VM boot status |

## Acknowledgement

Part of the code is derived from the [Firecracker](https://github.com/firecracker-microvm/firecracker) project.

## License

This project is licensed under [Apache License](http://www.apache.org/licenses/LICENSE-2.0), Version 2.0.
