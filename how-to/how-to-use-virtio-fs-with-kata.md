# Kata Containers with virtio-fs

- [Introduction](#introduction)
- [Pre-requisites](#pre-requisites)
- [Install Kata Containers with virtio-fs support](#install-kata-containers-with-virtio-fs-support)
- [Run a Kata Container utilizing virtio-fs](#run-a-kata-container-utilizing-virtio-fs)

## Introduction

Container deployments utilize explicit or implicit file sharing between host filesystem and containers. From a trust perspective, avoiding a shared file-system between the trusted host and untrusted container is recommended. This is not always feasible. In Kata Containers, block-based volumes are preferred as they allow usage of either device pass through or `virtio-blk` for access within the virtual machine.

As of the 1.7 release of Kata Containers, [9pfs](https://www.kernel.org/doc/Documentation/filesystems/9p.txt) is the default filesystem sharing mechanism. While this does allow for workload compatibility, it does so with degraded performance and potential for POSIX compliance limitations.

To help address these limitations, [virtio-fs](https://virtio-fs.gitlab.io/) has been developed. virtio-fs is a shared file system that lets virtual machines access a directory tree on the host. In Kata Containers, virtio-fs can be used to share container volumes, secrets, config-maps, configuration files (hostname, hosts, `resolv.conf`) and the container rootfs on the host with the guest.  virtio-fs provides significant performance and POSIX compliance improvements compared to 9pfs.

Enabling of virtio-fs requires changes in the guest kernel as well as the VMM. For Kata Containers, experimental virtio-fs support is enabled through the [NEMU VMM](https://github.com/intel/nemu).

**Note: virtio-fs support is experimental in the 1.7 release of Kata Containers. Work is underway to improve stability, performance and upstream integration. This is available for early preview - use at your own risk**

This document describes how to get Kata Containers to work with virtio-fs.

## Pre-requisites

* This feature currently requires the host to have hugepages support enabled. Enable this with the `sysctl vm.nr_hugepages=1024` command on the host.

## Install Kata Containers with virtio-fs support

The Kata Containers NEMU configuration, the NEMU VMM and the `virtiofs` daemon are available in the [Kata Container release](https://github.com/kata-containers/runtime/releases) artifacts starting with the 1.7 release. While the feature is experimental, distribution packages are not supported, but installation is available through [`kata-deploy`](https://github.com/kata-containers/packaging/tree/master/kata-deploy).

Install the latest release of Kata as follows:
```
docker run --runtime=runc -v /opt/kata:/opt/kata -v /var/run/dbus:/var/run/dbus -v /run/systemd:/run/systemd -v /etc/docker:/etc/docker -it katadocker/kata-deploy kata-deploy-docker install
```

This will place the Kata release artifacts in `/opt/kata`, and update Docker's configuration to include a runtime target, `kata-nemu`. Learn more about `kata-deploy` and how to use `kata-deploy` in Kubernetes [here](https://github.com/kata-containers/packaging/tree/master/kata-deploy#kubernetes-quick-start).


## Run a Kata Container utilizing virtio-fs

Once installed, start a new container, utilizing NEMU + `virtiofs`:
```bash
$ docker run --runtime=kata-nemu -it busybox
```

Verify the new container is running with the NEMU hypervisor as well as using `virtiofsd`. To do this look for the hypervisor path and the `virtiofs` daemon process on the host:
```bash
$ ps -aux | grep virtiofs
root ... /home/foo/build-x86_64_virt/x86_64_virt-softmmu/qemu-system-x86_64_virt
...  -machine virt,accel=kvm,kernel_irqchip,nvdimm ...
root ... /home/foo/build-x86_64_virt/virtiofsd-x86_64 ...
```
