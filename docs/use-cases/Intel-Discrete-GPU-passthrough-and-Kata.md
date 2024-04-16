# Using Intel Discrete GPU device with Kata Containers

This guide covers the use case for passing Intel Discrete GPUs to Kata.
These include the Intel® Data Center GPU Max Series and Intel® Data Center GPU Flex Series.
For integrated GPUs please refer to [Integrate-Intel-GPUs-with-Kata](Intel-GPU-passthrough-and-Kata.md)

> **Note:** These instructions are for a system that has an x86_64 CPU.

An Intel Discrete GPU can be passed to a Kata Container using GPU passthrough, 
or SR-IOV passthrough.

In Intel GPU pass-through mode, an entire physical GPU is directly assigned to one VM. 
In this mode of operation, the GPU is accessed exclusively by the Intel driver running in
the VM to which it is assigned. The GPU is not shared among VMs.

With SR-IOV mode, it is possible to pass a Virtual GPU instance to a virtual machine.
With this, multiple Virtual GPU instances can be carved out of a single physical GPU 
and be passed to different VMs, allowing the GPU to be shared.

| Technology | Description |
|-|-|
| GPU passthrough | Physical GPU assigned to a single VM |
| SR-IOV passthrough | Physical GPU shared by multiple VMs |

## Hardware Requirements

Intel GPUs Recommended for Virtualization:

- Intel® Data Center GPU Max Series (`Ponte Vecchio`)
- Intel® Data Center GPU Flex Series (`Arctic Sound-M`)
- Intel® Data Center GPU Arc Series 

The following steps outline the workflow for using an Intel Graphics device with Kata Containers.

## Host BIOS requirements

Hardware such as Intel Max and Flex series require larger PCI BARs. 

For large BAR devices, MMIO mapping above the 4GB address space should be enabled in the PCI configuration of the BIOS.

Some hardware vendors use a different name in the BIOS, such as:

- Above 4GB Decoding
- Memory Hole for PCI MMIO
- Memory Mapped I/O above 4GB

## Host Kernel Requirements

For device passthrough to work with the Max and Flex Series, an out of tree kernel driver is required.

For Ubuntu 22.04 server, follow these instructions to install the out of tree GPU driver:
```bash
$ sudo apt update
$ sudo apt install -y gpg-agent wget
$ wget -qO - https://repositories.intel.com/gpu/intel-graphics.key | \
    sudo gpg --dearmor --output /usr/share/keyrings/intel-graphics.gpg
$ source /etc/os-release
$ echo "deb [arch=amd64 signed-by=/usr/share/keyrings/intel-graphics.gpg] https://repositories.intel.com/gpu/ubuntu ${VERSION_CODENAME}/lts/2350 unified" | \
    sudo tee /etc/apt/sources.list.d/intel-gpu-${VERSION_CODENAME}.list
$ sudo apt update
$ sudo apt install -y linux-headers-"$(uname -r)" flex bison intel-fw-gpu intel-i915-dkms xpu-smi
$ sudo reboot
```
For support on other distributions, please refer to [DGPU-docs](https://dgpu-docs.intel.com/driver/installation.html)

You can also install the driver from source which is maintained at [intel-gpu-i915-backports](https://github.com/intel-gpu/intel-gpu-i915-backports)
Detailed instructions for reference can be found at: https://github.com/intel-gpu/intel-gpu-i915-backports/blob/backport/main/docs/README_ubuntu.md.

Below are the steps for installing the driver from source on an Ubuntu 22.04 LTS system:
```bash
$ export I915_BRANCH="backport/main"
$ git clone -b ${I915_BRANCH} --depth 1 https://github.com/intel-gpu/intel-gpu-i915-backports.git
$ cd intel-gpu-i915-backports/
$ sudo apt install -y dkms make debhelper devscripts build-essential flex bison mawk
$ sudo apt install -y linux-headers-"$(uname -r)" linux-image-unsigned-"$(uname -r)"
$ make i915dkmsdeb-pkg
```
The above make command will create Debian package in parent folder:  `intel-i915-dkms_<release version>.<kernel-version>.deb`
Install the package as:
```bash
$ sudo dpkg -i intel-i915-dkms_<release version>.<kernel-version>.deb
$ sudo reboot
```

Additionally, verify that the following kernel configs are enabled for your host kernel:
```
CONFIG_VFIO
CONFIG_VFIO_IOMMU_TYPE1
CONFIG_VFIO_PCI
```

## Host kernel command line 

Your host kernel needs to be booted with `intel_iommu=on` and `i915.enable_iaf=0` on the kernel command
line.

1. Run the following to change the kernel command line using grub:
```bash
$ sudo vim /etc/default/grub
```

2. At the end of the GRUB_CMDLINE_LINUX_DEFAULT append the below line:

`intel_iommu=on iommu=pt i915.max_vfs=63 i915.enable_iaf=0`

3. Update grub as per OS distribution:

For Ubuntu:
```bash
$ sudo update-grub
```

For CentOS/RHEL:
```bash
$ sudo grub2-mkconfig -o /boot/grub2/grub.cfg 
``` 

4. Reboot the system
```bash
$ sudo reboot
```

## Install and configure Kata Containers

To use this feature, you need Kata version 1.3.0 or above.
Follow the [Kata Containers setup instructions](../install/README.md)
to install the latest version of Kata.

To use large BARs devices (for example, NVIDIA Tesla P100), you need Kata version 1.11.0 or above.

In order to pass a GPU to a Kata Container, you need to enable the `hotplug_vfio_on_root_bus`
configuration in the Kata `configuration.toml` file as shown below.

```bash
$ sudo sed -i -e 's/^# *\(hotplug_vfio_on_root_bus\).*=.*$/\1 = true/g' /usr/share/defaults/kata-containers/configuration.toml
```

Make sure you are using the `q35` machine type by verifying `machine_type = "q35"` is
set in the `configuration.toml`. Make sure `pcie_root_port` is set to a positive value.

After making the above changes, configuration in the `configuration.toml` should look like this:
```
machine_type = "q35"

hotplug_vfio_on_root_bus = true
pcie_root_port = 1
```

## GPU passthrough with Kata Containers

Use the following steps to pass an Intel discrete GPU  with Kata:

1. Find the Bus-Device-Function (BDF) for GPU device:

   ```
   $ sudo lspci -nn -D | grep Display
   ```

   Run the previous command to determine the BDF for the GPU device on host.<br/>
   From the previous output, PCI address `0000:29:00.0` is assigned to the hardware GPU device.<br/>
   We choose this BDF to use it later to unbind the GPU device from the host for the purpose of demonstration.<br/>

2. Find the IOMMU group for the GPU device:

   ```bash
   $ BDF="0000:29:00.0"
   $ readlink -e /sys/bus/pci/devices/$BDF/iommu_group
   /sys/kernel/iommu_groups/27
   ```

   The previous output shows that the GPU belongs to IOMMU group 27.

3. Bind the GPU to the `vfio-pci` device driver:

   ```bash
   $ BDF="0000:29:00.0"
   $ DEV="/sys/bus/pci/devices/$BDF"
   $ echo "vfio-pci" | sudo tee "$DEV"/driver_override
   $ echo $BDF | sudo tee "$DEV"/driver/unbind
   $ echo "$BDF" | sudo tee "/sys/bus/pci/drivers_probe"
   ```

   After you run the previous commands, the GPU is  bound to `vfio-pci` driver.<br/>
   A new directory with the IOMMU group number is created under `/dev/vfio`:

   ```bash
   $ ls -l /dev/vfio
     total 0
     crw------- 1   root root  241,   0 May 18 15:38 27
     crw-rw-rw- 1 root root  10, 196 May 18 15:37 vfio
   ```

   Later, to return the device to the standard driver, we simply clear the
   `driver_override` and re-probe the device, ex:

   ```bash
   $ echo | sudo tee "$DEV/preferred_driver"
   $ echo $BDF | sudo tee $DEV/driver/unbind
   $ echo $BDF | sudo tee /sys/bus/pci/drivers_probe
   ```

5. Start a Kata container with GPU device:

   ```bash
   $ sudo ctr --debug run --runtime "io.containerd.kata.v2"  --device "/dev/vfio/27"  --rm -t  "docker.io/library/archlinux:latest" arch uname -r

   ```

   Run `lspci` within the container to verify the GPU device is seen in the list of
   the PCI devices. Note the vendor-device id of the GPU ("8086:0bd5") in the `lspci` output.

## SR-IOV mode for Intel Discrete GPUs

Use the following steps to pass an Intel Graphics device in SR-IOV mode to a Kata Container:

1. Find the BDF for GPU device:

   ```sh
   $ sudo lspci -nn -D | grep Display
     0000:29:00.0 Display controller [0380]: Intel Corporation Ponte Vecchio 1T [8086:0bd5] (rev 2f)
     0000:3a:00.0 Display controller [0380]: Intel Corporation Ponte Vecchio 1T [8086:0bd5] (rev 2f)
     0000:9a:00.0 Display controller [0380]: Intel Corporation Ponte Vecchio 1T [8086:0bd5] (rev 2f)
     0000:ca:00.0 Display controller [0380]: Intel Corporation Ponte Vecchio 1T [8086:0bd5] (rev 2f)
   ```

   Run the previous command to find out the BDF for the GPU device on host.
   We choose the GPU with PCI address "0000:3a:00.0" to assign a GPU SR-IOV interface.

2. Carve out SR-IOV slice for the GPU:

   List our total possible SR-IOV virtual interfaces for the GPU:

   ```bash
   $ BDF="0000:3a:00.0"
   $ cat  "/sys/bus/pci/devices/$BDF/sriov_totalvfs"
   63
   ``` 

   Create SR-IOV interfaces for the GPU:
   ```sh
   $ echo 4 | sudo tee /sys/bus/pci/devices/$BDF/sriov_numvfs
     4
   $ sudo lspci | grep Display
     29:00.0 Display controller: Intel Corporation Ponte Vecchio 1T (rev 2f)
     3a:00.0 Display controller: Intel Corporation Ponte Vecchio 1T (rev 2f)
     3a:00.1 Display controller: Intel Corporation Ponte Vecchio 1T (rev 2f)
     3a:00.2 Display controller: Intel Corporation Ponte Vecchio 1T (rev 2f)
     3a:00.3 Display controller: Intel Corporation Ponte Vecchio 1T (rev 2f)
     3a:00.4 Display controller: Intel Corporation Ponte Vecchio 1T (rev 2f)
     9a:00.0 Display controller: Intel Corporation Ponte Vecchio 1T (rev 2f)
     ca:00.0 Display controller: Intel Corporation Ponte Vecchio 1T (rev 2f)
   ```
   The above output shows the SR-IOV interfaces created for the GPU.

3. Find the IOMMU group for the GPU SR-IOV interface(VGPU):

   ```bash
   $ BDF="0000:3a:00:1"
   $ readlink -e "/sys/bus/pci/devices/$BDF/iommu_group"
     /sys/kernel/iommu_groups/437
   $ ls -l /dev/vfio
     total 0
     crw-------   1 root root  241,   0 May 18 11:30 437
     crw-rw-rw- 1 root root  10, 196 May 18 11:29 vfio
   ```

   Now you can use the device node `/dev/vfio/437` in docker command line to pass
   the VGPU to a Kata Container.

4. Start a Kata Containers container with GPU device enabled:

   ```bash
   $ sudo ctr --debug run --runtime "io.containerd.kata.v2"  --device /dev/vfio/437  --rm -t  "docker.io/library/archlinux:latest" arch uname -r
   ```
