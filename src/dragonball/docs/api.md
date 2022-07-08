# API

We provide plenty API for Kata runtime to interact with `Dragonball` virtual machine manager.
This document provides the introduction for each of them.

## `ConfigureBootSource`
Configure the boot source of the VM using `BootSourceConfig`. This action can only be called before the VM has booted.

### Boot Source Config
1. `kernel_path`: Path of the kernel image. `Dragonball` only supports compressed kernel image for now.
2. `initrd_path`: Path of the initrd (could be None)
3. `boot_args`: Boot arguments passed to the kernel (could be None)

## `SetVmConfiguration`
Set virtual machine configuration using `VmConfigInfo` to initialize VM.

### VM Config Info
1. `vcpu_count`: Number of vCPU to start. Currently we only support up to 255 vCPUs.
2. `max_vcpu_count`: Max number of vCPU can be added through CPU hotplug.
3. `cpu_pm`: CPU power management.
4. `cpu_topology`: CPU topology information (including `threads_per_core`, `cores_per_die`, `dies_per_socket` and `sockets`).
5. `vpmu_feature`: `vPMU` feature level.
6. `mem_type`: Memory type that can be either `hugetlbfs` or `shmem`, default is `shmem`.
7. `mem_file_path` : Memory file path.
8. `mem_size_mib`: The memory size in MiB. The maximum memory size is 1TB.
9. `serial_path`: Optional sock path.

