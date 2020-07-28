# Table of Contents

* [Table of Contents](#table-of-contents)
* [Introduction](#introduction)
    * [Helpful Links before starting](#helpful-links-before-starting)
    * [Steps to enable Intel QAT in Kata Containers](#steps-to-enable-intel-qat-in-kata-containers)
    * [Script variables](#script-variables)
        * [Set environment variables (Every Reboot)](#set-environment-variables-every-reboot)
    * [Prepare the Clear Linux Host](#prepare-the-clear-linux-host)
        * [Identify which PCI Bus the Intel QAT card is on](#identify-which-pci-bus-the-intel-qat-card-is-on)
        * [Install necessary bundles for Clear Linux](#install-necessary-bundles-for-clear-linux)
        * [Download Intel QAT drivers](#download-intel-qat-drivers)
        * [Copy Intel QAT configuration files and enable Virtual Functions](#copy-intel-qat-configuration-files-and-enable-virtual-functions)
        * [Expose and Bind Intel QAT virtual functions to VFIO-PCI (Every reboot)](#expose-and-bind-intel-qat-virtual-functions-to-vfio-pci-every-reboot)
        * [Check Intel QAT virtual functions are enabled](#check-intel-qat-virtual-functions-are-enabled)
    * [Prepare Kata Containers](#prepare-kata-containers)
        * [Download Kata kernel Source](#download-kata-kernel-source)
        * [Build Kata kernel](#build-kata-kernel)
        * [Copy Kata kernel](#copy-kata-kernel)
        * [Prepare Kata root filesystem](#prepare-kata-root-filesystem)
        * [Compile Intel QAT drivers for Kata Containers kernel and add to Kata Containers rootfs](#compile-intel-qat-drivers-for-kata-containers-kernel-and-add-to-kata-containers-rootfs)
        * [Copy Kata rootfs](#copy-kata-rootfs)
        * [Update Kata configuration to point to custom kernel and rootfs](#update-kata-configuration-to-point-to-custom-kernel-and-rootfs)
    * [Verify Intel QAT works in a Docker Kata Containers container](#verify-intel-qat-works-in-a-docker-kata-containers-container)
    * [Build OpenSSL Intel QAT engine container](#build-openssl-intel-qat-engine-container)
        * [Test Intel QAT in Docker](#test-intel-qat-in-docker)
        * [Troubleshooting](#troubleshooting)
    * [Optional Scripts](#optional-scripts)
        * [Verify Intel QAT card counters are incremented](#verify-intel-qat-card-counters-are-incremented)

# Introduction

Intel QuickAssist Technology (Intel QAT) provides hardware acceleration 
for security (cryptography) and compression. These instructions cover the 
steps for [Clear Linux](https://clearlinux.org) but can be adapted to any 
Linux distribution. Your distribution may already have the Intel QAT 
drivers, but it is likely they do not contain the necessary user space 
components. These instructions guide the user on how to download the kernel 
sources, compile kernel driver modules against those sources, and load them 
onto the host as well as preparing a specially built Kata Containers kernel 
and custom Kata Containers rootfs.  

## Helpful Links before starting

[Intel QAT Engine](https://github.com/intel/QAT_Engine)

[Intel QuickAssist Technology at `01.org`](https://01.org/intel-quickassist-technology)

[Intel Device Plugin for Kubernetes](https://github.com/intel/intel-device-plugins-for-kubernetes)

[Intel QuickAssist Crypto Poll Mode Driver](https://dpdk-docs.readthedocs.io/en/latest/cryptodevs/qat.html)

## Steps to enable Intel QAT in Kata Containers

There are some steps to complete only once, some steps to complete with every
reboot, and some steps to complete when the host kernel changes.

## Script variables

The following list of variables must be set before running through the 
scripts. These variables refer to locations to store modules and configuration 
files on the host and links to the drivers to use. Modify these as
needed to point to updated drivers or different install locations.

### Set environment variables (Every Reboot)

Make sure to check [`01.org`](https://01.org/intel-quickassist-technology) for 
the latest driver.

```sh
$ export QAT_DRIVER_VER=qat1.7.l.4.8.0-00005.tar.gz 
$ export QAT_DRIVER_URL=https://01.org/sites/default/files/downloads/${QAT_DRIVER_VER}
$ export QAT_CONF_LOCATION=~/QAT_conf
$ export QAT_DOCKERFILE=https://raw.githubusercontent.com/intel/intel-device-plugins-for-kubernetes/master/demo/openssl-qat-engine/Dockerfile
$ export QAT_SRC=~/src/QAT
$ export GOPATH=~/src/go
$ export OSBUILDER=~/src/osbuilder
$ export KATA_KERNEL_LOCATION=~/kata
$ export KATA_ROOTFS_LOCATION=~/kata
```

## Prepare the Clear Linux Host

The host could be a bare metal instance or a virtual machine. If using a 
virtual machine, make sure that KVM nesting is enabled. The following 
instructions reference an Intel QAT. Some of the instructions must be 
modified if using a different Intel QAT device. You can identify the Intel QAT
chipset by executing the following.

### Identify which PCI Bus the Intel QAT card is on

```sh
$ for i in 0434 0435 37c8 1f18 1f19; do lspci -d 8086:$i; done
```

### Install necessary bundles for Clear Linux

Clear Linux version 30780 (Released August 13, 2019) includes a 
`linux-firmware-qat` bundle that has the necessary QAT firmware along with a
functional QAT host driver that works with Kata Containers. 

```sh
$ sudo swupd bundle-add network-basic linux-firmware-qat make c-basic go-basic containers-virt dev-utils devpkg-elfutils devpkg-systemd devpkg-ssl
$ sudo clr-boot-manager update
$ sudo systemctl enable --now docker
$ sudo reboot
```

### Download Intel QAT drivers

This will download the Intel QAT drivers from [`01.org`](https://01.org/intel-quickassist-technology). 
Make sure to check the website for the latest version.

```sh
$ mkdir -p $QAT_SRC
$ cd $QAT_SRC
$ curl -L $QAT_DRIVER_URL | tar zx
```

### Copy Intel QAT configuration files and enable Virtual Functions

Modify the instructions below as necessary if using a different QAT hardware 
platform. You can learn more about customizing configuration files at the 
[Intel QAT Engine repository](https://github.com/intel/QAT_Engine/#copy-the-correct-intel-quickassist-technology-driver-config-files)
This section starts from a base config file and changes the `SSL` section to 
`SHIM` to support the OpenSSL engine. There are more tweaks that you can make
depending on the use case and how many Intel QAT engines should be run. You
can find more information about how to customize in the 
[Intel® QuickAssist Technology Software for Linux* - Programmer's Guide.](https://01.org/sites/default/files/downloads/336210qatswprogrammersguiderev006.pdf) 

> **Note: This section assumes that a QAT `c6xx` platform is used.**

```sh
$ mkdir -p $QAT_CONF_LOCATION
$ cp $QAT_SRC/quickassist/utilities/adf_ctl/conf_files/c6xxvf_dev0.conf.vm $QAT_CONF_LOCATION/c6xxvf_dev0.conf
$ sed -i 's/\[SSL\]/\[SHIM\]/g' $QAT_CONF_LOCATION/c6xxvf_dev0.conf
```

### Expose and Bind Intel QAT virtual functions to VFIO-PCI (Every reboot)

To enable virtual functions, the host OS should have IOMMU groups enabled. In 
the UEFI Firmware Intel Virtualization Technology for Directed I/O 
(Intel VT-d) must be enabled. Also, the kernel boot parameter should be 
`intel_iommu=on` or `intel_iommu=ifgx_off`. The default in Clear Linux currently 
is `intel_iommu=igfx_off` which should work with the Intel QAT device. The 
following commands assume you installed an Intel QAT card, IOMMU is on, and
VT-d is enabled. The vendor and device ID add to the `VFIO-PCI` driver so that
each exposed virtual function can be bound to the `VFIO-PCI` driver. Once
complete, each virtual function passes into a Kata Containers container using
the PCIe device passthrough feature. For Kubernetes, the Intel device plugin
for Kubernetes handles the binding of the driver but the VF’s still must be
enabled.

```sh
$ sudo modprobe vfio-pci
$ QAT_PCI_BUS_PF_NUMBERS=$((lspci -d :435 && lspci -d :37c8 && lspci -d :19e2 && lspci -d :6f54) | cut -d ' ' -f 1)
$ QAT_PCI_BUS_PF_1=$(echo $QAT_PCI_BUS_PF_NUMBERS | cut -d ' ' -f 1)
$ echo 16 | sudo tee /sys/bus/pci/devices/0000:$QAT_PCI_BUS_PF_1/sriov_numvfs
$ QAT_PCI_ID_VF=$(cat /sys/bus/pci/devices/0000:${QAT_PCI_BUS_PF_1}/virtfn0/uevent | grep PCI_ID)
$ QAT_VENDOR_AND_ID_VF=$(echo ${QAT_PCI_ID_VF/PCI_ID=} | sed 's/:/ /')
$ echo $QAT_VENDOR_AND_ID_VF | sudo tee --append /sys/bus/pci/drivers/vfio-pci/new_id
```
Loop through all the virtual functions and bind to the VFIO driver
```sh
$ for f in /sys/bus/pci/devices/0000:$QAT_PCI_BUS_PF_1/virtfn*
  do QAT_PCI_BUS_VF=$(basename $(readlink $f))
   echo $QAT_PCI_BUS_VF | sudo tee --append /sys/bus/pci/drivers/c6xxvf/unbind
   echo $QAT_PCI_BUS_VF | sudo tee --append /sys/bus/pci/drivers/vfio-pci/bind
  done
```

### Check Intel QAT virtual functions are enabled

If the following command returns empty, then the virtual functions are not 
properly enabled. This command checks the enumerated device IDs for just the 
virtual functions. Using the Intel QAT as an example, the physical device ID 
is `37c8` and virtual function device ID is `37c9`. The following command checks 
if VF's are enabled for any of the currently known Intel QAT device ID's. The
following `ls` command should show the 16 VF's bound to `VFIO-PCI`.

```sh
$ for i in 0442 0443 37c9 19e3; do lspci -d 8086:$i; done
```

Another way to check is to see what PCI devices that `VFIO-PCI` is mapped to.
It should match the device ID's of the VF's.
```sh
$ ls -la /sys/bus/pci/drivers/vfio-pci
```

## Prepare Kata Containers

### Download Kata kernel Source

This example automatically uses the latest Kata kernel supported by Kata. It
follows the instructions from the
[packaging kernel repository](../../tools/packaging/kernel)
and uses the latest Kata kernel
[config](../../tools/packaging/kernel/configs).
There are some patches that must be installed as well, which the 
`build-kernel.sh` script should automatically apply. If you are using a
different kernel version, then you might need to manually apply them. Since
the Kata Containers kernel has a minimal set of kernel flags set, you must
create a QAT kernel fragment with the necessary `CONFIG_CRYPTO_*` options set.
Update the config to set some of the `CRYPTO` flags to enabled. This might
change with different kernel versions. We tested the following instructions
with kernel `v4.19.28-41`.

```sh
$ mkdir -p $GOPATH
$ cd $GOPATH
$ go get -v github.com/kata-containers/packaging
$ cat << EOF > $GOPATH/src/github.com/kata-containers/packaging/kernel/configs/fragments/common/qat.conf
CONFIG_PCIEAER=y
CONFIG_UIO=y
CONFIG_CRYPTO_HW=y
CONFIG_CRYPTO_DEV_QAT_C62XVF=m
CONFIG_CRYPTO_CBC=y
CONFIG_MODULES=y
CONFIG_MODULE_SIG=y
CONFIG_CRYPTO_AUTHENC=y
CONFIG_CRYPTO_DH=y
EOF
$ $GOPATH/src/github.com/kata-containers/packaging/kernel/build-kernel.sh setup
```

### Build Kata kernel

```sh
$ export LINUX_VER=$(ls -d kata*)
$ sed -i 's/EXTRAVERSION =/EXTRAVERSION = .qat.container/' $LINUX_VER/Makefile
$ $GOPATH/src/github.com/kata-containers/packaging/kernel/build-kernel.sh build
```


### Copy Kata kernel

```sh
$ mkdir -p $KATA_KERNEL_LOCATION
$ cp $LINUX_VER/arch/x86/boot/bzImage $KATA_KERNEL_LOCATION/vmlinuz-${LINUX_VER}_qat
```

### Prepare Kata root filesystem

These instructions build upon the OS builder instructions located in the 
[Developer Guide](../Developer-Guide.md). The following instructions use Clear
Linux (Kata Containers default) as the root filesystem with systemd as the 
init and will add in the `kmod` binary, which is not a standard binary in a 
Kata rootfs image. The `kmod` binary is necessary to load the QAT kernel 
modules when the virtual machine rootfs boots. You should install Docker on
your system before running the following commands. If you need to use a custom 
`kata-agent`, then refer to the previous link on how to add it in.

```sh
$ mkdir -p $OSBUILDER
$ cd $OSBUILDER
$ git clone https://github.com/kata-containers/osbuilder.git
$ export ROOTFS_DIR=${OSBUILDER}/osbuilder/rootfs-builder/rootfs
$ export EXTRA_PKGS='kmod'
```
Make sure that the `kata-agent` version matches the installed `kata-runtime`
version.
```sh
$ export AGENT_VERSION=$(kata-runtime version | head -n 1 | grep -o "[0-9.]\+")
$ cd ${OSBUILDER}/osbuilder/rootfs-builder
$ sudo rm -rf ${ROOTFS_DIR}
$ script -fec 'sudo -E GOPATH=$GOPATH USE_DOCKER=true SECCOMP=no ./rootfs.sh clearlinux'
```

### Compile Intel QAT drivers for Kata Containers kernel and add to Kata Containers rootfs

After the Kata Containers kernel builds with the proper configuration flags, 
you must build the Intel QAT drivers against that Kata Containers kernel
version in a similar way they were previously built for the host OS. You must 
set the `KERNEL_SOURCE_ROOT` variable to the Kata Containers kernel source 
directory and build the Intel QAT drivers again.

```sh
$ cd $GOPATH
$ export LINUX_VER=$(ls -d kata*)
$ export KERNEL_MAJOR_VERSION=$(awk '/^VERSION =/{print $NF}' $GOPATH/$LINUX_VER/Makefile)
$ export KERNEL_PATHLEVEL=$(awk '/^PATCHLEVEL =/{print $NF}' $GOPATH/$LINUX_VER/Makefile)
$ export KERNEL_SUBLEVEL=$(awk '/^SUBLEVEL =/{print $NF}' $GOPATH/$LINUX_VER/Makefile)
$ export KERNEL_EXTRAVERSION=$(awk '/^EXTRAVERSION =/{print $NF}' $GOPATH/$LINUX_VER/Makefile)
$ export KERNEL_ROOTFS_DIR=${KERNEL_MAJOR_VERSION}.${KERNEL_PATHLEVEL}.${KERNEL_SUBLEVEL}${KERNEL_EXTRAVERSION}
$ cd $QAT_SRC
$ KERNEL_SOURCE_ROOT=$GOPATH/$LINUX_VER ./configure --disable-qat-lkcf --enable-icp-sriov=guest
$ sudo -E make all -j$(nproc)
$ sudo -E make INSTALL_MOD_PATH=$ROOTFS_DIR qat-driver-install -j$(nproc)
```
The `usdm_drv` module also needs to be copied into the rootfs modules path and
`depmod` should be run. 
```sh
$ sudo cp $QAT_SRC/build/usdm_drv.ko $ROOTFS_DIR/usr/lib/modules/${KERNEL_ROOTFS_DIR}/updates/drivers  
$ sudo depmod -a -b ${ROOTFS_DIR} ${KERNEL_ROOTFS_DIR}
$ cd ${OSBUILDER}/osbuilder/image-builder
$ script -fec 'sudo -E USE_DOCKER=true ./image_builder.sh ${ROOTFS_DIR}'
```

> **Note: Ignore any errors on modules.builtin and modules.order when running 
> `depmod`.**

### Copy Kata rootfs

```sh
$ mkdir -p $KATA_ROOTFS_LOCATION
$ cp ${OSBUILDER}/osbuilder/image-builder/kata-containers.img $KATA_ROOTFS_LOCATION
```

### Update Kata configuration to point to custom kernel and rootfs

You must update the `configuration.toml` for Kata Containers to point to the 
custom kernel, custom rootfs, and to specify which modules to load when the 
virtual machine is booted when a container is run. The following example
assumes you installed an Intel QAT, and you need to load those modules.

```sh
$ sudo mkdir -p /etc/kata-containers
$ sudo cp /usr/share/defaults/kata-containers/configuration-qemu.toml /etc/kata-containers/configuration.toml
$ sudo sed -i "s|kernel_params = \"\"|kernel_params = \"modules-load=usdm_drv,qat_c62xvf\"|g" /etc/kata-containers/configuration.toml
$ sudo sed -i "s|\/usr\/share\/kata-containers\/kata-containers.img|${KATA_KERNEL_LOCATION}\/kata-containers.img|g" /etc/kata-containers/configuration.toml
$ sudo sed -i "s|\/usr\/share\/kata-containers\/vmlinuz.container|${KATA_ROOTFS_LOCATION}\/vmlinuz-${LINUX_VER}_qat|g" /etc/kata-containers/configuration.toml
```

## Verify Intel QAT works in a Docker Kata Containers container

The following instructions leverage an OpenSSL Dockerfile that builds the 
Intel QAT engine to allow OpenSSL to offload crypto functions. It is a 
convenient way to test that VFIO device passthrough for the Intel QAT VF’s are
working properly with the Kata Containers VM.

## Build OpenSSL Intel QAT engine container

Use the OpenSSL Intel QAT [Dockerfile](https://github.com/intel/intel-device-plugins-for-kubernetes/tree/master/demo/openssl-qat-engine) 
to build a container image with an optimized OpenSSL engine for 
Intel QAT. Using `docker build` with the Kata Containers runtime can sometimes
have issues. Therefore, we recommended you change the default runtime to
`runc` before doing a build. Instructions for this are below.

```sh
$ cd $QAT_SRC
$ curl -O $QAT_DOCKERFILE
$ sudo sed -i 's/kata-runtime/runc/g' /etc/systemd/system/docker.service.d/50-runtime.conf
$ sudo systemctl daemon-reload && sudo systemctl restart docker
$ sudo docker build -t openssl-qat-engine .
```

> **Note: The Intel QAT driver version in this container might not match the 
> Intel QAT driver compiled and loaded on the host when compiling.**

### Test Intel QAT in Docker

The host should already be setup with 16 virtual functions of the Intel QAT 
card bound to `VFIO-PCI`. Verify this by looking in `/dev/vfio` for a listing
of devices. Replace the number 90 with one of the VF’s exposed in `/dev/vfio`.
It might require you to add an `IPC_LOCK` capability to your Docker runtime
depending on which rootfs you use.

```sh
$ sudo docker run -it --runtime=kata-runtime --cap-add=IPC_LOCK --cap-add=SYS_ADMIN --device=/dev/vfio/90 -v /dev:/dev -v ${QAT_CONF_LOCATION}:/etc openssl-qat-engine bash
```

Below are some commands to run in the container image to verify Intel QAT is 
working

```sh
bash-5.0# cat /proc/modules
bash-5.0# adf_ctl restart
bash-5.0# adf_ctl status
bash-5.0# openssl engine -c -t qat
```

Test with Intel QAT card acceleration

```sh
bash-5.0# openssl speed -engine qat -elapsed -async_jobs 72 rsa2048 
```

Test with CPU acceleration

```sh
bash-5.0# openssl speed -elapsed rsa2048
```

### Troubleshooting

* Check that `/dev/vfio` has VF’s enabled.

```sh
$ ls /dev/vfio
57  58  59  60  61  62  63  64  65  66  67  68  69  70  71  72  vfio
```

* Check that the modules load when inside the Kata Container.

```sh
bash-5.0# egrep "qat|usdm_drv" /proc/modules
qat_c62xvf 16384 - - Live 0x0000000000000000 (O)
usdm_drv 86016 - - Live 0x0000000000000000 (O)
intel_qat 184320 - - Live 0x0000000000000000 (O)
```

* Verify that at least the first `c6xxvf_dev0.conf` file mounts inside the 
container image in `/etc`. You will need one configuration file for each VF 
passed into the container.

```sh
bash-5.0# ls /etc
c6xxvf_dev0.conf   c6xxvf_dev11.conf  c6xxvf_dev14.conf  c6xxvf_dev3.conf  c6xxvf_dev6.conf  c6xxvf_dev9.conf  resolv.conf
c6xxvf_dev1.conf   c6xxvf_dev12.conf  c6xxvf_dev15.conf  c6xxvf_dev4.conf  c6xxvf_dev7.conf  hostname
c6xxvf_dev10.conf  c6xxvf_dev13.conf  c6xxvf_dev2.conf   c6xxvf_dev5.conf c6xxvf_dev8.conf  hosts
```

* Check `dmesg` inside the container to see if there are any issues with the 
Intel QAT driver.

* If there are issues building the OpenSSL Intel QAT container image, then 
check to make sure that runc is the default runtime for building container.

```sh
$ cat /etc/systemd/system/docker.service.d/50-runtime.conf
[Service]
Environment="DOCKER_DEFAULT_RUNTIME=--default-runtime runc"
```

## Optional Scripts

### Verify Intel QAT card counters are incremented

Use the `lspci` command to figure out which PCI bus the Intel QAT accelerators
are on. The counters will increase when the accelerator is actively being
used. To verify QAT is actively accelerating the containerized application,
use the following instructions to check if any of the counters are
incrementing. You will have to change the PCI device ID to match your system.

```sh
$ for i in 0434 0435 37c8 1f18 1f19; do lspci -d 8086:$i; done
$ sudo watch cat /sys/kernel/debug/qat_c6xx_0000\:b1\:00.0/fw_counters
$ sudo watch cat /sys/kernel/debug/qat_c6xx_0000\:b3\:00.0/fw_counters
$ sudo watch cat /sys/kernel/debug/qat_c6xx_0000\:b5\:00.0/fw_counters
```
