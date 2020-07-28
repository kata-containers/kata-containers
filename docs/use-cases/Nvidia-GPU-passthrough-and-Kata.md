# Using Nvidia GPU device with Kata Containers

- [Using Nvidia GPU device with Kata Containers](#using-nvidia-gpu-device-with-kata-containers)
	- [Hardware Requirements](#hardware-requirements)
	- [Host BIOS Requirements](#host-bios-requirements)
	- [Host Kernel Requirements](#host-kernel-requirements)
	- [Install and configure Kata Containers](#install-and-configure-kata-containers)
	- [Build Kata Containers kernel with GPU support](#build-kata-containers-kernel-with-gpu-support)
	- [Nvidia GPU pass-through mode with Kata Containers](#nvidia-gpu-pass-through-mode-with-kata-containers)
	- [Nvidia vGPU mode with Kata Containers](#nvidia-vgpu-mode-with-kata-containers)
	- [Install Nvidia Driver in Kata Containers](#install-nvidia-driver-in-kata-containers)
	- [References](#references)


An Nvidia GPU device can be passed to a Kata Containers container using GPU passthrough
(Nvidia GPU pass-through mode) as well as GPU mediated passthrough (Nvidia vGPU mode). 

Nvidia GPU pass-through mode, an entire physical GPU is directly assigned to one VM,
bypassing the Nvidia Virtual GPU Manager. In this mode of operation, the GPU is accessed
exclusively by the Nvidia driver running in the VM to which it is assigned.
The GPU is not shared among VMs.

Nvidia Virtual GPU (vGPU) enables multiple virtual machines (VMs) to have simultaneous,
direct access to a single physical GPU, using the same Nvidia graphics drivers that are
deployed on non-virtualized operating systems. By doing this, Nvidia vGPU provides VMs
with unparalleled graphics performance, compute performance, and application compatibility,
together with the cost-effectiveness and scalability brought about by sharing a GPU
among multiple workloads.

| Technology | Description | Behaviour | Detail |
| --- | --- | --- | --- |
| Nvidia GPU pass-through mode | GPU passthrough | Physical GPU assigned to a single VM | Direct GPU assignment to VM without limitation |
| Nvidia vGPU mode | GPU sharing | Physical GPU shared by multiple VMs | Mediated passthrough |

## Hardware Requirements
Nvidia GPUs Recommended for Virtualization:

- Nvidia Tesla (T4, M10, P6, V100 or newer)
- Nvidia Quadro RTX 6000/8000

## Host BIOS Requirements

Some hardware requires a larger PCI BARs window, for example, Nvidia Tesla P100, K40m
```
$ lspci -s 04:00.0 -vv | grep Region
      Region 0: Memory at c6000000 (32-bit, non-prefetchable) [size=16M]
      Region 1: Memory at 383800000000 (64-bit, prefetchable) [size=16G] #above 4G
      Region 3: Memory at 383c00000000 (64-bit, prefetchable) [size=32M]
```

For large BARs devices, MMIO mapping above 4G address space should be `enabled`
in the PCI configuration of the BIOS.

Some hardware vendors use different name in BIOS, such as:

- Above 4G Decoding
- Memory Hole for PCI MMIO
- Memory Mapped I/O above 4GB

The following steps outline the workflow for using an Nvidia GPU with Kata.

## Host Kernel Requirements
The following configurations need to be enabled on your host kernel:

- `CONFIG_VFIO`
- `CONFIG_VFIO_IOMMU_TYPE1`
- `CONFIG_VFIO_MDEV`
- `CONFIG_VFIO_MDEV_DEVICE`
- `CONFIG_VFIO_PCI`

Your host kernel needs to be booted with `intel_iommu=on` on the kernel command line.

## Install and configure Kata Containers
To use non-large BARs devices (for example, Nvidia Tesla T4), you need Kata version 1.3.0 or above.
Follow the [Kata Containers setup instructions](../install/README.md)
to install the latest version of Kata.

The following configuration in the Kata `configuration.toml` file as shown below can work:
```
machine_type = "pc"

hotplug_vfio_on_root_bus = true
```

To use large BARs devices (for example, Nvidia Tesla P100), you need Kata version 1.11.0 or above.

The following configuration in the Kata `configuration.toml` file as shown below can work:

Hotplug for PCI devices by `shpchp` (Linux's SHPC PCI Hotplug driver):
```
machine_type = "q35"

hotplug_vfio_on_root_bus = false
```

Hotplug for PCIe devices by `pciehp` (Linux's PCIe Hotplug driver):
```
machine_type = "q35"

hotplug_vfio_on_root_bus = true
pcie_root_port = 1
```

## Build Kata Containers kernel with GPU support
The default guest kernel installed with Kata Containers does not provide GPU support.
To use an Nvidia GPU with Kata Containers, you need to build a kernel with the
necessary GPU support.

The following kernel config options need to be enabled:
```
# Support PCI/PCIe device hotplug (Required for large BARs device)
CONFIG_HOTPLUG_PCI_PCIE=y
CONFIG_HOTPLUG_PCI_SHPC=y

# Support for loading modules (Required for load Nvidia drivers)
CONFIG_MODULES=y
CONFIG_MODULE_UNLOAD=y

# Enable the MMIO access method for PCIe devices (Required for large BARs device)
CONFIG_PCI_MMCONFIG=y
```

The following kernel config options need to be disabled:
```
# Disable Open Source Nvidia driver nouveau
# It conflicts with Nvidia official driver
CONFIG_DRM_NOUVEAU=n
```
> **Note**: `CONFIG_DRM_NOUVEAU` is normally disabled by default.
It is worth checking that it is not enabled in your kernel configuration to prevent any conflicts.


Build the Kata Containers kernel with the previous config options,
using the instructions described in [Building Kata Containers kernel](../../tools/packaging/kernel).
For further details on building and installing guest kernels,
see [the developer guide](../Developer-Guide.md#install-guest-kernel-images).

There is an easy way to build a guest kernel that supports Nvidia GPU:
```
## Build guest kernel with ../../tools/packaging/kernel

# Prepare (download guest kernel source, generate .config)
$ ./build-kernel.sh -v 4.19.86 -g nvidia -f setup

# Build guest kernel
$ ./build-kernel.sh -v 4.19.86 -g nvidia build

# Install guest kernel
$ sudo -E ./build-kernel.sh -v 4.19.86 -g nvidia install
/usr/share/kata-containers/vmlinux-nvidia-gpu.container -> vmlinux-4.19.86-70-nvidia-gpu
/usr/share/kata-containers/vmlinuz-nvidia-gpu.container -> vmlinuz-4.19.86-70-nvidia-gpu
```

To build Nvidia Driver in Kata container, `kernel-devel` is required.  
This is a way to generate rpm packages for `kernel-devel`:
```
$ cd kata-linux-4.19.86-68
$ make rpm-pkg
Output RPMs:
~/rpmbuild/RPMS/x86_64/kernel-devel-4.19.86_nvidia_gpu-1.x86_64.rpm
```
> **Note**:
> - `kernel-devel` should be installed in Kata container before run Nvidia driver installer.
> - Run `make deb-pkg` to build the deb package.

Before using the new guest kernel, please update the `kernel` parameters in `configuration.toml`.
```
kernel = "/usr/share/kata-containers/vmlinuz-nvidia-gpu.container"
```

## Nvidia GPU pass-through mode with Kata Containers
Use the following steps to pass an Nvidia GPU device in pass-through mode with Kata:

1. Find the Bus-Device-Function (BDF) for GPU device on host:
   ```
   $ sudo lspci -nn -D | grep -i nvidia
   0000:04:00.0 3D controller [0302]: NVIDIA Corporation Device [10de:15f8] (rev a1)
   0000:84:00.0 3D controller [0302]: NVIDIA Corporation Device [10de:15f8] (rev a1)
   ```
   > PCI address `0000:04:00.0` is assigned to the hardware GPU device.
   > `10de:15f8` is the device ID of the hardware GPU device.

2. Find the IOMMU group for the GPU device:
   ```
   $ BDF="0000:04:00.0"
   $ readlink -e /sys/bus/pci/devices/$BDF/iommu_group
   /sys/kernel/iommu_groups/45
   ```
   The previous output shows that the GPU belongs to IOMMU group 45.

3. Check the IOMMU group number under `/dev/vfio`:
   ```
   $ ls -l /dev/vfio
   total 0
   crw------- 1 root root 248,   0 Feb 28 09:57 45
   crw------- 1 root root 248,   1 Feb 28 09:57 54
   crw-rw-rw- 1 root root  10, 196 Feb 28 09:57 vfio
   ```

4. Start a Kata container with GPU device:
   ```
   $ sudo docker run -it --runtime=kata-runtime --cap-add=ALL --device /dev/vfio/45 centos /bin/bash
   ```

5. Run `lspci` within the container to verify the GPU device is seen in the list
   of the PCI devices. Note the vendor-device id of the GPU (`10de:15f8`) in the `lspci` output.
   ```
   $ lspci -nn -D | grep '10de:15f8'
   0000:01:01.0 3D controller [0302]: NVIDIA Corporation GP100GL [Tesla P100 PCIe 16GB] [10de:15f8] (rev a1)
   ```

6. Additionally, you can check the PCI BARs space of ​​the Nvidia GPU device in the container:
   ```
   $ lspci -s 01:01.0 -vv | grep Region
		Region 0: Memory at c0000000 (32-bit, non-prefetchable) [disabled] [size=16M]
		Region 1: Memory at 4400000000 (64-bit, prefetchable) [disabled] [size=16G]
		Region 3: Memory at 4800000000 (64-bit, prefetchable) [disabled] [size=32M]
   ```
   > **Note**: If you see a message similar to the above, the BAR space of the Nvidia
   > GPU has been successfully allocated.

## Nvidia vGPU mode with Kata Containers

Nvidia vGPU is a licensed product on all supported GPU boards. A software license
is required to enable all vGPU features within the guest VM.

> **Note**: There is no suitable test environment, so it is not written here.


## Install Nvidia Driver in Kata Containers
Download the official Nvidia driver from
[https://www.nvidia.com/Download/index.aspx](https://www.nvidia.com/Download/index.aspx),
for example `NVIDIA-Linux-x86_64-418.87.01.run`.

Install the `kernel-devel`(generated in the previous steps) for guest kernel:
```
$ sudo rpm -ivh kernel-devel-4.19.86_gpu-1.x86_64.rpm
```

Here is an example to extract, compile and install Nvidia driver:
```
## Extract
$ sh ./NVIDIA-Linux-x86_64-418.87.01.run -x

## Compile and install (It will take some time)
$ cd NVIDIA-Linux-x86_64-418.87.01
$ sudo ./nvidia-installer -a -q --ui=none \
 --no-cc-version-check \
 --no-opengl-files --no-install-libglvnd \
 --kernel-source-path=/usr/src/kernels/`uname -r`
```

Or just run one command line:
```
$ sudo sh ./NVIDIA-Linux-x86_64-418.87.01.run -a -q --ui=none \
 --no-cc-version-check \
 --no-opengl-files --no-install-libglvnd \
 --kernel-source-path=/usr/src/kernels/`uname -r`
```

To view detailed logs of the installer:
```
$ tail -f /var/log/nvidia-installer.log
```

Load Nvidia driver module manually
```
# Optional（generate modules.dep and map files for Nvidia driver）
$ sudo depmod

# Load module
$ sudo modprobe nvidia-drm

# Check module
$ lsmod | grep nvidia
nvidia_drm             45056  0
nvidia_modeset       1093632  1 nvidia_drm
nvidia              18202624  1 nvidia_modeset
drm_kms_helper        159744  1 nvidia_drm
drm                   364544  3 nvidia_drm,drm_kms_helper
i2c_core               65536  3 nvidia,drm_kms_helper,drm
ipmi_msghandler        49152  1 nvidia
```


Check Nvidia device status with `nvidia-smi`
```
$ nvidia-smi
Tue Mar  3 00:03:49 2020
+-----------------------------------------------------------------------------+
| NVIDIA-SMI 418.87.01    Driver Version: 418.87.01    CUDA Version: 10.1     |
|-------------------------------+----------------------+----------------------+
| GPU  Name        Persistence-M| Bus-Id        Disp.A | Volatile Uncorr. ECC |
| Fan  Temp  Perf  Pwr:Usage/Cap|         Memory-Usage | GPU-Util  Compute M. |
|===============================+======================+======================|
|   0  Tesla P100-PCIE...  Off  | 00000000:01:01.0 Off |                    0 |
| N/A   27C    P0    25W / 250W |      0MiB / 16280MiB |      0%      Default |
+-------------------------------+----------------------+----------------------+

+-----------------------------------------------------------------------------+
| Processes:                                                       GPU Memory |
|  GPU       PID   Type   Process name                             Usage      |
|=============================================================================|
|  No running processes found                                                 |
+-----------------------------------------------------------------------------+

```

## References

- [Configuring a VM for GPU Pass-Through by Using the QEMU Command Line](https://docs.nvidia.com/grid/latest/grid-vgpu-user-guide/index.html#using-gpu-pass-through-red-hat-el-qemu-cli)
- https://gitlab.com/nvidia/container-images/driver/-/tree/master
- https://github.com/NVIDIA/nvidia-docker/wiki/Driver-containers-(Beta)
