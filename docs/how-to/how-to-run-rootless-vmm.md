## Introduction
To improve security, Kata Container supports running the VMM process (QEMU and cloud-hypervisor) as a non-`root` user. 
This document describes how to enable the rootless VMM mode and its limitations.

## Pre-requisites
The permission and ownership of the `kvm` device node (`/dev/kvm`) need to be configured to:
```
$ crw-rw---- 1 root kvm
```
use the following commands:
```
$ sudo groupadd kvm -r
$ sudo chown root:kvm /dev/kvm
$ sudo chmod 660 /dev/kvm
```

## Configure rootless VMM
By default, the VMM process still runs as the root user. There are two ways to enable rootless VMM:
1. Set the `rootless` flag to `true` in the hypervisor section of `configuration.toml`.
2. Set the Kubernetes annotation `io.katacontainers.hypervisor.rootless` to `true`.

## Implementation details
When `rootless` flag is enabled, upon a request to create a Pod, Kata Containers runtime creates a random user and group (e.g. `kata-123`), and uses them to start the hypervisor process. 
The `kvm` group is also given to the hypervisor process as a supplemental group to give the hypervisor process access to the `/dev/kvm` device. 
Another necessary change is to move the hypervisor runtime files (e.g. `vhost-fs.sock`, `qmp.sock`) to a directory (under `/run/user/[uid]/`) where only the non-root hypervisor has access to.

## Limitations

1. Only the VMM process is running as a non-root user. Other processes such as Kata Container shimv2 and `virtiofsd` still run as the root user.
2. Currently, this feature is only supported in QEMU and cloud-hypervisor. For firecracker, you can use jailer to run the VMM process with a non-root user.
3. Certain features will not work when rootless VMM is enabled, including:
   1. Passing devices to the guest (`virtio-blk`, `virtio-scsi`) will not work if the non-privileged user does not have permission to access it (leading to a permission denied error). A more permissive permission (e.g. 666) may overcome this issue. However, you need to be aware of the potential security implications of reducing the security on such devices.
   2. `vfio` device will also not work because of permission denied error.