# Storage

## Limits

Kata Containers is [compatible](README.md#compatibility) with existing
standards and runtime. From the perspective of storage, this means no
limits are placed on the amount of storage a container
[workload](README.md#workload) may use.

Since cgroups are not able to set limits on storage allocation, if you
wish to constrain the amount of storage a container uses, consider
using an existing facility such as `quota(1)` limits or
[device mapper](#devicemapper) limits.

## virtio SCSI

If a block-based graph driver is [configured](README.md#configuration),
`virtio-scsi` is used to _share_ the workload image (such as
`busybox:latest`) into the container's environment inside the VM.

## virtio FS

If a block-based graph driver is _not_ [configured](README.md#configuration), a
[`virtio-fs`](https://virtio-fs.gitlab.io) (`VIRTIO`) overlay
filesystem mount point is used to _share_ the workload image instead. The
[agent](README.md#agent) uses this mount point as the root filesystem for the
container processes.

For virtio-fs, the [runtime](README.md#runtime) starts one `virtiofsd` daemon
(that runs in the host context) for each VM created.

## Devicemapper

The
[devicemapper `snapshotter`](https://github.com/containerd/containerd/blob/main/docs/snapshotters/devmapper.md)
is a special case. The `snapshotter` uses dedicated block devices
rather than formatted filesystems, and operates at the block level
rather than the file level. This knowledge is used to directly use the
underlying block device instead of the overlay file system for the
container root file system. The block device maps to the top
read-write layer for the overlay. This approach gives much better I/O
performance compared to using `virtio-fs` to share the container file
system.

#### Hot plug and unplug

Kata Containers has the ability to hot plug add and hot plug remove
block devices. This makes it possible to use block devices for
containers started after the VM has been launched.

Users can check to see if the container uses the `devicemapper` block
device as its rootfs by calling `mount(8)` within the container. If
the `devicemapper` block device is used, the root filesystem (`/`)
will be mounted from `/dev/vda`. Users can disable direct mounting of
the underlying block device through the runtime
[configuration](README.md#configuration).
