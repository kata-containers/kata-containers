
# Kata Containers with NEMU

* [Introduction](#introduction)
* [Pre-requisites](#pre-requisites)
* [NEMU](#nemu)
    * [Download and build](#download-and-build)
        * [x86_64](#x86_64)
        * [aarch64](#aarch64)
* [Configure Kata Containers](#configure-kata-containers)

Kata Containers relies by default on the QEMU hypervisor in order to spawn the virtual machines running containers. [NEMU](https://github.com/intel/nemu) is a fork of QEMU that:
- Reduces the number of lines of code.
- Removes all legacy devices.
- Reduces the emulation as far as possible.

## Introduction

This document describes how to run Kata Containers with NEMU, first by explaining how to download, build and install it. Then it walks through the steps needed to update your Kata Containers configuration in order to run with NEMU. 

## Pre-requisites
This document requires Kata Containers to be [installed](../install/README.md) on your system.

Also, it's worth noting that NEMU only supports `x86_64` and `aarch64` architecture.

## NEMU

### Download and build

```bash
$ git clone https://github.com/intel/nemu.git
$ cd nemu
$ git fetch origin
$ git checkout origin/experiment/automatic-removal
```
#### x86_64
```
$ SRCDIR=$PWD ./tools/build_x86_64_virt.sh
```
#### aarch64
```
$ SRCDIR=$PWD ./tools/build_aarch64.sh
```

> **Note:** The branch `experiment/automatic-removal` is a branch published by Jenkins after it has applied the automatic removal script to the `topic/virt-x86` branch. The purpose of this code removal being to reduce the source tree by removing files not being used by NEMU.

After those commands have successfully returned, you will find the NEMU binary at `$HOME/build-x86_64_virt/x86_64_virt-softmmu/qemu-system-x86_64_virt` (__x86__), or `$HOME/build-aarch64/aarch64-softmmu/qemu-system-aarch64` (__ARM__).

You also need the `OVMF` firmware in order to boot the virtual machine's kernel. It can currently be found at this [location](https://github.com/intel/ovmf-virt/releases).
```bash
$ sudo mkdir -p /usr/share/nemu
$ OVMF_URL=$(curl -sL https://api.github.com/repos/intel/ovmf-virt/releases/latest | jq -S '.assets[0].browser_download_url')
$ curl -o OVMF.fd -L $(sed -e 's/^"//' -e 's/"$//' <<<"$OVMF_URL")
$ sudo install -o root -g root -m 0640 OVMF.fd /usr/share/nemu/
```
> **Note:** The OVMF firmware will be located at this temporary location until the changes can be pushed upstream.


## Configure Kata Containers
All you need from this section is to modify the configuration file `/usr/share/defaults/kata-containers/configuration.toml` to specify the options related to the hypervisor.


```diff
 [hypervisor.qemu]
-path = "/usr/bin/qemu-lite-system-x86_64"
+path = "/home/foo/build-x86_64_virt/x86_64_virt-softmmu/qemu-system-x86_64_virt"
 kernel = "/usr/share/kata-containers/vmlinuz.container"
 initrd = "/usr/share/kata-containers/kata-containers-initrd.img"
 image = "/usr/share/kata-containers/kata-containers.img"
-machine_type = "pc"
+machine_type = "virt"
 
 # Optional space-separated list of options to pass to the guest kernel.
 # For example, use `kernel_params = "vsyscall=emulate"` if you are having
@@ -31,7 +31,7 @@
 
 # Path to the firmware.
 # If you want that qemu uses the default firmware leave this option empty
-firmware = ""
+firmware = "/usr/share/nemu/OVMF.fd"
 
 # Machine accelerators
 # comma-separated list of machine accelerators to pass to the hypervisor.
```

As you can see from this snippet above, all you need to change is:
- The path to the hypervisor binary, `/home/foo/build-x86_64_virt/x86_64_virt-softmmu/qemu-system-x86_64_virt` in this example.
- The machine type from `pc` to `virt`.
- The path to the firmware binary, `/usr/share/nemu/OVMF.fd` in this example.

Once you have saved those modifications, you can start a new container:
```bash
$ docker run --runtime=kata-runtime -it busybox
```
And you will be able to verify this new container is running with the NEMU hypervisor by looking for the hypervisor path and the machine type from the `qemu` process running on your system:
```bash
$ ps -aux | grep qemu
root ... /home/foo/build-x86_64_virt/x86_64_virt-softmmu/qemu-system-x86_64_virt
...  -machine virt,accel=kvm,kernel_irqchip,nvdimm ...
```

Also relying on `kata-runtime kata-env` is a reliable way to validate you are using the expected hypervisor:
```bash
$ kata-runtime kata-env | awk -v RS= '/\[Hypervisor\]/'
[Hypervisor]
  MachineType = "virt"
  Version = "NEMU (like QEMU) version 3.0.0 (v3.0.0-179-gaf9a791)\nCopyright (c) 2003-2017 Fabrice Bellard and the QEMU Project developers"
  Path = "/home/foo/build-x86_64_virt/x86_64_virt-softmmu/qemu-system-x86_64_virt"
  BlockDeviceDriver = "virtio-scsi"
  EntropySource = "/dev/urandom"
  Msize9p = 8192
  MemorySlots = 10
  Debug = true
  UseVSock = false
```
