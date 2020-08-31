# Kata Containers with virtio-fs

- [Introduction](#introduction)
- [Pre-requisites](#pre-requisites)
- [Install Kata Containers with virtio-fs support](#install-kata-containers-with-virtio-fs-support)
- [Run a Kata Container utilizing virtio-fs](#run-a-kata-container-utilizing-virtio-fs)

## Introduction

Container deployments utilize explicit or implicit file sharing between host filesystem and containers. From a trust perspective, avoiding a shared file-system between the trusted host and untrusted container is recommended. This is not always feasible. In Kata Containers, block-based volumes are preferred as they allow usage of either device pass through or `virtio-blk` for access within the virtual machine.

As of the 1.7 release of Kata Containers, [9pfs](https://www.kernel.org/doc/Documentation/filesystems/9p.txt) is the default filesystem sharing mechanism. While this does allow for workload compatibility, it does so with degraded performance and potential for POSIX compliance limitations.

To help address these limitations, [virtio-fs](https://virtio-fs.gitlab.io/) has been developed. virtio-fs is a shared file system that lets virtual machines access a directory tree on the host. In Kata Containers, virtio-fs can be used to share container volumes, secrets, config-maps, configuration files (hostname, hosts, `resolv.conf`) and the container rootfs on the host with the guest.  virtio-fs provides significant performance and POSIX compliance improvements compared to 9pfs.

Enabling of virtio-fs requires changes in the guest kernel as well as the VMM. For Kata Containers, experimental virtio-fs support is enabled through `qemu` and `cloud-hypervisor` VMMs.

**Note: virtio-fs support is experimental in the 1.7 release of Kata Containers. Work is underway to improve stability, performance and upstream integration. This is available for early preview - use at your own risk**

This document describes how to get Kata Containers to work with virtio-fs.

## Pre-requisites

Before Kata 1.8 this feature required the host to have hugepages support enabled. Enable this with the `sysctl vm.nr_hugepages=1024` command on the host.In later versions of Kata, virtio-fs leverages `/dev/shm` as the shared memory backend. The default size of `/dev/shm` on a system is typically half of the total system memory. This can pose a physical limit to the maximum number of pods that can be launched with virtio-fs. This can be overcome by increasing the size of `/dev/shm` as shown below:

```bash
$ mount -o remount,size=${desired_shm_size} /dev/shm
```
 
## Install Kata Containers with virtio-fs support

The Kata Containers `qemu` configuration with virtio-fs and the `virtiofs` daemon are available in the [Kata Container release](https://github.com/kata-containers/runtime/releases) artifacts starting with the 1.9 release. Installation is available through [distribution packages](https://github.com/kata-containers/documentation/blob/master/install/README.md#supported-distributions) as well through [`kata-deploy`](https://github.com/kata-containers/packaging/tree/master/kata-deploy).

**Note: Support for virtio-fs was first introduced in `NEMU` hypervisor in Kata 1.8 release. This hypervisor has been deprecated.**

Install the latest release of Kata with `kata-deploy` as follows:
```
docker run --runtime=runc -v /opt/kata:/opt/kata -v /var/run/dbus:/var/run/dbus -v /run/systemd:/run/systemd -v /etc/docker:/etc/docker -it katadocker/kata-deploy kata-deploy-docker install
```

This will place the Kata release artifacts in `/opt/kata`, and update Docker's configuration to include a runtime target, `kata-qemu-virtiofs`. Learn more about `kata-deploy` and how to use `kata-deploy` in Kubernetes [here](https://github.com/kata-containers/packaging/tree/master/kata-deploy#kubernetes-quick-start).

## Run a Kata Container utilizing virtio-fs

Once installed, start a new container, utilizing `qemu` + `virtiofs`:
```bash
$ docker run --runtime=kata-qemu-virtiofs -it busybox
```

Verify the new container is running with the `qemu` hypervisor as well as using `virtiofsd`. To do this look for the hypervisor path and the `virtiofs` daemon process on the host:
```bash
$ ps -aux | grep virtiofs
root ... /home/foo/build-x86_64_virt/x86_64_virt-softmmu/qemu-system-x86_64_virt
...  -machine virt,accel=kvm,kernel_irqchip,nvdimm ...
root ... /home/foo/build-x86_64_virt/virtiofsd-x86_64 ...
```

You can also try out virtio-fs using `cloud-hypervisor` VMM:
```bash
$ docker run --runtime=kata-clh -it busybox
```
