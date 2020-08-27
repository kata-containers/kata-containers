# Using Intel GPU device with Kata Containers

- [Using Intel GPU device with Kata Containers](#using-intel-gpu-device-with-kata-containers)   
   - [Hardware Requirements](#hardware-requirements)   
   - [Host Kernel Requirements](#host-kernel-requirements)   
   - [Install and configure Kata Containers](#install-and-configure-kata-containers)   
   - [Build Kata Containers kernel with GPU support](#build-kata-containers-kernel-with-gpu-support)   
   - [GVT-d with Kata Containers](#gvt-d-with-kata-containers)   
   - [GVT-g with Kata Containers](#gvt-g-with-kata-containers)   

An Intel Graphics device can be passed to a Kata Containers container using GPU
passthrough (Intel GVT-d) as well as GPU mediated passthrough (Intel GVT-g).

Intel GVT-d (one VM to one physical GPU) also named as Intel-Graphics-Device
passthrough feature is one flavor of graphics virtualization approach.
This flavor allows direct assignment of an entire GPU to a single user,
passing the native driver capabilities through the hypervisor without any limitations.

Intel GVT-g (multiple VMs to one physical GPU) is a full GPU virtualization solution
with mediated pass-through.<br/>
A virtual GPU instance is maintained for each VM, with part of performance critical
resources, directly assigned. The ability to run a native graphics driver inside a
VM without hypervisor intervention in performance critical paths, achieves a good
balance among performance, feature, and sharing capability.

| Technology | Description | Behaviour | Detail |
|-|-|-|-|
| Intel GVT-d | GPU passthrough | Physical GPU assigned to a single VM | Direct GPU assignment to VM without limitation |
| Intel GVT-g | GPU sharing | Physical GPU shared by multiple VMs | Mediated passthrough |

## Hardware Requirements

 - For client platforms, 5th generation Intel® Core Processor Graphics or higher are required.
 - For server platforms, E3_v4 or higher Xeon Processor Graphics are required.

The following steps outline the workflow for using an Intel Graphics device with Kata.

## Host Kernel Requirements

The following configurations need to be enabled on your host kernel:

```
CONFIG_VFIO_IOMMU_TYPE1=m
CONFIG_VFIO=m
CONFIG_VFIO_PCI=m
CONFIG_VFIO_MDEV=m
CONFIG_VFIO_MDEV_DEVICE=m
CONFIG_DRM_I915_GVT=m
CONFIG_DRM_I915_GVT_KVMGT=m
```

Your host kernel needs to be booted with `intel_iommu=on` on the kernel command
line.

## Install and configure Kata Containers

To use this feature, you need Kata version 1.3.0 or above.
Follow the [Kata Containers setup instructions](../install/README.md)
to install the latest version of Kata.

In order to pass a GPU to a Kata Container, you need to enable the `hotplug_vfio_on_root_bus`
configuration in the Kata `configuration.toml` file as shown below.

```
$ sudo sed -i -e 's/^# *\(hotplug_vfio_on_root_bus\).*=.*$/\1 = true/g' /usr/share/defaults/kata-containers/configuration.toml
```

Make sure you are using the `pc` machine type by verifying `machine_type = "pc"` is
set in the `configuration.toml`.

## Build Kata Containers kernel with GPU support

The default guest kernel installed with Kata Containers does not provide GPU support.
To use an Intel GPU with Kata Containers, you need to build a kernel with the necessary
GPU support.

The following i915 kernel config options need to be enabled:
```
CONFIG_DRM=y
CONFIG_DRM_I915=y
CONFIG_DRM_I915_USERPTR=y
```

Build the Kata Containers kernel with the previous config options, using the instructions
described in [Building Kata Containers kernel](../../tools/packaging/kernel).
For further details on building and installing guest kernels, see [the developer guide](../Developer-Guide.md#install-guest-kernel-images).

There is an easy way to build a guest kernel that supports Intel GPU:
```
## Build guest kernel with ../../tools/packaging/kernel

# Prepare (download guest kernel source, generate .config)
$ ./build-kernel.sh -g intel -f setup

# Build guest kernel
$ ./build-kernel.sh -g intel build

# Install guest kernel
$ sudo -E ./build-kernel.sh -g intel install
/usr/share/kata-containers/vmlinux-intel-gpu.container -> vmlinux-5.4.15-70-intel-gpu
/usr/share/kata-containers/vmlinuz-intel-gpu.container -> vmlinuz-5.4.15-70-intel-gpu
```

Before using the new guest kernel, please update the `kernel` parameters in `configuration.toml`.
```
kernel = "/usr/share/kata-containers/vmlinuz-intel-gpu.container"
```

## GVT-d with Kata Containers

Use the following steps to pass an Intel Graphics device in GVT-d mode with Kata:

1. Find the Bus-Device-Function (BDF) for GPU device:

   ```
   $ sudo lspci -nn -D | grep Graphics
     0000:00:02.0 VGA compatible controller [0300]: Intel Corporation Broadwell-U Integrated Graphics [8086:1616] (rev 09)
   ```

   Run the previous command to determine the BDF for the GPU device on host.<br/>
   From the previous output, PCI address `0000:00:02.0` is assigned to the hardware GPU device.<br/>
   This BDF is used later to unbind the GPU device from the host.<br/>
   "8086 1616" is the device ID of the hardware GPU device. It is used later to
   rebind the GPU device to `vfio-pci` driver.

2. Find the IOMMU group for the GPU device:

   ```
   $ BDF="0000:00:02.0"
   $ readlink -e /sys/bus/pci/devices/$BDF/iommu_group
   /sys/kernel/iommu_groups/1
   ```

   The previous output shows that the GPU belongs to IOMMU group 1.

3. Unbind the GPU:

   ```
   $ echo $BDF | sudo tee /sys/bus/pci/devices/$BDF/driver/unbind
   ```

4. Bind the GPU to the `vfio-pci` device driver:

   ```
   $ sudo modprobe vfio-pci
   $ echo 8086 1616 | sudo tee /sys/bus/pci/drivers/vfio-pci/new_id
   $ echo $BDF | sudo tee --append /sys/bus/pci/drivers/vfio-pci/bind
   ```

   After you run the previous commands, the GPU is  bound to `vfio-pci` driver.<br/>
   A new directory with the IOMMU group number is created under `/dev/vfio`:

   ```
   $ ls -l /dev/vfio
     total 0
     crw------- 1   root root  241,   0 May 18 15:38 1
     crw-rw-rw- 1 root root  10, 196 May 18 15:37 vfio
   ```

5. Start a Kata container with GPU device:

   ```
   $ sudo docker run -it --runtime=kata-runtime --rm --device /dev/vfio/1 -v /dev:/dev debian /bin/bash
   ```

   Run `lspci` within the container to verify the GPU device is seen in the list of
   the PCI devices. Note the vendor-device id of the GPU ("8086:1616") in the `lspci` output.

   ```
   $ lspci -nn -D
     0000:00:00.0 Class [0600]: Device [8086:1237] (rev 02)
     0000:00:01.0 Class [0601]: Device [8086:7000]
     0000:00:01.1 Class [0101]: Device [8086:7010]
     0000:00:01.3 Class [0680]: Device [8086:7113] (rev 03)
     0000:00:02.0 Class [0604]: Device [1b36:0001]
     0000:00:03.0 Class [0780]: Device [1af4:1003]
     0000:00:04.0 Class [0100]: Device [1af4:1004]
     0000:00:05.0 Class [0002]: Device [1af4:1009]
     0000:00:06.0 Class [0200]: Device [1af4:1000]
     0000:00:0f.0 Class [0300]: Device [8086:1616] (rev 09)
   ```

   Additionally, you can access the device node for the graphics device:

   ```
   $ ls /dev/dri
     card0  renderD128
   ```

## GVT-g with Kata Containers

For GVT-g, you append `i915.enable_gvt=1` in addition to `intel_iommu=on`
on your host kernel command line and then reboot your host.

Use the following steps to pass an Intel Graphics device in GVT-g mode to a Kata Container:

1. Find the BDF for GPU device:

   ```
   $ sudo lspci -nn -D | grep Graphics
     0000:00:02.0 VGA compatible controller [0300]: Intel Corporation Broadwell-U Integrated Graphics [8086:1616] (rev 09)
   ```

   Run the previous command to find out the BDF for the GPU device on host.
   The previous output shows PCI address "0000:00:02.0" is assigned to the GPU device.

2. Choose the MDEV (Mediated Device) type for VGPU (Virtual GPU):

   For background on `mdev` types, please follow this [kernel documentation](https://github.com/torvalds/linux/blob/master/Documentation/driver-api/vfio-mediated-device.rst).

   * List out the `mdev` types for the VGPU:

     ```
     $ BDF="0000:00:02.0"

     $ ls /sys/devices/pci0000:00/$BDF/mdev_supported_types
       i915-GVTg_V4_1  i915-GVTg_V4_2  i915-GVTg_V4_4  i915-GVTg_V4_8
     ```

   * Inspect the `mdev` types and choose one that fits your requirement:

     ```
     $ cd /sys/devices/pci0000:00/0000:00:02.0/mdev_supported_types/i915-GVTg_V4_8 && ls
       available_instances  create  description  device_api  devices

     $ cat description
       low_gm_size: 64MB
       high_gm_size: 384MB
       fence: 4
       resolution: 1024x768
       weight: 2

     $ cat available_instances
       7
     ```

     The output of file `description` represents the GPU resources that are
     assigned to the VGPU with specified MDEV type.The output of file `available_instances`
     represents the remaining amount of VGPUs you can create with specified MDEV type.

3. Create a VGPU:

   * Generate a UUID:

     ```
     $ gpu_uuid=$(uuid)
     ```

   * Write the UUID to the `create` file under the chosen `mdev` type:

     ```
     $ echo $(gpu_uuid) | sudo tee /sys/devices/pci0000:00/0000:00:02.0/mdev_supported_types/i915-GVTg_V4_8/create
     ```

4. Find the IOMMU group for the VGPU:

   ```
   $ ls -la /sys/devices/pci0000:00/0000:00:02.0/mdev_supported_types/i915-GVTg_V4_8/devices/${gpu_uuid}/iommu_group
     lrwxrwxrwx 1 root root 0 May 18 14:35 devices/bbc4aafe-5807-11e8-a43e-03533cceae7d/iommu_group -> ../../../../kernel/iommu_groups/0

   $ ls -l /dev/vfio
     total 0
     crw-------   1 root root  241,   0 May 18 11:30 0
     crw-rw-rw- 1 root root  10, 196 May 18 11:29 vfio
   ```

   The IOMMU group "0" is created from the previous output.<br/>
   Now you can use the device node `/dev/vfio/0` in docker command line to pass
   the VGPU to a Kata Container.

5. Start Kata container with GPU device enabled:

   ```
   $ sudo docker run -it --runtime=kata-runtime --rm --device /dev/vfio/0 -v /dev:/dev debian /bin/bash
   $ lspci -nn -D
     0000:00:00.0 Class [0600]: Device [8086:1237] (rev 02)
     0000:00:01.0 Class [0601]: Device [8086:7000]
     0000:00:01.1 Class [0101]: Device [8086:7010]
     0000:00:01.3 Class [0680]: Device [8086:7113] (rev 03)
     0000:00:02.0 Class [0604]: Device [1b36:0001]
     0000:00:03.0 Class [0780]: Device [1af4:1003]
     0000:00:04.0 Class [0100]: Device [1af4:1004]
     0000:00:05.0 Class [0002]: Device [1af4:1009]
     0000:00:06.0 Class [0200]: Device [1af4:1000]
     0000:00:0f.0 Class [0300]: Device [8086:1616] (rev 09)
    ```

   BDF "0000:00:0f.0" is assigned to the VGPU device.

   Additionally, you can access the device node for the graphics device:

   ```
   $ ls /dev/dri
     card0  renderD128
   ```
