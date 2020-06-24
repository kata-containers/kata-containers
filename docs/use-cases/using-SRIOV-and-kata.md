# Setup to use SR-IOV with Kata Containers and Docker*

- [Install the SR-IOV Docker\* plugin](#install-the-sr-iov-docker-plugin)
- [Host setup for SR-IOV](#host-setup-for-sr-iov)
	- [Checking your NIC for SR-IOV](#checking-your-nic-for-sr-iov)
	- [IOMMU Groups and PCIe Access Control Services](#iommu-groups-and-pcie-access-control-services)
	- [Update the host kernel](#update-the-host-kernel)
- [Set up the SR-IOV Device](#set-up-the-sr-iov-device)
- [Example: Launch a Kata Containers container using SR-IOV](#example-launch-a-kata-containers-container-using-sr-iov)

Single Root I/O Virtualization (SR-IOV) enables splitting a physical device into
virtual functions (VFs). Virtual functions enable direct passthrough to virtual
machines or containers. For Kata Containers, we enabled a Container Network
Model (CNM) plugin. Additionally, we made the necessary changes in the
runtime to detect virtual functions in a container's network namespace to use
SR-IOV for network based devices.

## Install the SR-IOV Docker\* plugin

To create a network with associated VFs, which can be passed to
Kata Containers, you must install a SR-IOV Docker plugin. The
created network is based on a physical function (PF) device. The network can
create `n` containers, where `n` is the number of VFs associated with the
Physical Function (PF).

To install the plugin, follow the [plugin installation instructions](https://github.com/clearcontainers/sriov).


## Host setup for SR-IOV

In order to setup your host for SR-IOV, the following has to be true:

- The host system must support Intel VT-d.
- Your device (NIC) must support SR-IOV.
- The host kernel must have Input-Output Memory Management Unit (IOMMU)
  and Virtual Function I/O (VFIO) support.
- `CONFIG_VFIO_NOIOMMU` must be disabled in the host kernel
  configuration. You must rebuild your host system's kernel in
  order to disable `CONFIG_VFIO_NOIOMMU` in the kernel configuration.
- Optionally, you might need to add a PCI override for your Network Interface
  Controller (NIC). The section [Checking your NIC for SR-IOV](#checking-your-nic-for-sr-iov) describes how to assess if you need to make NIC changes and how to make
  the necessary changes.

Besides, you need to enable the NIC driver in your guest kernel config (e.g. mlx5 for Mellanox NIC).
All the modules need to be complied as built-in instead of loadable.

### Checking your NIC for SR-IOV

The following is an example of how to use `lspci` to check if your NIC supports
SR-IOV.

```
$ lspci | fgrep -i ethernet
01:00.0 Ethernet controller: Intel Corporation Ethernet Controller 10-Gigabit X540-AT2 (rev 03)

...
$ #sudo required below to read the card capabilities

$ sudo lspci -s 01:00.0 -v | grep SR-IOV
        Capabilities: [160] Single Root I/O Virtualization (SR-IOV)
```

If your card does not report this capability, then it does not support SR-IOV.

### IOMMU Groups and PCIe Access Control Services

Run the following command to see how the IOMMU groups are setup on your
host system:
```
$ find /sys/kernel/iommu_groups/ -type l
```

The command's output details whether or not your NIC is setup
appropriately with respect to PCIe Access Control Services (ACS).
If the IOMMU groups are setup properly, the PCI for each ACS-enabled NIC port
should be in its own IOMMU group. If the PCI bridge is within the same IOMMU
group as your NIC, it indicates that either your device does not support ACS
or your device does not appropriately share this default capability.

If you do not see any output when running the previous
command, then you likely need to update your host's kernel configuration.

For more details, see the blog post, "[IOMMU Groups, inside and out](http://vfio.blogspot.com/2014/08/iommu-groups-inside-and-out.html)"

### Update the host kernel


Depending on your host kernel configuration, you might have to rebuild the
kernel. If the following conditions are true, you do not need to rebuild
your kernel:

- `CONFIG_VFIO_IOMMU_TYPE1`, `CONFIG_VFIO`, and `CONFIG_VFIO_PCI` are set in
the kernel configuration. Your kernel is built with VFIO support when
configurations are set.
- `CONFIG_VFIO_NOIOMMU` is disabled in the host kernel configuration.

See the following steps one through three if you need to rebuild the kernel.

The following steps, which are based on the Ubuntu 16.04 distribution, update
the SR-IOV host system. If you use a different distribution, make
appropriate adjustments to the commands.

Before building a new kernel, keep in mind:

- You need to be *very clear* of the security and maintenance implications
  of creating a new **host kernel**.
- Mistakes in installing new kernels and updating the bootloader could make
  your system unbootable.
- We advise you to ensure you have a recent (and tested) full system backup
  before proceeding.

1. Grab kernel sources:

   ```
   $ sudo apt-get install linux-source-<linux-version>
   $ sudo apt-get install linux-headers-<linux-version>
   $ cd /usr/src/linux-source-<linux-version>/
   $ sudo tar -xvf linux-source-<linux-version>.tar.bz2
   $ cd linux-source-<linux-version>
   $ sudo apt-get install libssl-dev
   ```

2. Examine and update the `config` file if necessary:

   ```
   $ sudo cp /boot/config-4.8.0-36-generic .config
   $ # verify resulting .config does not have NOIOMMU set; ie: `CONFIG_VFIO_NOIOMMU` is not set
   $ grep -q "^CONFIG_VFIO_NOIOMMU" /boot/config-$(uname -r) || echo ok
   $ # verify `CONFIG_VFIO_IOMMU_TYPE1`, `CONFIG_VFIO=m` and `CONFIG_VFIO_PCI=m` are set as well.
   $ for opt in CONFIG_VFIO_IOMMU_TYPE1 CONFIG_VFIO CONFIG_VFIO_PCI
     do
      grep "^${opt}=" /boot/config-$(uname -r)
     done
   $ sudo make olddefconfig
   ```

   You might want to modify the kernel `Makefile` to add a unique identifier
   to the `EXTRAVERSION` variable prior to running the make. Including the `EXTRAVERSION`
   variable causes the `uname -r` command to indicate that a customized kernel is
   installed and running.

3. Build and install the kernel:

   ```
   $ make -j <number_of_cpus>
   $ make modules
   $ sudo make modules_install
   $ sudo make install
   ```

4. Edit grub to enable `intel-iommu`:

   ```
   edit /etc/default/grub and add intel_iommu=on to cmdline:
   $ sudo sed -i -e 's/^kernel_params = "\(.*\)"/GRUB_CMDLINE_LINUX="\1 intel_iommu=on"/g' /etc/default/grub
   $ sudo update-grub
   ```

5. Reboot the system and verify:

   Host system should be ready now. Reboot the system.
   ```
   $ sudo reboot
   ```

   To verify the kernel version and the kernel command line, take a look at
   `/proc/version` and `/proc/cmdline`

6. Verify Intel VT-d is initialized:

   To check if Intel VT-d initialized correctly, look for the following
   line in the `dmesg` output:
   ```
   DMAR: Intel(R) Virtualization Technology for Directed I/O
   ```

   Older kernels use a different prefix (e.g. PCI-DMA):
   ```
   PCI-DMA: Intel(R) Virtualization Technology for Directed I/O
   ```

7. Add the `vfio-pci` module:

   ```
   sudo modprobe vfio-pci
   ```

8. Add PCI quirk for SR-IOV NIC if necessary:

   ```
   $ find /sys/kernel/iommu_groups/ -type l
   ```
   The previous command verifies that your NIC appears in its own IOMMU group
   and no other devices appear in the same group. In the rare case where your
   PCI NIC does not appear in its own group, it is likely that the NIC does
   not support ACS or you built and ran an old kernel. Depending on your NIC
   and if it enforces isolation, you might resolve this by adding a
   `pcie_acs_override=` option to your kernel command line and reboot.
   See [PCIE-ACS-override-option](https://lkml.org/lkml/2013/5/30/513) for
   detailed information about this option.

## Set up the SR-IOV Device

All the steps in prior sections need to be performed just once to prepare the
SR-IOV host systems. The following is needed per system boot in order to
facilitate setting up a physical device's virtual functions.

The following procedure sets up your SR-IOV device and needs to be done per
system boot. Set up includes loading a device driver, finding out how many
virtual functions (VF) you can create, and creating those virtual functions.
Once you create VFs you cannot increase or decrease the number of VFs without
first setting the number back to zero. Based on this, it is expected that you
set the number of VFs for a physical device just once.

1. Add `vfio-pci` device driver:

   ```
   $ sudo modprobe vfio-pci
   ```
   `vfio-pci` is a driver used to reserve a VF PCI device.

2. Find the NICs of interest:

   ```
   $ lspci | grep Ethernet
   00:19.0 Ethernet controller: Intel Corporation Ethernet Connection I217-LM (rev 04)
   01:00.0 Ethernet controller: Intel Corporation Ethernet Controller 10-Gigabit X540-AT2 (rev 01)
   01:00.1 Ethernet controller: Intel Corporation Ethernet Controller 10-Gigabit X540-AT2 (rev 01)
   ```

   The previous example finds the PCI details for the NICs in question.
   In our case, both 01:00.0 and 01:00.1 are the two ports on our x540-AT2 card
   that we will use. You can use `lshw` command to get further details on the
   controller and verify it supports SR-IOV.

3. Check how many VFs you can create:

   ```
   $ cat /sys/bus/pci/devices/0000\:01\:00.0/sriov_totalvfs
   63
   $ cat /sys/bus/pci/devices/0000\:01\:00.1/sriov_totalvfs
   63
   ```
   The previous commands show how many VFs you can create. The `sriov_totalvfs`
   file under `sysfs` for a PCI device specifies the total number of VFs that you
   can create.

4. Create the VFs:

   ```
   # echo 1 | sudo tee /sys/bus/pci/devices/0000\:01\:00.0/sriov_numvfs
   # echo 1 | sudo tee /sys/bus/pci/devices/0000\:01\:00.1/sriov_numvfs
   ```

   Create virtual functions by editing `sriov_numvfs`. In our example, we create
   virtual functions by editing `sriov_numvfs`. This example
   creates one VF per physical device. Note, creating one VF eliminates the
   usefulness of SR-IOV, and is done for simplicity in this example.

 5. Verify the VFs were added to the host:

    ```
    $ sudo lspci | grep Ethernet | grep Virtual
    02:10.0 Ethernet controller: Intel Corporation X540 Ethernet Controller Virtual Function (rev 01)
    02:10.1 Ethernet controller: Intel Corporation X540 Ethernet Controller Virtual Function (rev 01)
    ```

6. Assign a MAC address to each VF:

   ```
   $ sudo ip link set <pf> vf <vfidx> mac <fake MAC address>
   ```

   Depending on the NIC being used, you might need to explicitly set the MAC
   address for the VF device. Setting the MAC address guarantees that the
   address is consistent on the host and when passed to the guest. Verify a MAC
   address is assigned to the VF using command `ip link show dev <vf>`.

## Example: Launch a Kata Containers container using SR-IOV

The following example launches a Kata Containers container using SR-IOV:

1. Build and start SR-IOV plugin:

   To install the SR-IOV plugin, follow the [SR-IOV plugin installation instructions](https://github.com/clearcontainers/sriov)

2. Create the docker network:

   ```
   $ sudo docker network create -d sriov --internal --opt pf_iface=enp1s0f0 --opt vlanid=100 --subnet=192.168.0.0/24 vfnet

   E0505 09:35:40.550129    2541 plugin.go:297] Numvfs and Totalvfs are not same on the PF - Initialize numvfs to totalvfs
   ee2e5a594f9e4d3796eda972f3b46e52342aea04cbae8e5eac9b2dd6ff37b067
   ```

   The previous commands create the required SR-IOV docker network, subnet, `vlanid`,
   and physical interface.

3. Start containers and test their connectivity:

   ```
   $ sudo docker run --runtime=kata-runtime --net=vfnet --cap-add SYS_ADMIN --ip=192.168.0.10 -it alpine
   ```

   The previous example starts a container making use of SR-IOV.
   If two machines with SR-IOV enabled NICs are connected back-to-back and each
   has a network with matching `vlanid` created, use the following two commands
   to test the connectivity:

   Machine 1:
   ```
   sriov-1:~$ sudo docker run --runtime=kata-runtime --net=vfnet  --cap-add SYS_ADMIN --ip=192.168.0.10 -it mcastelino/iperf bash -c "mount -t ramfs -o size=20M ramfs /tmp; iperf3 -s"

   ```
   Machine 2:
   ```
   sriov-2:~$ sudo docker run --runtime=kata-runtime --net=vfnet --cap-add SYS_ADMIN --ip=192.168.0.11 -it mcastelino/iperf iperf3 -c 192.168.0.10 bash -c "mount -t ramfs -o size=20M ramfs /tmp; iperf3 -c 192.168.0.10"
   ```
