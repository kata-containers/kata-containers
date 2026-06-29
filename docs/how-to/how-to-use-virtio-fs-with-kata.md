# Kata Containers with virtio-fs

## Introduction

Container deployments utilize explicit or implicit file sharing between host filesystem and containers. From a trust perspective, avoiding a shared file-system between the trusted host and untrusted container is recommended. This is not always feasible. In Kata Containers, block-based volumes are preferred as they allow usage of either device pass through or `virtio-blk` for access within the virtual machine.

As of the 2.0 release of Kata Containers, [virtio-fs](https://virtio-fs.gitlab.io/) is the default filesystem sharing mechanism. In Kata Containers, virtio-fs can be used to share container volumes, secrets, config-maps, configuration files (hostname, hosts, `resolv.conf`), and the container rootfs on the host with the guest.  `virtio-fs` provides significant performance and POSIX compliance improvements over `9pfs`.

`virtio-fs` leverages `/dev/shm` as the shared memory backend. The default size of `/dev/shm` on a system is typically half of the total system memory. This can pose a physical limit to the maximum number of Pods that can be launched with `virtio-fs`. This can be overcome by increasing the size of `/dev/shm` as shown below:

```bash
$ mount -o remount,size=${desired_shm_size} /dev/shm
```

`virtio-fs` support works out of the box for `cloud-hypervisor` and `qemu`, when Kata Containers is deployed using `kata-deploy`. Learn more about `kata-deploy` and how to use `kata-deploy` in Kubernetes [here](../../tools/packaging/kata-deploy/helm-chart/README.md).
