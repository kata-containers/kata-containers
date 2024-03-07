# Using NVIDIA GPU device with Kata Containers

An NVIDIA GPU device can be passed to a Kata Containers container using GPU
passthrough (NVIDIA GPU pass-through mode) as well as GPU mediated passthrough
(NVIDIA `vGPU` mode).

NVIDIA GPU pass-through mode, an entire physical GPU is directly assigned to one
VM, bypassing the NVIDIA Virtual GPU Manager. In this mode of operation, the GPU
is accessed exclusively by the NVIDIA driver running in the VM to which it is
assigned. The GPU is not shared among VMs.

NVIDIA Virtual GPU (`vGPU`) enables multiple virtual machines (VMs) to have
simultaneous, direct access to a single physical GPU, using the same NVIDIA
graphics drivers that are deployed on non-virtualized operating systems. By
doing this, NVIDIA `vGPU` provides VMs with unparalleled graphics performance,
compute performance, and application compatibility, together with the
cost-effectiveness and scalability brought about by sharing a GPU among multiple
workloads. A `vGPU` can be either time-sliced or Multi-Instance GPU (MIG)-backed
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

Some hardware vendors use a different name in BIOS, such as:

- Above 4G Decoding
- Memory Hole for PCI MMIO
- Memory Mapped I/O above 4GB

If one is using a GPU based on the Ampere architecture and later additionally
SR-IOV needs to be enabled for the `vGPU` use-case.

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

However, there are special cases like `Dragonball VMM`. It directly supports device `hot-plug/hot-unplug`
via upcall to avoid `ACPI` virtualization and minimize `VM` overhead. Since upcall isn't upstream kernel
code, using `Dragonball VMM` for NVIDIA GPU `hot-plug/hot-unplug` requires applying the Upcall patchset in
addition to the above kernel configuration items. Follow these steps to build for NVIDIA GPU `hot-[un]plug`
for `Dragonball`:

```sh 
# Prepare .config to support both upcall and nvidia gpu 
$ ./build-kernel.sh -v 5.10.25 -e -t dragonball -g nvidia -f setup

# Build guest kernel to support both upcall and nvidia gpu 
$ ./build-kernel.sh -v 5.10.25 -e -t dragonball -g nvidia build

# Install guest kernel to support both upcall and nvidia gpu
$ sudo -E ./build-kernel.sh -v 5.10.25 -e -t dragonball -g nvidia install
```

> **Note**:
> - `-v 5.10.25` is for the guest kernel version.
> - `-e` here means experimental, mainly because `upcall` patches are not in upstream Linux kernel.
> - `-t dragonball` is for specifying hypervisor type.
> - `-f` is for generating `.config` file.

To build NVIDIA Driver in Kata container, `linux-headers` are required.
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

1. Find the Bus-Device-Function (BDF) for the GPU device on the host:

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

4. Start a Kata container with the GPU device:

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
is required to enable all vGPU features within the guest VM. NVIDIA vGPU manager
needs to be installed on the host to configure GPUs in vGPU mode. See [NVIDIA Virtual GPU Software Documentation v14.0 through 14.1](https://docs.nvidia.com/grid/14.0/) for more details.

### NVIDIA vGPU time-sliced

In the time-sliced mode, the GPU is not partitioned and the workload uses the
whole GPU and shares access to the GPU engines. Processes are scheduled in
series. The best effort scheduler is the default one and can be exchanged by
other scheduling policies see the documentation above how to do that.

Beware if you had `MIG` enabled before to disable `MIG` on the GPU if you want
to use `time-sliced` `vGPU`.

```sh
$ sudo nvidia-smi -mig 0
```

Enable the virtual functions for the physical GPU in the `sysfs` file system.

```sh
$ sudo /usr/lib/nvidia/sriov-manage -e 0000:41:00.0
```

Get the `BDF` of the available virtual function on the GPU, and choose one for the
following steps.

```sh
$ cd /sys/bus/pci/devices/0000:41:00.0/
$ ls -l |  grep virtfn
```

#### List all available vGPU instances

The following shell snippet will walk the `sysfs` and only print instances
that are available, that can be created.

```sh
# The 00.0 is often the PF of the device the VFs will have the funciont in the
# BDF incremented by some values so e.g. the very first VF is 0000:41:00.4

cd /sys/bus/pci/devices/0000:41:00.0/

for vf in $(ls -d virtfn*)
do
        BDF=$(basename $(readlink -f $vf))
        for md in $(ls -d $vf/mdev_supported_types/*)
        do
                AVAIL=$(cat $md/available_instances)
                NAME=$(cat $md/name)
                DIR=$(basename $md)

                if [ $AVAIL -gt 0 ]; then
                        echo "| BDF          | INSTANCES | NAME           | DIR        |"
                        echo "+--------------+-----------+----------------+------------+"
                        printf "| %12s |%10d |%15s | %10s |\n\n" "$BDF" "$AVAIL" "$NAME" "$DIR"
                fi

        done
done
```

If there are available instances you get something like this (for the first VF),
beware that the output is highly dependent on the GPU you have, if there is no
output check again if `MIG` is really disabled.

```sh
| BDF          | INSTANCES | NAME           | DIR        |
+--------------+-----------+----------------+------------+
| 0000:41:00.4 |         1 |  GRID A100D-4C | nvidia-692 |

| BDF          | INSTANCES | NAME           | DIR        |
+--------------+-----------+----------------+------------+
| 0000:41:00.4 |         1 |  GRID A100D-8C | nvidia-693 |

| BDF          | INSTANCES | NAME           | DIR        |
+--------------+-----------+----------------+------------+
| 0000:41:00.4 |         1 | GRID A100D-10C | nvidia-694 |

| BDF          | INSTANCES | NAME           | DIR        |
+--------------+-----------+----------------+------------+
| 0000:41:00.4 |         1 | GRID A100D-16C | nvidia-695 |

| BDF          | INSTANCES | NAME           | DIR        |
+--------------+-----------+----------------+------------+
| 0000:41:00.4 |         1 | GRID A100D-20C | nvidia-696 |

| BDF          | INSTANCES | NAME           | DIR        |
+--------------+-----------+----------------+------------+
| 0000:41:00.4 |         1 | GRID A100D-40C | nvidia-697 |

| BDF          | INSTANCES | NAME           | DIR        |
+--------------+-----------+----------------+------------+
| 0000:41:00.4 |         1 | GRID A100D-80C | nvidia-698 |

```

Change to the `mdev_supported_types` directory for the virtual function on which
you want to create the `vGPU`. Taking the first output as an example:

```sh
$ cd virtfn0/mdev_supported_types/nvidia-692
$ UUIDGEN=$(uuidgen)
$ sudo bash -c "echo $UUIDGEN > create"
```

Confirm that the `vGPU` was created. You should see the `UUID` pointing to a
subdirectory of the `sysfs` space.

```sh
$ ls -l /sys/bus/mdev/devices/
```

Get the `IOMMU` group number and verify there is a `VFIO` device created to use
with Kata.

```sh
$ ls -l /sys/bus/mdev/devices/*/
$ ls -l /dev/vfio
```

Use the `VFIO` device created in the same way as in the pass-through use-case.
Beware that the guest needs the NVIDIA guest drivers, so one would need to build
a new guest `OS` image.

### NVIDIA vGPU MIG-backed

We're not going into detail what `MIG` is but briefly it is a technology to
partition the hardware into independent instances with guaranteed quality of
service. For more details see [NVIDIA Multi-Instance GPU User Guide](https://docs.nvidia.com/datacenter/tesla/mig-user-guide/).

First enable `MIG` mode for a GPU, depending on the platform you're running
a reboot would be necessary. Some platforms support GPU reset.

```sh
$ sudo nvidia-smi -mig 1
```

If the platform supports a GPU reset one can run, otherwise you will get a
warning to reboot the server.

```sh
$ sudo nvidia-smi --gpu-reset
```

The driver per default provides a number of profiles that users can opt-in when
configuring the MIG feature.

```sh
$ sudo nvidia-smi mig -lgip
+-----------------------------------------------------------------------------+
| GPU instance profiles:                                                      |
| GPU   Name             ID    Instances   Memory     P2P    SM    DEC   ENC  |
|                              Free/Total   GiB              CE    JPEG  OFA  |
|=============================================================================|
|   0  MIG 1g.10gb       19     7/7        9.50       No     14     0     0   |
|                                                             1     0     0   |
+-----------------------------------------------------------------------------+
|   0  MIG 1g.10gb+me    20     1/1        9.50       No     14     1     0   |
|                                                             1     1     1   |
+-----------------------------------------------------------------------------+
|   0  MIG 2g.20gb       14     3/3        19.50      No     28     1     0   |
|                                                             2     0     0   |
+-----------------------------------------------------------------------------+
                              ...
```

Create the GPU instances that correspond to the `vGPU` types of the `MIG-backed`
`vGPUs` that you will create [NVIDIA A100 PCIe 80GB Virtual GPU Types](https://docs.nvidia.com/grid/13.0/grid-vgpu-user-guide/index.html#vgpu-types-nvidia-a100-pcie-80gb).

```sh
# MIG 1g.10gb --> vGPU A100D-1-10C
$ sudo nvidia-smi mig -cgi 19
```

List the GPU instances and get the GPU instance id to create the compute
instance.

```sh
$ sudo nvidia-smi mig -lgi # list the created GPU instances
$ sudo nvidia-smi mig -cci -gi 9 # each GPU instance can have several compute
                                 # instances. Instance -> Workload
```

Verify that the compute instances were created within the GPU instance

```sh
$ nvidia-smi
                              ... snip ...
+-----------------------------------------------------------------------------+
| MIG devices:                                                                |
+------------------+----------------------+-----------+-----------------------+
| GPU  GI  CI  MIG |         Memory-Usage |        Vol|         Shared        |
|      ID  ID  Dev |           BAR1-Usage | SM     Unc| CE  ENC  DEC  OFA  JPG|
|                  |                      |        ECC|                       |
|==================+======================+===========+=======================|
|  0    9   0   0  |      0MiB /  9728MiB | 14      0 |  1   0    0    0    0 |
|                  |      0MiB /  4095MiB |           |                       |
+------------------+----------------------+-----------+-----------------------+
                              ... snip ...
```

We can use the [snippet](#list-all-available-vgpu-instances) from before to list
the available `vGPU` instances, this time `MIG-backed`.

```sh
| BDF          | INSTANCES | NAME           | DIR        |
+--------------+-----------+----------------+------------+
| 0000:41:00.4 |         1 |GRID A100D-1-10C | nvidia-699 |

| BDF          | INSTANCES | NAME           | DIR        |
+--------------+-----------+----------------+------------+
| 0000:41:00.5 |         1 |GRID A100D-1-10C | nvidia-699 |

| BDF          | INSTANCES | NAME           | DIR        |
+--------------+-----------+----------------+------------+
| 0000:41:01.6 |         1 |GRID A100D-1-10C | nvidia-699 |
                       ... snip ...
```

Repeat the steps after the [snippet](#list-all-available-vgpu-instances) listing
to create the corresponding `mdev` device and use the guest `OS` created in the
previous section with `time-sliced` `vGPUs`.

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
needed parts in the guest OS. In this case, we have an Ubuntu based rootfs.

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

Inside the rootfs one is going to install the drivers and toolkit to enable the
easy creation of GPU containers with Kata. We can also use this rootfs for any
other container not specifically only for GPUs.

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

Make sure the hook shell is executable:

```sh
chmod +x $ROOTFS_DIR/usr/share/oci/hooks/prestart/nvidia-container-toolkit.sh
```

As the last step one can do some cleanup of files or package caches. Build the
rootfs and configure it for use with Kata according to the development guide.

Enable the `guest_hook_path` in Kata's `configuration.toml`

```sh
guest_hook_path = "/usr/share/oci/hooks"
```

One has built a NVIDIA rootfs, kernel and now we can run any GPU container
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

As the last step one can remove the additional packages and files that were added
to the `$ROOTFS_DIR` to keep it as small as possible.

## References

- [Configuring a VM for GPU Pass-Through by Using the QEMU Command Line](https://docs.nvidia.com/grid/latest/grid-vgpu-user-guide/index.html#using-gpu-pass-through-red-hat-el-qemu-cli)
- https://gitlab.com/nvidia/container-images/driver/-/tree/master
- https://github.com/NVIDIA/nvidia-docker/wiki/Driver-containers
