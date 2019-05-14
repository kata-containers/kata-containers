
# Kata Containers with virtio-fs

* [Introduction](#introduction)
* [Pre-requisites](#pre-requisites)
* [Configure Kata Containers](#configure-kata-containers)

## Introduction

[virtio-fs](https://virtio-fs.gitlab.io/) is a shared file system that lets virtual machines access a directory tree on the host. In Kata, virtio-fs can be used to share the rootfs of the container on the host within the guest to provide significant performance improvements over 9p (the current default sharing mechanism).

**Note: virtio-fs support is experimental in the 1.7 release of Kata Containers. Work is underway to improve stability, performance and upstream integration. This is available for early preview - use at your own risk**

This document describes how to get Kata Containers to work with virtio-fs.

## Pre-requisites

This document requires Kata Containers to be [installed](https://github.com/kata-containers/documentation/blob/master/install/README.md) on your system.

* virtio-fs is currently only available with [NEMU](https://github.com/kata-containers/documentation/blob/master/how-to/how-to-use-kata-containers-with-nemu.md)
* This feature currently requires the host to have hugepages support enabled. Enable this with the `sysctl vm.nr_hugepages=1024` command on the host.

## Configure Kata Containers

To configure Kata Containers, modify the configuration file `/usr/share/defaults/kata-containers/configuration.toml` to specify the below options related to this feature.

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
@@ -31,7 +31,7 @@ kernel_params = ""

 # Path to the firmware.
 # If you want that qemu uses the default firmware leave this option empty
-firmware = ""
+firmware = "/usr/share/nemu/OVMF.fd"

 # Machine accelerators
 # comma-separated list of machine accelerators to pass to the hypervisor.
@@ -100,10 +100,10 @@ disable_block_device_use = false
 # Shared file system type:
 #   - virtio-9p (default)
 #   - virtio-fs
-shared_fs = "virtio-9p"
+shared_fs = "virtio-fs"

 # Path to vhost-user-fs daemon.
-virtio_fs_daemon = ""
+virtio_fs_daemon = "/home/foo/build-x86_64_virt/virtiofsd-x86_64"
```

As you can see from the previous snippet, you only need to change the following:
- the path to the hypervisor binary. `/home/foo/build-x86_64_virt/x86_64_virt-softmmu/qemu-system-x86_64_virt` in this example.
- The machine name from `pc` to `virt`,
- The path of the firmware binary, `/usr/share/nemu/OVMF.fd` in this example,
- The `shared_fs` option to `virtio_fs`,
- The path of the virtiofsd daemon. `/home/foo/build-x86_64_virt/virtiofsd-x86_64` in this example.

Once you save these modifications, start a new container:
```bash
$ docker run --runtime=kata-runtime -it busybox
```
Verify the new container is running with the NEMU hypervisor as well as using virtiofsd. To do this look for the hypervisor path and the virtiofs daemon process on the host:
```bash
$ ps -aux | grep virt
root ... /home/foo/build-x86_64_virt/x86_64_virt-softmmu/qemu-system-x86_64_virt
...  -machine virt,accel=kvm,kernel_irqchip,nvdimm ...
root ... /home/foo/build-x86_64_virt/virtiofsd-x86_64 ...
```
