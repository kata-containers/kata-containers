# Use SPDK vhost-user-blk volumes with Cloud Hypervisor

SPDK exports a block device from a userspace storage stack and hands it to a
virtual machine through `vhost-user-blk`. With `runtime-rs` the device is not
`mknod`-ed on the host and no kernel `vhost` driver is involved: the runtime
tells the VMM to connect to the SPDK Unix socket, and the guest sees an ordinary
virtio-blk disk.

This page covers the Cloud Hypervisor specifics: the `configuration.toml`
settings, the socket permissions, and the errors you get when a precondition is
missing. The full SPDK and `kata-ctl direct-volume` walkthrough lives in
[Setup to run SPDK vhost-user devices with Kata Containers](../use-cases/using-SPDK-vhostuser-and-kata.md).

!!! note "runtime-rs only"
    `spdkvol` and `spoolvol` are implemented by `SPDKVolume` in `runtime-rs`.
    The Go runtime has a different direct-volume implementation and is not
    described here.

!!! warning "Cloud Hypervisor support"
    Dispatching `DeviceType::VhostUserBlk` to the Cloud Hypervisor API is
    tracked in kata-containers/kata-containers#12001. Builds without that
    change reach the default branch of the Cloud Hypervisor device handler and
    fail with `unhandled device: VhostUserBlk(...)`.

## Prerequisites

| Component | Requirement |
| --- | --- |
| Hypervisor | Cloud Hypervisor, `[hypervisor.clh]` |
| Runtime | `runtime-rs` (`io.containerd.kata.v2` shim) |
| Backend | SPDK `spdk_tgt` with a `vhost_blk` controller |
| Socket | Unix stream socket, `S_IFSOCK`, connectable by the VMM user |
| Guest driver | `virtio-blk-pci` or `virtio-blk-mmio` |

The SPDK side needs a vhost target that exposes a block controller over a Unix
socket. Any current SPDK release providing the `vhost_create_blk_controller`
RPC works; build SPDK with the vhost library and DPDK enabled, as described in
the [SPDK getting started guide](https://spdk.io/doc/getting_started.html).
The controller socket is named after the second argument of
`vhost_create_blk_controller` and is created in the directory passed to
`spdk_tgt -S`.

## Configure the runtime

`SPDKVolume` takes the transport for the new device from
`block_device_driver`, so no per-volume driver is needed. The Cloud Hypervisor
template already defaults to `virtio-blk-pci`; make sure a customised
configuration does not lower it to another transport, because `SPDKVolume`
rejects anything except the two virtio-blk transports.

```toml title="configuration-clh-runtime-rs.toml ([hypervisor.clh] section)"
[hypervisor.clh]
path = "/opt/kata/bin/cloud-hypervisor"
kernel = "/opt/kata/share/kata-containers/vmlinux.container"
image = "/opt/kata/share/kata-containers/kata-containers.img"

# vhost-user-blk devices reuse this transport. `virtio-blk-pci` and
# `virtio-blk-mmio` are the only values SPDKVolume accepts.
block_device_driver = "virtio-blk-pci"

# The VMM process must be able to connect to the SPDK socket. With
# `rootless = false` Cloud Hypervisor runs as root and can reach a
# root-owned socket directory.
rootless = false
```

!!! note "Where the configuration file lives"
    Depending on the installation, the `runtime-rs` configuration is installed
    as `/opt/kata/share/defaults/kata-containers/configuration-clh-runtime-rs.toml`,
    under a `runtime-rs/` subdirectory, or below `/usr/share/defaults`, and
    `kata-deploy` writes its own copy. Edit the file used by the runtime class
    you start the workload with.

!!! tip
    If you already have a working containerd or Kubernetes configuration, check
    `block_device_driver` and `rootless` and leave everything else at the
    distribution default.

The queue geometry does not come from the configuration file. `SPDKVolume` reads
it from the volume metadata and keeps the runtime defaults when the keys are
absent:

```json title="metadata in mountInfo.json"
{
  "num_queues": "4",
  "queue_size": "256"
}
```

`num_queues` must parse as a `usize` and `queue_size` as a `u32`; anything else
is rejected before the device is created.

## Socket permissions

The socket is the only thing connecting the VMM to SPDK, and the runtime does
not relax its permissions for you.

1. The path must exist and be a Unix socket. A missing path fails with
   `stat socket failed`; a regular file or directory fails with
   `device ... is not valid`.
2. The directory holding the socket must be searchable by the user running
   Cloud Hypervisor, and the socket must be connectable by that user. With the
   default `rootless = false` that user is `root`.
3. With `rootless = true` the VMM runs as an unprivileged user, so a root-owned
   `0700` directory is not reachable. Either keep the socket in a directory the
   VMM user can traverse, grant access with an ACL, or leave `rootless`
   disabled.
4. On SELinux or AppArmor hosts, the policy applied to the Cloud Hypervisor
   process must allow connecting to the socket directory.
5. Do not place the socket on a per-container mount or a tmpfs that is
   unmounted while the container runs.

```bash title="Make the SPDK socket reachable for the VMM"
# Root VMM (default): the socket directory only has to be searchable by root.
sudo install -d -m 0750 -o root -g root /run/vhu

# Rootless VMM: grant the runtime user access explicitly instead of making
# the socket world-writable.
sudo setfacl -m u:<vmm-user>:rwx /run/vhu
sudo setfacl -m u:<vmm-user>:rw  /run/vhu/vhost-blk-rawdisk01.sock
```

`ls -l` on a correctly created controller shows the socket type, for example:

```text
srw-rw---- 1 root root 0 /run/vhu/vhost-blk-rawdisk01.sock
```

## Register the volume and start a workload

Export the block device from SPDK (see
[the SPDK walkthrough](../use-cases/using-SPDK-vhostuser-and-kata.md) for the
`spdk_tgt`, `bdev_aio_create` and `vhost_create_blk_controller` commands), then
register it with the runtime:

```bash title="Register the direct volume"
$ sudo kata-ctl direct-volume add /kubelet/kata-spdk-vol-001/volume001 \
    '{"device": "/run/vhu/vhost-blk-rawdisk01.sock",
      "volume-type": "spdkvol",
      "fstype": "ext4",
      "metadata": {},
      "options": []}'
```

The JSON keys are `volume-type` and `fstype`; `kata-ctl` writes them to
`/run/kata-containers/shared/direct-volumes/` where the runtime picks the volume
up:

```json title="/run/kata-containers/shared/direct-volumes/<encoded-path>/mountInfo.json"
{
  "volume-type": "spdkvol",
  "device": "/run/vhu/vhost-blk-rawdisk01.sock",
  "fstype": "ext4",
  "metadata": {},
  "options": []
}
```

```bash title="Run a container with the SPDK volume"
$ sudo ctr run -t --rm --runtime io.containerd.kata.v2 \
    --mount type=spdkvol,src=/kubelet/kata-spdk-vol-001/volume001,dst=/disk001,options=rbind:rw \
    quay.io/prometheus/busybox:latest kata-spdk-vol-001 sh
```

Inside the guest the volume appears as the next free virtio-blk disk, for
example `/dev/vdb` when `/dev/vda` is the root filesystem. `spoolvol` follows the
same code path; it only carries a different volume type tag and is the value
used by the SPDK spool integration.

## Verify the setup

```bash title="Check the device inside the guest"
$ lsblk
NAME   MAJ:MIN RM  SIZE RO TYPE MOUNTPOINTS
vda    254:0    0  256M  1 disk
vdb    254:16   0   20G  0 disk /disk001
```

Writing 32 MiB and comparing its checksum from a second container is the
quickest end-to-end check that the data really lives on the SPDK backend:

```bash title="Write 32 MiB and record the checksum"
$ head -c 32M /dev/urandom > /disk001/test.bin
$ sha256sum /disk001/test.bin
```

Start another container on the same volume and run `sha256sum` again; both
outputs must match.

The device is removed from the VM when the container and its sandbox are torn
down; there is no separate cleanup step for the device itself.

## Troubleshooting

!!! failure "`unhandled device: VhostUserBlk(...)`"
    The Cloud Hypervisor build in use does not dispatch vhost-user-blk devices
    yet, see kata-containers/kata-containers#12001. The same message appears
    when the volume is attached to a hypervisor driver without a vhost-user-blk
    branch.

!!! failure "`stat socket failed` when the container starts"
    The socket path in `mountInfo.json` does not exist on the host. Check that
    `spdk_tgt` is still running and that the path matches the directory used
    with `spdk_tgt -S`.

!!! failure "`device ... is not valid` when the container starts"
    The path exists but is not a socket, for example because a regular file was
    created at that location. Recreate the controller with
    `vhost_create_blk_controller`.

!!! failure "`mountinfo.json is invalid` when the container starts"
    The `volume-type` is neither `spdkvol` nor `spoolvol`, usually because of a
    typo or an outdated example that uses `volume_type`.

!!! failure "The VMM cannot connect to the socket"
    Permission or policy problem: check the directory mode, the owner of the
    socket, `rootless`, and the SELinux or AppArmor profile in effect. The
    runtime does not change socket ownership.

## Limitations

- The SPDK target must be running before the container starts. If it exits or
  loses its backing file, guest I/O fails until the container is recreated.
- `num_queues` and `queue_size` are passed to the backend; values beyond what
  the SPDK controller supports are rejected or degrade throughput.
- Only `spdkvol` and `spoolvol` reach this code path. Plain `directvol` uses the
  raw block device and never creates a vhost-user device.
