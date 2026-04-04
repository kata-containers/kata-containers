# Use EROFS Snapshotter with Kata Containers (runtime-rs)

## Project Overview

The [EROFS snapshotter](https://erofs.docs.kernel.org) is a native containerd
snapshotter that converts OCI container image layers into EROFS-formatted blobs.
When used with Kata Containers `runtime-rs`, the EROFS snapshotter enables
**block-level image pass-through** to the guest VM, bypassing virtio-fs / 9p
entirely. This delivers lower overhead, better performance, and smaller memory
footprints compared to traditional shared-filesystem approaches.

## Quick Start Guide

This section provides a quick overview of the steps to get started with EROFS snapshotter and Kata Containers. For detailed instructions, see the [Installation Guide](#installation-guide) section.

### Quick Steps

1. **Install erofs-utils**: Install erofs-utils (version >= 1.7) on your host system
2. **Configure containerd**: Enable EROFS snapshotter and differ in containerd configuration
3. **Configure Kata Containers**: Set up runtime-rs with appropriate hypervisor settings
4. **Run a container**: Use `ctr` or Kubernetes to run containers with EROFS snapshotter

### Prerequisites

| Component | Version Requirement |
|-----------|-------------------|
| Linux kernel | >= 5.4 (with `erofs` module) |
| erofs-utils | >= 1.7 (>= 1.8 recommended) |
| containerd | >= 2.2 (with EROFS snapshotter and differ support) |
| Kata Containers | Latest `main` branch with runtime-rs |
| QEMU | >= 5.0 (VMDK flat-extent support and >= 8.0 recommended) |

## Installation Guide

This section provides detailed step-by-step instructions for installing and configuring EROFS snapshotter with Kata Containers.

### Step 1: Install erofs-utils

```bash
# Debian/Ubuntu
$ sudo apt install erofs-utils

# Fedora
$ sudo dnf install erofs-utils
```

Verify the version:

```bash
$ mkfs.erofs --version
# Should show 1.7 or higher
```

Load the kernel module:

```bash
$ sudo modprobe erofs
```

### Step 2: Configure containerd

#### Enable the EROFS snapshotter and differ

Edit your containerd configuration (typically `/etc/containerd/config.toml`):

```toml
version = 3
...
  [plugins.'io.containerd.cri.v1.runtime']
    ...
    [plugins.'io.containerd.cri.v1.runtime'.containerd]
      ...
      [plugins.'io.containerd.cri.v1.runtime'.containerd.runtimes]
        [plugins.'io.containerd.cri.v1.runtime'.containerd.runtimes.kata]
          runtime_type = 'io.containerd.kata.v2'
          pod_annotations = ["*"]
          container_annotations = ["*"]
          privileged_without_host_devices = false
          cni_max_conf_num = 0
          snapshotter = ''
          sandboxer = 'podsandbox'
  ...

  [plugins.'io.containerd.differ.v1.erofs']
    mkfs_options = ["-T0", "--mkfs-time", "--sort=none"]
    enable_tar_index = false

  [plugins.'io.containerd.service.v1.diff-service']
    default = ['erofs', 'walking']
    sync_fs = false

  [plugins.'io.containerd.snapshotter.v1.erofs']
    root_path = ''
    ovl_mount_options = []
    enable_fsverity = false
    set_immutable = false
    default_size = '<SIZE>' # SIZE=6G or 10G or other size 
    max_unmerged_layers = 1
```

#### Verify the EROFS plugins are loaded

Check if EROFS module is loaded

```bash
$ lsmod | grep erofs
erofs                 188416  0
netfs                 614400  1 erofs
```

If not loaded:

```bash
$ sudo modprobe erofs
```

Restart containerd and check:

```bash
$ sudo systemctl restart containerd
$ sudo ctr plugins ls | grep erofs
io.containerd.snapshotter.v1    erofs    linux/amd64    ok
io.containerd.differ.v1         erofs    linux/amd64    ok
```

Check containerd snapshotter status

```bash
$ sudo ctr plugins ls | grep erofs
io.containerd.mount-handler.v1            erofs                    linux/amd64    ok        
io.containerd.snapshotter.v1              erofs                    linux/amd64    ok        
io.containerd.differ.v1                   erofs                    linux/amd64    ok 
```

Both `snapshotter` and `differ` should show `ok`.

### Step 3: Configure Kata Containers (runtime-rs)

Edit the Kata configuration file (e.g.,
`configuration-qemu-runtime-rs.toml`):

```toml
[hypervisor.qemu]
# shared_fs can be set to "none" since EROFS layers are passed via
# block devices, not via virtio-fs. If you still need virtio-fs for
# other purposes (e.g., file sharing), keep "virtio-fs".
# For pure block-device EROFS mode:
shared_fs = "none"
```

> **Note**: The `shared_fs = "none"` setting is for the case where all
> container images use the EROFS snapshotter. If you have a mixed environment,
> keep `shared_fs = "virtio-fs"` so that non-EROFS containers can still use
> virtio-fs.


### Quick Test

Once the installation is complete, you can quickly test with:

Using `ctr` for example.

```bash
# Pull the image
$ sudo ctr image pull docker.io/library/wordpress:latest

# Run with EROFS snapshotter and Kata runtime-rs
$ sudo ctr run --runtime io.containerd.kata.v2 --snapshotter=erofs --rm -t library/wordpress:latest test001 date
Wed Apr  1 07:10:53 UTC 2026

$ sudo ctr run --runtime io.containerd.kata.v2 --snapshotter=erofs --rm -t wordpress:latest test001 lsblk
NAME   MAJ:MIN RM   SIZE RO TYPE MOUNTPOINTS
vda    254:0    0   256M  0 disk 
`-vda1 254:1    0   253M  0 part 
vdb    254:16   0     6G  0 disk 
vdc    254:32   0 759.7M  0 disk 
```

> **Note**: Ensure that the containerd CRI configuration maps the `kata`
> handler to the Kata runtime with `snapshotter = "erofs"` as shown in
> [Step 2](#step-2-configure-containerd).

### Architecture

The following diagram illustrates the data flow:

```
  Host                                        Guest VM
  ====                                        ========

  containerd                                  kata-agent
      |                                           |
      v                                           v
  EROFS snapshotter                           1. mount ext4 /dev/vdX
      |                                          (writable upper)
      |-- Mount[0]: ext4 rw layer                 |
      |     (block device on host)            2. mount erofs /dev/vdY
      |                                          (read-only lower)
      |-- Mount[1]: erofs layers                  |
      |     source: layer.erofs               3. overlay mount
      |     device=extra1.erofs                  lowerdir=<erofs_mount>
      |     device=extra2.erofs                  upperdir=<ext4_mount>/upper
      |                                          workdir=<ext4_mount>/work
      v                                           |
  runtime-rs                                      v
      |                                       container rootfs
      |-- single erofs: attach as Raw            ready
      |-- multi erofs:  generate VMDK
      |     descriptor + attach as Vmdk
      |
      v
  QEMU (virtio-blk)
      |-- /dev/vdX: ext4 rw layer
      |-- /dev/vdY: erofs layer(s)
```

### VMDK flat-extent descriptor (multi-layer case)

VMDK Descriptor Format (twoGbMaxExtentFlat)
The descriptor follows the [VMware Virtual Disk Format specification](https://github.com/libyal/libvmdk/blob/main/documentation/VMWare%20Virtual%20Disk%20Format%20(VMDK).asciidoc):

- Header: `# Disk DescriptorFile` marker and version info
- Extent descriptions: `RW <sectors> FLAT "<filename>" <offset>`
  - `sectors`: number of 512-byte sectors for this extent
  - `filename`: absolute path to the backing file
  - `offset`: starting sector offset within the file (0-based)
- DDB (Disk Data Base): virtual hardware and geometry metadata

Files larger than 2GB are automatically split into multiple extents
(MAX_2GB_EXTENT_SECTORS per extent) as required by the twoGbMaxExtentFlat format.

When multiple EROFS layers are merged, `runtime-rs` generates a VMDK
descriptor file (`twoGbMaxExtentFlat` format):

```
# Disk DescriptorFile
version=1
CID=fffffffe
parentCID=ffffffff
createType="twoGbMaxExtentFlat"

# Extent description
RW 2048 FLAT "/path/to/fsmeta.erofs" 0
RW 4096 FLAT "/path/to/layer1.erofs" 0
RW 8192 FLAT "/path/to/layer2.erofs" 0

# The Disk Data Base
#DDB

ddb.virtualHWVersion = "4"
ddb.geometry.cylinders = "15"
ddb.geometry.heads = "16"
ddb.geometry.sectors = "63"
ddb.adapterType = "ide"
```

QEMU's VMDK driver reads this descriptor and presents all extents as a
single contiguous block device to the guest. The guest kernel's EROFS driver
then mounts this combined device with multi-device support.

### How it works

The containerd EROFS snapshotter prepares a multi-layer rootfs layout:

```
Mount[0]: ext4  rw layer       --> virtio-blk device (writable upper layer)
Mount[1]: erofs layers          --> virtio-blk device (read-only, via VMDK for multi-extent)
Mount[2]: overlay               --> guest agent mounts overlay combining upper + lower
```

For the EROFS read-only layers:

- **Single layer**: The single `.erofs` blob is attached directly as a raw
  virtio-blk device.
- **Multiple layers**: Multiple `.erofs` blobs (the base layer + `device=`
  extra layers) are merged into a single virtual block device using a VMDK
  flat-extent descriptor (`twoGbMaxExtentFlat` format). QEMU's VMDK driver
  parses the descriptor and concatenates all extents transparently.

Inside the guest VM, the kata-agent:

1. Mounts the ext4 block device as the writable upper layer.
2. Mounts the erofs block device as the read-only lower layer.
3. Creates an overlay filesystem combining the two.

### Verify QEMU VMDK support

The multi-layer EROFS rootfs relies on QEMU's VMDK block driver to present
a VMDK flat-extent descriptor as a single virtual disk. QEMU must be compiled
with VMDK format support enabled (this is typically on by default, but some
minimal or custom builds may disable it).

Run the following command to check:

```bash
$ qemu-system-x86_64 -drive format=help 2>&1 | grep vmdk
```

You should see `vmdk` in the `Supported formats` list, for example:

```
Supported formats: blkdebug blklogwrites blkreplay blkverify bochs cloop
compress copy-before-write copy-on-read dmg file ftp ftps host_cdrom
host_device http https luks nbd null-aio null-co nvme parallels preallocate
qcow qcow2 qed quorum raw replication snapshot-access ssh throttle vdi vhdx
vmdk vpc vvfat
```

If `vmdk` does not appear, you need to rebuild QEMU with VMDK support enabled.

#### Build the guest components

The guest kernel must have `CONFIG_EROFS_FS=y` (or `=m` with the module
auto-loaded). The kata-agent in the guest image must include multi-layer
EROFS support.

Refer to [how-to-use-erofs-build-rootfs.md](how-to-use-erofs-build-rootfs.md)
for building a guest rootfs with EROFS support.

### Limitations

> **Hypervisor support**: The fsmerged EROFS rootfs feature currently **only
> supports QEMU** as the hypervisor, because it depends on the VMDK
> flat-extent descriptor format for merging multiple EROFS layers into a
> single block device. The following hypervisors do **not** support VMDK
> format block devices at this time, and therefore **cannot** be used with
> fsmerged EROFS rootfs:
>
> - **Cloud Hypervisor (CLH)** — no VMDK block device support ([WIP](https://github.com/cloud-hypervisor/cloud-hypervisor/issues/7167))
> - **Firecracker** — no VMDK block device support ([WIP](https://github.com/firecracker-microvm/firecracker/pull/5741))
> - **Dragonball** — no VMDK block device support (TODO)
>
> For single-layer EROFS (only one `.erofs` blob, no `device=` extra layers),
> the blob is attached as a raw block device without a VMDK descriptor. This
> mode may work with other hypervisors that support raw virtio-blk devices,
> but has not been fully tested.

## References

- [EROFS documentation](https://erofs.docs.kernel.org)
- [Containerd EROFS snapshotter](https://github.com/containerd/containerd/blob/main/docs/snapshotters/erofs.md)
- [Configure Kata to use EROFS build rootfs](how-to-use-erofs-build-rootfs.md)
- [Kata Containers architecture](../design/architecture)
