# Using NVIDIA GPU device with Kata Containers

An NVIDIA GPU device can be passed to a Kata Containers container using GPU
passthrough (NVIDIA GPU pass-through mode) as well as GPU mediated passthrough
(NVIDIA vGPU mode).

NVIDIA GPU pass-through mode, an entire physical GPU is directly assigned to one
VM, bypassing the NVIDIA Virtual GPU Manager. In this mode of operation, the GPU
is accessed exclusively by the NVIDIA driver running in the VM to which it is
assigned. The GPU is not shared among VMs.

NVIDIA Virtual GPU (vGPU) enables multiple virtual machines (VMs) to have
simultaneous, direct access to a single physical GPU, using the same NVIDIA
graphics drivers that are deployed on non-virtualized operating systems. By
doing this, NVIDIA vGPU provides VMs with unparalleled graphics performance,
compute performance, and application compatibility, together with the
cost-effectiveness and scalability brought about by sharing a GPU among multiple
workloads. A vGPU can be either time-sliced or Multi-Instance GPU (MIG)-backed
with [MIG-slices](https://docs.nvidia.com/datacenter/tesla/mig-user-guide/).

| Technology | Description | Behavior | Detail |
| --- | --- | --- | --- |
| NVIDIA GPU pass-through mode | GPU passthrough | Physical GPU assigned to a single VM | Direct GPU assignment to VM without limitation |
| NVIDIA vGPU time-sliced | GPU time-sliced | Physical GPU time-sliced for multiple VMs | Mediated passthrough |
| NVIDIA vGPU MIG-backed | GPU with MIG-slices | Physical GPU MIG-sliced for multiple VMs | Mediated passthrough |

## Hardware Requirements

NVIDIA GPUs Recommended for Virtualization:

- NVIDIA Tesla (T4, M10, P6, V100 or newer)
- NVIDIA Quadro RTX 6000/8000

## Host BIOS Requirements

Some hardware requires a larger PCI BARs window, for example, NVIDIA Tesla P100,
K40m

```sh
$ lspci -s d0:00.0 -vv | grep Region
        Region 0: Memory at e7000000 (32-bit, non-prefetchable) [size=16M]
        Region 1: Memory at 222800000000 (64-bit, prefetchable) [size=32G] # Above 4G
        Region 3: Memory at 223810000000 (64-bit, prefetchable) [size=32M]
```

For large BARs devices, MMIO mapping above 4G address space should be `enabled`
in the PCI configuration of the BIOS.

Some hardware vendors use different name in BIOS, such as:

- Above 4G Decoding
- Memory Hole for PCI MMIO
- Memory Mapped I/O above 4GB

If one is using a GPU based on the Ampere architecture and later additionally
SR-IOV needs to be enabled for the vGPU use-case.

The following steps outline the workflow for using an NVIDIA GPU with Kata.

## Host Kernel Requirements

The following configurations need to be enabled on your host kernel:

- `CONFIG_VFIO`
- `CONFIG_VFIO_IOMMU_TYPE1`
- `CONFIG_VFIO_MDEV`
- `CONFIG_VFIO_MDEV_DEVICE`
- `CONFIG_VFIO_PCI`

Your host kernel needs to be booted with `intel_iommu=on` on the kernel command
line.

## Install and configure Kata Containers

To use non-large BARs devices (for example, NVIDIA Tesla T4), you need Kata
version 1.3.0 or above. Follow the [Kata Containers setup
instructions](../install/README.md) to install the latest version of Kata.

To use large BARs devices (for example, NVIDIA Tesla P100), you need Kata
version 1.11.0 or above.

The following configuration in the Kata `configuration.toml` file as shown below
can work:

Hotplug for PCI devices with small BARs by `acpi_pcihp` (Linux's ACPI PCI
Hotplug driver):

```sh
machine_type = "q35"

hotplug_vfio_on_root_bus = false
```

Hotplug for PCIe devices with large BARs by `pciehp` (Linux's PCIe Hotplug
driver):

```sh
machine_type = "q35"

hotplug_vfio_on_root_bus = true
pcie_root_port = 1
```

## Build Kata Containers kernel with GPU support

The default guest kernel installed with Kata Containers does not provide GPU
support. To use an NVIDIA GPU with Kata Containers, you need to build a kernel
with the necessary GPU support.

The following kernel config options need to be enabled:

```sh
# Support PCI/PCIe device hotplug (Required for large BARs device)
CONFIG_HOTPLUG_PCI_PCIE=y

# Support for loading modules (Required for load NVIDIA drivers)
CONFIG_MODULES=y
CONFIG_MODULE_UNLOAD=y

# Enable the MMIO access method for PCIe devices (Required for large BARs device)
 CONFIG_PCI_MMCONFIG=y
```

The following kernel config options need to be disabled:

```sh
# Disable Open Source NVIDIA driver nouveau
# It conflicts with NVIDIA official driver
CONFIG_DRM_NOUVEAU=n
```

> **Note**: `CONFIG_DRM_NOUVEAU` is normally disabled by default.
It is worth checking that it is not enabled in your kernel configuration to
prevent any conflicts.

Build the Kata Containers kernel with the previous config options, using the
instructions described in [Building Kata Containers
kernel](../../tools/packaging/kernel). For further details on building and
installing guest kernels, see [the developer
guide](../Developer-Guide.md#install-guest-kernel-images).

There is an easy way to build a guest kernel that supports NVIDIA GPU:

```sh
## Build guest kernel with ../../tools/packaging/kernel

# Prepare (download guest kernel source, generate .config)
$ ./build-kernel.sh -v 5.15.23 -g nvidia -f setup

# Build guest kernel
$ ./build-kernel.sh -v 5.15.23 -g nvidia build

# Install guest kernel
$ sudo -E ./build-kernel.sh -v 5.15.23 -g nvidia install
```

To build NVIDIA Driver in Kata container, `linux-headers` is required.
This is a way to generate deb packages for `linux-headers`:

> **Note**:
> Run `make rpm-pkg` to build the rpm package.
> Run `make deb-pkg` to build the deb package.
>

```sh
$ cd kata-linux-5.15.23-89
$ make deb-pkg
```
Before using the new guest kernel, please update the `kernel` parameters in
 `configuration.toml`.

```sh
kernel = "/usr/share/kata-containers/vmlinuz-nvidia-gpu.container"
```

## NVIDIA GPU pass-through mode with Kata Containers

Use the following steps to pass an NVIDIA GPU device in pass-through mode with Kata:

1. Find the Bus-Device-Function (BDF) for GPU device on host:

   ```sh
   $ sudo lspci -nn -D | grep -i nvidia
   0000:d0:00.0 3D controller [0302]: NVIDIA Corporation Device [10de:20b9] (rev a1)
   ```

   > PCI address `0000:d0:00.0` is assigned to the hardware GPU device.
   > `10de:20b9` is the device ID of the hardware GPU device.

2. Find the IOMMU group for the GPU device:

   ```sh
   $ BDF="0000:d0:00.0"
   $ readlink -e /sys/bus/pci/devices/$BDF/iommu_group
   ```

   The previous output shows that the GPU belongs to IOMMU group 192. The next
   step is to bind the GPU to the VFIO-PCI driver.

   ```sh
   $ BDF="0000:d0:00.0"
   $ DEV="/sys/bus/pci/devices/$BDF"
   $ echo "vfio-pci" > $DEV/driver_override
   $ echo $BDF > $DEV/driver/unbind
   $ echo $BDF > /sys/bus/pci/drivers_probe
   # To return the device to the standard driver, we simply clear the
   # driver_override and reprobe the device, ex:
   $ echo > $DEV/preferred_driver
   $ echo $BDF > $DEV/driver/unbind
   $ echo $BDF > /sys/bus/pci/drivers_probe
   ```

3. Check the IOMMU group number under `/dev/vfio`:

   ```sh
   $ ls -l /dev/vfio
   total 0
   crw------- 1 zvonkok zvonkok 243,   0 Mar 18 03:06 192
   crw-rw-rw- 1 root    root     10, 196 Mar 18 02:27 vfio
   ```

4. Start a Kata container with GPU device:

   ```sh
   # You may need to `modprobe vhost-vsock` if you get
   # host system doesn't support vsock: stat /dev/vhost-vsock
   $ sudo ctr --debug run --runtime "io.containerd.kata.v2"  --device /dev/vfio/192  --rm -t  "docker.io/library/archlinux:latest" arch uname -r
   ```

5. Run `lspci` within the container to verify the GPU device is seen in the list
   of the PCI devices. Note the vendor-device id of the GPU (`10de:20b9`) in the `lspci` output.

   ```sh
   $ sudo ctr --debug run --runtime "io.containerd.kata.v2"  --device /dev/vfio/192  --rm -t  "docker.io/library/archlinux:latest" arch sh -c "lspci -nn | grep '10de:20b9'"
   ```

6. Additionally, you can check the PCI BARs space of ​​the NVIDIA GPU device in the container:

   ```sh
   $ sudo ctr --debug run --runtime "io.containerd.kata.v2"  --device /dev/vfio/192  --rm -t  "docker.io/library/archlinux:latest" arch sh -c "lspci -s 02:00.0 -vv | grep Region"
   ```

   > **Note**: If you see a message similar to the above, the BAR space of the NVIDIA
   > GPU has been successfully allocated.

## NVIDIA vGPU mode with Kata Containers

NVIDIA vGPU is a licensed product on all supported GPU boards. A software license
is required to enable all vGPU features within the guest VM.

> **TODO**: Will follow up with instructions

## Install NVIDIA Driver + Toolkit in Kata Containers Guest OS

Consult the [Developer-Guide](https://github.com/kata-containers/kata-containers/blob/main/docs/Developer-Guide.md#create-a-rootfs-image) on how to create a
rootfs base image for a distribution of your choice. This is going to be used as
a base for a NVIDIA enabled guest OS. Use the `EXTRA_PKGS` variable to install
all the needed packages to compile the drivers. Also copy the kernel development
packages from the previous `make deb-pkg` into `$ROOTFS_DIR`.

```sh
export EXTRA_PKGS="gcc make curl gnupg"
```

Having the `$ROOTFS_DIR` exported in the previous step we can now install all the
need parts in the guest OS. In this case we have an Ubuntu based rootfs.

First off all mount the special filesystems into the rootfs

```sh
$ sudo mount -t sysfs -o ro none ${ROOTFS_DIR}/sys
$ sudo mount -t proc -o ro none ${ROOTFS_DIR}/proc
$ sudo mount -t tmpfs none ${ROOTFS_DIR}/tmp
$ sudo mount -o bind,ro /dev ${ROOTFS_DIR}/dev
$ sudo mount -t devpts none ${ROOTFS_DIR}/dev/pts
```

Now we can enter `chroot`

```sh
$ sudo chroot ${ROOTFS_DIR}
```

Inside the rootfs one is going to install the drivers and toolkit to enable easy
creation of GPU containers with Kata. We can also use this rootfs for any other
container not specifically only for GPUs.

As a prerequisite install the copied kernel development packages

```sh
$ sudo dpkg -i *.deb
```

Get the driver run file, since we need to build the driver against a kernel that
is not running on the host we need the ability to specify the exact version we
want the driver to build against. Take the kernel version one used for building
the NVIDIA kernel (`5.15.23-nvidia-gpu`).

```sh
$ wget https://us.download.nvidia.com/XFree86/Linux-x86_64/510.54/NVIDIA-Linux-x86_64-510.54.run
$ chmod +x NVIDIA-Linux-x86_64-510.54.run
# Extract the source files so we can run the installer with arguments
$ ./NVIDIA-Linux-x86_64-510.54.run -x
$ cd NVIDIA-Linux-x86_64-510.54
$ ./nvidia-installer -k 5.15.23-nvidia-gpu
```
Having the drivers installed we need to install the toolkit which will take care
of providing the right bits into the container.

```sh
$ distribution=$(. /etc/os-release;echo $ID$VERSION_ID)
$ curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | sudo gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
$ curl -s -L https://nvidia.github.io/libnvidia-container/$distribution/libnvidia-container.list | sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' | sudo tee /etc/apt/sources.list.d/nvidia-container-toolkit.list
$ apt update
$ apt install nvidia-container-toolkit
```

Create the hook execution file for Kata:

```
# Content of $ROOTFS_DIR/usr/share/oci/hooks/prestart/nvidia-container-toolkit.sh

#!/bin/bash -x

/usr/bin/nvidia-container-toolkit -debug $@
```

As a last step one can do some cleanup of files or package caches. Build the
rootfs and configure it for use with Kata according to the development guide.

Enable the `guest_hook_path` in Kata's `configuration.toml`

```sh
guest_hook_path = "/usr/share/oci/hooks"
```

One has build a NVIDIA rootfs, kernel and now we can run any GPU container
without installing the drivers into the container. Check NVIDIA device status
with `nvidia-smi`

```sh
$  sudo ctr --debug run --runtime "io.containerd.kata.v2"  --device /dev/vfio/192  --rm -t "docker.io/nvidia/cuda:11.6.0-base-ubuntu20.04" cuda nvidia-smi
Fri Mar 18 10:36:59 2022
+-----------------------------------------------------------------------------+
| NVIDIA-SMI 510.54       Driver Version: 510.54       CUDA Version: 11.6     |
|-------------------------------+----------------------+----------------------+
| GPU  Name        Persistence-M| Bus-Id        Disp.A | Volatile Uncorr. ECC |
| Fan  Temp  Perf  Pwr:Usage/Cap|         Memory-Usage | GPU-Util  Compute M. |
|                               |                      |               MIG M. |
|===============================+======================+======================|
|   0  NVIDIA A30X         Off  | 00000000:02:00.0 Off |                    0 |
| N/A   38C    P0    67W / 230W |      0MiB / 24576MiB |      0%      Default |
|                               |                      |             Disabled |
+-------------------------------+----------------------+----------------------+

+-----------------------------------------------------------------------------+
| Processes:                                                                  |
|  GPU   GI   CI        PID   Type   Process name                  GPU Memory |
|        ID   ID                                                   Usage      |
|=============================================================================|
|  No running processes found                                                 |
+-----------------------------------------------------------------------------+
```

As a last step one can remove the additional packages and files that were added
to the `$ROOTFS_DIR` to keep it as small as possible.

## References

- [Configuring a VM for GPU Pass-Through by Using the QEMU Command Line](https://docs.nvidia.com/grid/latest/grid-vgpu-user-guide/index.html#using-gpu-pass-through-red-hat-el-qemu-cli)
- https://gitlab.com/nvidia/container-images/driver/-/tree/master
- https://github.com/NVIDIA/nvidia-docker/wiki/Driver-containers
