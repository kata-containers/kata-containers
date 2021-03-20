# Table of Contents

- [Table of Contents](#table-of-contents)
- [Introduction](#introduction)
  - [Helpful Links before starting](#helpful-links-before-starting)
  - [Steps to enable Intel® QAT in Kata Containers](#steps-to-enable-intel-qat-in-kata-containers)
  - [Script variables](#script-variables)
    - [Set environment variables (Every Reboot)](#set-environment-variables-every-reboot)
  - [Prepare the Ubuntu Host](#prepare-the-ubuntu-host)
    - [Identify which PCI Bus the Intel® QAT card is on](#identify-which-pci-bus-the-intel-qat-card-is-on)
    - [Install necessary packages for Ubuntu](#install-necessary-packages-for-ubuntu)
    - [Download Intel® QAT drivers](#download-intel-qat-drivers)
    - [Copy Intel® QAT configuration files and enable virtual functions](#copy-intel-qat-configuration-files-and-enable-virtual-functions)
    - [Expose and Bind Intel® QAT virtual functions to VFIO-PCI (Every reboot)](#expose-and-bind-intel-qat-virtual-functions-to-vfio-pci-every-reboot)
    - [Check Intel® QAT virtual functions are enabled](#check-intel-qat-virtual-functions-are-enabled)
  - [Prepare Kata Containers](#prepare-kata-containers)
    - [Download Kata kernel Source](#download-kata-kernel-source)
    - [Build Kata kernel](#build-kata-kernel)
    - [Copy Kata kernel](#copy-kata-kernel)
    - [Prepare Kata root filesystem](#prepare-kata-root-filesystem)
    - [Compile Intel® QAT drivers for Kata Containers kernel and add to Kata Containers rootfs](#compile-intel-qat-drivers-for-kata-containers-kernel-and-add-to-kata-containers-rootfs)
    - [Copy Kata rootfs](#copy-kata-rootfs)
  - [Verify Intel® QAT works in a container](#verify-intel-qat-works-in-a-container)
    - [Build OpenSSL Intel® QAT engine container](#build-openssl-intel-qat-engine-container)
    - [Test Intel® QAT with the ctr tool](#test-intel-qat-with-the-ctr-tool)
    - [Test Intel® QAT in Kubernetes](#test-intel-qat-in-kubernetes)
    - [Troubleshooting](#troubleshooting)
  - [Optional Scripts](#optional-scripts)
    - [Verify Intel® QAT card counters are incremented](#verify-intel-qat-card-counters-are-incremented)

# Introduction

Intel® QuickAssist Technology (QAT) provides hardware acceleration 
for security (cryptography) and compression. These instructions cover the 
steps for the latest [Ubuntu LTS release](https://ubuntu.com/download/desktop) 
which already include the QAT host driver. These instructions can be adapted to 
any Linux distribution. These instructions guide the user on how to download 
the kernel sources, compile kernel driver modules against those sources, and 
load them onto the host as well as preparing a specially built Kata Containers 
kernel and custom Kata Containers rootfs.

* Download kernel sources
* Compile Kata kernel
* Compile kernel driver modules against those sources
* Download rootfs
* Add driver modules to rootfs
* Build rootfs image 

## Helpful Links before starting

[Intel® QuickAssist Technology at `01.org`](https://01.org/intel-quickassist-technology)

[Intel® QuickAssist Technology Engine for OpenSSL](https://github.com/intel/QAT_Engine)

[Intel Device Plugin for Kubernetes](https://github.com/intel/intel-device-plugins-for-kubernetes)

[Intel® QuickAssist Technology for Crypto Poll Mode Driver](https://dpdk-docs.readthedocs.io/en/latest/cryptodevs/qat.html)

## Steps to enable Intel® QAT in Kata Containers

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

```bash
$ export QAT_DRIVER_VER=qat1.7.l.4.12.0-00011.tar.gz
$ export QAT_DRIVER_URL=https://downloadmirror.intel.com/30178/eng/${QAT_DRIVER_VER}
$ export QAT_CONF_LOCATION=~/QAT_conf
$ export QAT_DOCKERFILE=https://raw.githubusercontent.com/intel/intel-device-plugins-for-kubernetes/master/demo/openssl-qat-engine/Dockerfile
$ export QAT_SRC=~/src/QAT
$ export GOPATH=~/src/go
$ export KATA_KERNEL_LOCATION=~/kata
$ export KATA_ROOTFS_LOCATION=~/kata
```

## Prepare the Ubuntu Host

The host could be a bare metal instance or a virtual machine. If using a 
virtual machine, make sure that KVM nesting is enabled. The following 
instructions reference an Intel® C62X chipset. Some of the instructions must be 
modified if using a different Intel® QAT device. The Intel® QAT chipset can be
identified by executing the following.

### Identify which PCI Bus the Intel® QAT card is on

```bash
$ for i in 0434 0435 37c8 1f18 1f19; do lspci -d 8086:$i; done
```

### Install necessary packages for Ubuntu

These packages are necessary to compile the Kata kernel, Intel® QAT driver, and to
prepare the rootfs for Kata. [Docker](https://docs.docker.com/engine/install/ubuntu/)
also needs to be installed to be able to build the rootfs. To test that 
everything works a Kubernetes pod is started requesting Intel® QAT resources. For the
pass through of the virtual functions the kernel boot parameter needs to have
`INTEL_IOMMU=on`.

```bash
$ sudo apt update
$ sudo apt install -y golang-go build-essential python pkg-config zlib1g-dev libudev-dev bison libelf-dev flex libtool automake autotools-dev autoconf bc libpixman-1-dev coreutils libssl-dev
$ sudo sed -i 's/GRUB_CMDLINE_LINUX_DEFAULT=""/GRUB_CMDLINE_LINUX_DEFAULT="intel_iommu=on"/' /etc/default/grub
$ sudo update-grub
$ sudo reboot
```

### Download Intel® QAT drivers

This will download the [Intel® QAT drivers](https://01.org/intel-quickassist-technology). 
Make sure to check the website for the latest version.

```bash
$ mkdir -p $QAT_SRC
$ cd $QAT_SRC
$ curl -L $QAT_DRIVER_URL | tar zx
```

### Copy Intel® QAT configuration files and enable virtual functions

Modify the instructions below as necessary if using a different Intel® QAT hardware 
platform. You can learn more about customizing configuration files at the 
[Intel® QAT Engine repository](https://github.com/intel/QAT_Engine/#copy-the-correct-intel-quickassist-technology-driver-config-files)
This section starts from a base config file and changes the `SSL` section to 
`SHIM` to support the OpenSSL engine. There are more tweaks that you can make
depending on the use case and how many Intel® QAT engines should be run. You
can find more information about how to customize in the 
[Intel® QuickAssist Technology Software for Linux* - Programmer's Guide.](https://01.org/sites/default/files/downloads/336210qatswprogrammersguiderev006.pdf) 

> **Note: This section assumes that a Intel® QAT `c6xx` platform is used.**

```bash
$ mkdir -p $QAT_CONF_LOCATION
$ cp $QAT_SRC/quickassist/utilities/adf_ctl/conf_files/c6xxvf_dev0.conf.vm $QAT_CONF_LOCATION/c6xxvf_dev0.conf
$ sed -i 's/\[SSL\]/\[SHIM\]/g' $QAT_CONF_LOCATION/c6xxvf_dev0.conf
```

### Expose and Bind Intel® QAT virtual functions to VFIO-PCI (Every reboot)

To enable virtual functions, the host OS should have IOMMU groups enabled. In 
the UEFI Firmware Intel® Virtualization Technology for Directed I/O 
(Intel® VT-d) must be enabled. Also, the kernel boot parameter should be 
`intel_iommu=on` or `intel_iommu=ifgx_off`. This should have been set from
the instructions above. Check the output of `/proc/cmdline` to confirm. The 
following commands assume you installed an Intel® QAT card, IOMMU is on, and
VT-d is enabled. The vendor and device ID add to the `VFIO-PCI` driver so that
each exposed virtual function can be bound to the `VFIO-PCI` driver. Once
complete, each virtual function passes into a Kata Containers container using
the PCIe device passthrough feature. For Kubernetes, the 
[Intel device plugin](https://github.com/intel/intel-device-plugins-for-kubernetes)
for Kubernetes handles the binding of the driver, but the VF’s still must be
enabled.

```bash
$ sudo modprobe vfio-pci
$ QAT_PCI_BUS_PF_NUMBERS=$((lspci -d :435 && lspci -d :37c8 && lspci -d :19e2 && lspci -d :6f54) | cut -d ' ' -f 1)
$ QAT_PCI_BUS_PF_1=$(echo $QAT_PCI_BUS_PF_NUMBERS | cut -d ' ' -f 1)
$ echo 16 | sudo tee /sys/bus/pci/devices/0000:$QAT_PCI_BUS_PF_1/sriov_numvfs
$ QAT_PCI_ID_VF=$(cat /sys/bus/pci/devices/0000:${QAT_PCI_BUS_PF_1}/virtfn0/uevent | grep PCI_ID)
$ QAT_VENDOR_AND_ID_VF=$(echo ${QAT_PCI_ID_VF/PCI_ID=} | sed 's/:/ /')
$ echo $QAT_VENDOR_AND_ID_VF | sudo tee --append /sys/bus/pci/drivers/vfio-pci/new_id
```

Loop through all the virtual functions and bind to the VFIO driver

```bash
$ for f in /sys/bus/pci/devices/0000:$QAT_PCI_BUS_PF_1/virtfn*
  do QAT_PCI_BUS_VF=$(basename $(readlink $f))
   echo $QAT_PCI_BUS_VF | sudo tee --append /sys/bus/pci/drivers/c6xxvf/unbind
   echo $QAT_PCI_BUS_VF | sudo tee --append /sys/bus/pci/drivers/vfio-pci/bind
  done
```

### Check Intel® QAT virtual functions are enabled

If the following command returns empty, then the virtual functions are not 
properly enabled. This command checks the enumerated device IDs for just the 
virtual functions. Using the Intel® QAT as an example, the physical device ID 
is `37c8` and virtual function device ID is `37c9`. The following command checks 
if VF's are enabled for any of the currently known Intel® QAT device ID's. The
following `ls` command should show the 16 VF's bound to `VFIO-PCI`.

```bash
$ for i in 0442 0443 37c9 19e3; do lspci -d 8086:$i; done
```

Another way to check is to see what PCI devices that `VFIO-PCI` is mapped to.
It should match the device ID's of the VF's.

```bash
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
create a Intel® QAT kernel fragment with the necessary `CONFIG_CRYPTO_*` options set.
Update the config to set some of the `CRYPTO` flags to enabled. This might
change with different kernel versions. The following instructions were tested
with kernel `v5.4.0-64-generic`.

```bash
$ mkdir -p $GOPATH
$ cd $GOPATH
$ go get -v github.com/kata-containers/kata-containers
$ cat << EOF > $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kernel/configs/fragments/common/qat.conf
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
$ $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kernel/build-kernel.sh setup
```

### Build Kata kernel

```bash
$ cd $GOPATH
$ export LINUX_VER=$(ls -d kata-linux-*)
$ sed -i 's/EXTRAVERSION =/EXTRAVERSION = .qat.container/' $LINUX_VER/Makefile
$ $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kernel/build-kernel.sh build
```

### Copy Kata kernel

```bash
$ export KATA_KERNEL_NAME=vmlinux-${LINUX_VER}_qat
$ mkdir -p $KATA_KERNEL_LOCATION
$ cp ${GOPATH}/${LINUX_VER}/vmlinux ${KATA_KERNEL_LOCATION}/${KATA_KERNEL_NAME}
```

### Prepare Kata root filesystem

These instructions build upon the OS builder instructions located in the 
[Developer Guide](../Developer-Guide.md). At this point it is recommended that
[Docker](https://docs.docker.com/engine/install/ubuntu/) is installed first, and
then [Kata-deploy](https://github.com/kata-containers/kata-containers/tree/main/tools/packaging/kata-deploy)
is use to install Kata. This will make sure that the correct `agent` version 
is installed into the rootfs in the steps below.

The following instructions use Debian as the root filesystem with systemd as 
the init and will add in the `kmod` binary, which is not a standard binary in 
a Kata rootfs image. The `kmod` binary is necessary to load the Intel® QAT 
kernel modules when the virtual machine rootfs boots. 

```bash
$ export OSBUILDER=$GOPATH/src/github.com/kata-containers/kata-containers/tools/osbuilder
$ export ROOTFS_DIR=${OSBUILDER}/rootfs-builder/rootfs
$ export EXTRA_PKGS='kmod'
```

Make sure that the `kata-agent` version matches the installed `kata-runtime`
version. Also make sure the `kata-runtime` install location is in your `PATH` 
variable. The following `AGENT_VERSION` can be set manually to match
the `kata-runtime` version if the following commands don't work.

```bash
$ export PATH=$PATH:/opt/kata/bin
$ cd $GOPATH
$ export AGENT_VERSION=$(kata-runtime version | head -n 1 | grep -o "[0-9.]\+")
$ cd ${OSBUILDER}/rootfs-builder
$ sudo rm -rf ${ROOTFS_DIR}
$ script -fec 'sudo -E GOPATH=$GOPATH USE_DOCKER=true SECCOMP=no ./rootfs.sh debian'
```

### Compile Intel® QAT drivers for Kata Containers kernel and add to Kata Containers rootfs

After the Kata Containers kernel builds with the proper configuration flags, 
you must build the Intel® QAT drivers against that Kata Containers kernel
version in a similar way they were previously built for the host OS. You must 
set the `KERNEL_SOURCE_ROOT` variable to the Kata Containers kernel source 
directory and build the Intel® QAT drivers again. The  `make` command will
install the Intel® QAT modules into the Kata rootfs.

```bash
$ cd $GOPATH
$ export LINUX_VER=$(ls -d kata*)
$ export KERNEL_MAJOR_VERSION=$(awk '/^VERSION =/{print $NF}' $GOPATH/$LINUX_VER/Makefile)
$ export KERNEL_PATHLEVEL=$(awk '/^PATCHLEVEL =/{print $NF}' $GOPATH/$LINUX_VER/Makefile)
$ export KERNEL_SUBLEVEL=$(awk '/^SUBLEVEL =/{print $NF}' $GOPATH/$LINUX_VER/Makefile)
$ export KERNEL_EXTRAVERSION=$(awk '/^EXTRAVERSION =/{print $NF}' $GOPATH/$LINUX_VER/Makefile)
$ export KERNEL_ROOTFS_DIR=${KERNEL_MAJOR_VERSION}.${KERNEL_PATHLEVEL}.${KERNEL_SUBLEVEL}${KERNEL_EXTRAVERSION}
$ cd $QAT_SRC
$ KERNEL_SOURCE_ROOT=$GOPATH/$LINUX_VER ./configure --enable-icp-sriov=guest
$ sudo -E make all -j$(nproc)
$ sudo -E make INSTALL_MOD_PATH=$ROOTFS_DIR qat-driver-install -j$(nproc)
```

The `usdm_drv` module also needs to be copied into the rootfs modules path and
`depmod` should be run. 

```bash
$ sudo cp $QAT_SRC/build/usdm_drv.ko $ROOTFS_DIR/lib/modules/${KERNEL_ROOTFS_DIR}/updates/drivers  
$ sudo depmod -a -b ${ROOTFS_DIR} ${KERNEL_ROOTFS_DIR}
$ cd ${OSBUILDER}/image-builder
$ script -fec 'sudo -E USE_DOCKER=true ./image_builder.sh ${ROOTFS_DIR}'
```

> **Note: Ignore any errors on modules.builtin and modules.order when running 
> `depmod`.**

### Copy Kata rootfs

```bash
$ mkdir -p $KATA_ROOTFS_LOCATION
$ cp ${OSBUILDER}/image-builder/kata-containers.img $KATA_ROOTFS_LOCATION
```

## Verify Intel® QAT works in a container

The following instructions uses a OpenSSL Dockerfile that builds the 
Intel® QAT engine to allow OpenSSL to offload crypto functions. It is a 
convenient way to test that VFIO device passthrough for the Intel® QAT VF’s are
working properly with the Kata Containers VM.

### Build OpenSSL Intel® QAT engine container

Use the OpenSSL Intel® QAT [Dockerfile](https://github.com/intel/intel-device-plugins-for-kubernetes/tree/master/demo/openssl-qat-engine) 
to build a container image with an optimized OpenSSL engine for 
Intel® QAT. Using `docker build` with the Kata Containers runtime can sometimes
have issues. Therefore, make sure that `runc` is the default Docker container 
runtime.

```bash
$ cd $QAT_SRC
$ curl -O $QAT_DOCKERFILE
$ sudo docker build -t openssl-qat-engine .
```

> **Note: The Intel® QAT driver version in this container might not match the 
> Intel® QAT driver compiled and loaded on the host when compiling.**

### Test Intel® QAT with the ctr tool

The `ctr` tool can be used to interact with the containerd daemon. It may be 
more convenient to use this tool to verify the kernel and image instead of
setting up a Kubernetes cluster. The correct Kata runtimes need to be added
to the containerd `config.toml`. Below is a sample snippet that can be added
to allow QEMU and Cloud Hypervisor (CLH) to work with `ctr`.

```
[plugins.cri.containerd.runtimes.kata-qemu]
  runtime_type = "io.containerd.kata-qemu.v2"
  privileged_without_host_devices = true
  pod_annotations = ["io.katacontainers.*"]
  [plugins.cri.containerd.runtimes.kata-qemu.options]
    ConfigPath = "/opt/kata/share/defaults/kata-containers/configuration-qemu.toml"
[plugins.cri.containerd.runtimes.kata-clh]
  runtime_type = "io.containerd.kata-clh.v2"
  privileged_without_host_devices = true
  pod_annotations = ["io.katacontainers.*"]
  [plugins.cri.containerd.runtimes.kata-clh.options]
    ConfigPath = "/opt/kata/share/defaults/kata-containers/configuration-clh.toml"
```

In addition, containerd expects the binary to be in `/usr/local/bin` so add 
this small script so that it redirects to be able to use either QEMU or
Cloud Hypervisor with Kata.

```bash
$ echo '#!/bin/bash' | sudo tee /usr/local/bin/containerd-shim-kata-qemu-v2
$ echo 'KATA_CONF_FILE=/opt/kata/share/defaults/kata-containers/configuration-qemu.toml /opt/kata/bin/containerd-shim-kata-v2 $@' | sudo tee -a /usr/local/bin/containerd-shim-kata-qemu-v2
$ sudo chmod +x /usr/local/bin/containerd-shim-kata-qemu-v2
$ echo '#!/bin/bash' | sudo tee /usr/local/bin/containerd-shim-kata-clh-v2
$ echo 'KATA_CONF_FILE=/opt/kata/share/defaults/kata-containers/configuration-clh.toml /opt/kata/bin/containerd-shim-kata-v2 $@' | sudo tee -a /usr/local/bin/containerd-shim-kata-clh-v2
$ sudo chmod +x /usr/local/bin/containerd-shim-kata-clh-v2
```

After the OpenSSL image is built and imported into containerd, a Intel® QAT 
virtual function exposed in the step above can be added to the `ctr` command. 
Make sure to change the `/dev/vfio` number to one that actually exists on the 
host system. When using the `ctr` tool, the`configuration.toml` for Kata needs 
to point to the custom Kata kernel and rootfs built above and the Intel® QAT 
modules in the Kata rootfs need to load at boot. The following steps assume that 
`kata-deploy` was used to install Kata and QEMU is being tested. If using a 
different hypervisor, different install method for Kata, or a different 
Intel® QAT chipset then the command will need to be modified. 

> **Note: The following was tested with 
[containerd v1.3.9](https://github.com/containerd/containerd/releases/tag/v1.3.9).**

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/configuration-qemu.toml"
$ sudo sed -i "/kernel =/c kernel = "\"${KATA_ROOTFS_LOCATION}/${KATA_KERNEL_NAME}\""" $config_file
$ sudo sed -i "/image =/c image = "\"${KATA_KERNEL_LOCATION}/kata-containers.img\""" $config_file
$ sudo sed -i -e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 modules-load=usdm_drv,qat_c62xvf"/g' $config_file 
$ sudo docker save -o openssl-qat-engine.tar openssl-qat-engine:latest
$ sudo ctr images import openssl-qat-engine.tar
$ sudo ctr run --runtime io.containerd.run.kata-qemu.v2 --privileged -t --rm --device=/dev/vfio/180 --mount type=bind,src=/dev,dst=/dev,options=rbind:rw --mount type=bind,src=${QAT_CONF_LOCATION}/c6xxvf_dev0.conf,dst=/etc/c6xxvf_dev0.conf,options=rbind:rw  docker.io/library/openssl-qat-engine:latest bash
```

Below are some commands to run in the container image to verify Intel® QAT is 
working

```sh
root@67561dc2757a/ # cat /proc/modules
qat_c62xvf 16384 - - Live 0xffffffffc00d9000 (OE)
usdm_drv 86016 - - Live 0xffffffffc00e8000 (OE)
intel_qat 249856 - - Live 0xffffffffc009b000 (OE)

root@67561dc2757a/ # adf_ctl restart
Restarting all devices.
Processing /etc/c6xxvf_dev0.conf

root@67561dc2757a/ # adf_ctl status
Checking status of all devices.
There is 1 QAT acceleration device(s) in the system:
 qat_dev0 - type: c6xxvf,  inst_id: 0,  node_id: 0,  bsf: 0000:01:01.0,  #accel: 1 #engines: 1 state: up

root@67561dc2757a/ # openssl engine -c -t qat-hw
(qat-hw) Reference implementation of QAT crypto engine v0.6.1
 [RSA, DSA, DH, AES-128-CBC-HMAC-SHA1, AES-128-CBC-HMAC-SHA256, AES-256-CBC-HMAC-SHA1, AES-256-CBC-HMAC-SHA256, TLS1-PRF, HKDF, X25519, X448]
     [ available ]
```

### Test Intel® QAT in Kubernetes

Start a Kubernetes cluster with containerd as the CRI. The host should 
already be setup with 16 virtual functions of the Intel® QAT card bound to 
`VFIO-PCI`. Verify this by looking in `/dev/vfio` for a listing of devices. 
You might need to disable Docker before initializing Kubernetes. Be aware 
that the OpenSSL container image built above will need to be exported from
Docker and imported into containerd.

If Kata is installed through [`kata-deploy`](https://github.com/kata-containers/kata-containers/blob/stable-2.0/tools/packaging/kata-deploy/README.md)
there will be multiple `configuration.toml` files associated with different 
hypervisors. Rather than add in the custom Kata kernel, Kata rootfs, and 
kernel modules to each `configuration.toml` as the default, instead use
[annotations](https://github.com/kata-containers/kata-containers/blob/stable-2.0/docs/how-to/how-to-load-kernel-modules-with-kata.md)
in the Kubernetes YAML file to tell Kata which kernel and rootfs to use. The 
easy way to do this is to use `kata-deploy` which will install the Kata binaries
to `/opt` and properly configure the `/etc/containerd/config.toml` with annotation 
support. However, the `configuration.toml` needs to enable support for
annotations as well. The following configures both QEMU and Cloud Hypervisor
`configuration.toml` files that are currently available with Kata Container 
versions 2.0 and higher.

```bash
$ sudo sed -i 's/enable_annotations\s=\s\[\]/enable_annotations = [".*"]/' /opt/kata/share/defaults/kata-containers/configuration-qemu.toml
$ sudo sed -i 's/enable_annotations\s=\s\[\]/enable_annotations = [".*"]/' /opt/kata/share/defaults/kata-containers/configuration-clh.toml
```

Export the OpenSSL image from Docker and import into containerd.

```bash
$ sudo docker save -o openssl-qat-engine.tar openssl-qat-engine:latest
$ sudo ctr -n=k8s.io images import openssl-qat-engine.tar
```

The [Intel® QAT Plugin](https://github.com/intel/intel-device-plugins-for-kubernetes/blob/master/cmd/qat_plugin/README.md)
needs to be started so that the virtual functions can be discovered and
used by Kubernetes. 

The following YAML file can be used to start a Kata container with Intel® QAT
support. If Kata is installed with `kata-deploy`, then the containerd 
`configuration.toml` should have all of the Kata runtime classes already 
populated and annotations supported. To use a Intel® QAT virtual function, the 
Intel® QAT plugin needs to be started after the VF's are bound to `VFIO-PCI` as 
described [above](#expose-and-bind-intel-qat-virtual-functions-to-vfio-pci-every-reboot). 
Edit the following to point to the correct Kata kernel and rootfs location 
built with Intel® QAT support.

```bash
$ cat << EOF > kata-openssl-qat.yaml
apiVersion: v1
kind: Pod
metadata:
  name: kata-openssl-qat
  labels:
    app: kata-openssl-qat
  annotations:
    io.katacontainers.config.hypervisor.kernel: "$KATA_KERNEL_LOCATION/$KATA_KERNEL_NAME"
    io.katacontainers.config.hypervisor.image: "$KATA_ROOTFS_LOCATION/kata-containers.img"
    io.katacontainers.config.hypervisor.kernel_params: "modules-load=usdm_drv,qat_c62xvf"
spec:
  runtimeClassName: kata-qemu
  containers:
  - name: kata-openssl-qat
    image: docker.io/library/openssl-qat-engine:latest
    imagePullPolicy: IfNotPresent
    resources:
      limits:
        qat.intel.com/generic: 1
        cpu: 1
    securityContext:
      capabilities:
        add: ["IPC_LOCK", "SYS_ADMIN"]
    volumeMounts:
      - mountPath: /etc/c6xxvf_dev0.conf
        name: etc-mount
      - mountPath: /dev
        name: dev-mount
  volumes:
    - name: dev-mount
      hostPath:
        path: /dev
    - name: etc-mount
      hostPath:
        path: $QAT_CONF_LOCATION/c6xxvf_dev0.conf
EOF
```

Use `kubectl` to start the pod. Verify that Intel® QAT card acceleration is 
working with the Intel® QAT engine.
```bash
$ kubectl apply -f kata-openssl-qat.yaml
```

```sh
$ kubectl exec -it kata-openssl-qat -- adf_ctl restart
Restarting all devices.
Processing /etc/c6xxvf_dev0.conf

$ kubectl exec -it kata-openssl-qat -- adf_ctl status
Checking status of all devices.
There is 1 QAT acceleration device(s) in the system:
 qat_dev0 - type: c6xxvf,  inst_id: 0,  node_id: 0,  bsf: 0000:01:01.0,  #accel: 1 #engines: 1 state: up

$ kubectl exec -it kata-openssl-qat -- openssl engine -c -t qat-hw
(qat-hw) Reference implementation of QAT crypto engine v0.6.1
 [RSA, DSA, DH, AES-128-CBC-HMAC-SHA1, AES-128-CBC-HMAC-SHA256, AES-256-CBC-HMAC-SHA1, AES-256-CBC-HMAC-SHA256, TLS1-PRF, HKDF, X25519, X448]
     [ available ]
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
Intel® QAT driver.

* If there are issues building the OpenSSL Intel® QAT container image, then 
check to make sure that runc is the default runtime for building container.

```sh
$ cat /etc/systemd/system/docker.service.d/50-runtime.conf
[Service]
Environment="DOCKER_DEFAULT_RUNTIME=--default-runtime runc"
```

## Optional Scripts

### Verify Intel® QAT card counters are incremented

To check the built in firmware counters, the Intel® QAT driver has to be compiled 
and installed to the host and can't rely on the built in host driver. The 
counters will increase when the accelerator is actively being used. To verify 
Intel® QAT is actively accelerating the containerized application, use the 
following instructions to check if any of the counters increment. Make 
sure to change the PCI Device ID to match whats in the system.

```bash
$ for i in 0434 0435 37c8 1f18 1f19; do lspci -d 8086:$i; done
$ sudo watch cat /sys/kernel/debug/qat_c6xx_0000\:b1\:00.0/fw_counters
$ sudo watch cat /sys/kernel/debug/qat_c6xx_0000\:b3\:00.0/fw_counters
$ sudo watch cat /sys/kernel/debug/qat_c6xx_0000\:b5\:00.0/fw_counters
```