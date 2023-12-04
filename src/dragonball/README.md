# Introduction
`Dragonball Sandbox` is a light-weight virtual machine manager (VMM) based on Linux Kernel-based Virtual Machine (KVM),
which is optimized for container workloads with:
- container image management and acceleration service
- flexible and high-performance virtual device drivers
- low CPU and memory overhead
- minimal startup time
- optimized concurrent startup speed

`Dragonball Sandbox` aims to provide a simple solution for the Kata Containers community. It is integrated into Kata 3.0
runtime as a built-in VMM and gives users an out-of-the-box Kata Containers experience without complex environment setup
and configuration process.

# Getting Started
[TODO](https://github.com/kata-containers/kata-containers/issues/4302)

# Documentation

- Device: [Device Document](docs/device.md)
- vCPU: [vCPU Document](docs/vcpu.md)
- API: [API Document](docs/api.md)
- `Upcall`: [`Upcall` Document](docs/upcall.md)
- `dbs_acpi`: [`dbs_acpi` Document](src/dbs_acpi/README.md)
- `dbs_address_space`: [`dbs_address_space` Document](src/dbs_address_space/README.md)
- `dbs_allocator`: [`dbs_allocator` Document](src/dbs_allocator/README.md)
- `dbs_arch`: [`dbs_arch` Document](src/dbs_arch/README.md)
- `dbs_boot`: [`dbs_boot` Document](src/dbs_boot/README.md)
- `dbs_device`: [`dbs_device` Document](src/dbs_device/README.md)
- `dbs_interrupt`: [`dbs_interrput` Document](src/dbs_interrupt/README.md)
- `dbs_legacy_devices`: [`dbs_legacy_devices` Document](src/dbs_legacy_devices/README.md)
- `dbs_tdx`: [`dbs_tdx` Document](src/dbs_tdx/README.md)
- `dbs_upcall`: [`dbs_upcall` Document](src/dbs_upcall/README.md)
- `dbs_utils`: [`dbs_utils` Document](src/dbs_utils/README.md)
- `dbs_virtio_devices`: [`dbs_virtio_devices` Document](src/dbs_virtio_devices/README.md)
- `dbs_pci`: [`dbc_pci` Document](src/dbs_pci/README.md)

Currently, the documents are still actively adding.
You could see the [official documentation](docs/) page for more details.

# Supported Architectures
- x86-64
- aarch64
 
# Supported Kernel
[TODO](https://github.com/kata-containers/kata-containers/issues/4303)

# Acknowledgement
Part of the code is based on the [Cloud Hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor) project, [`crosvm`](https://github.com/google/crosvm) project and [Firecracker](https://github.com/firecracker-microvm/firecracker) project. They are all rust written virtual machine managers with advantages on safety and security.

`Dragonball sandbox` is designed to be a VMM that is customized for Kata Containers and we will focus on optimizing container workloads for Kata ecosystem. The focus on the Kata community is what differentiates us from other rust written virtual machines.

# License

`Dragonball` is licensed under [Apache License](http://www.apache.org/licenses/LICENSE-2.0), Version 2.0.