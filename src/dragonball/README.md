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

Device: [Device Document](docs/device.md)
vCPU: [vCPU Document](docs/vcpu.md)
API: [API Document](docs/api.md)

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