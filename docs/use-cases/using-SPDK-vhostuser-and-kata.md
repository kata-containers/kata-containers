# Setup to run SPDK vhost-user devices with Kata Containers

> **Note:** This guide only applies to QEMU, since the vhost-user storage
> device is only available for QEMU now. The enablement work on other
> hypervisors is still ongoing.

## SPDK vhost-user Target Overview

The Storage Performance Development Kit (SPDK) provides a set of tools and
libraries for writing high performance, scalable, user-mode storage applications.

virtio, vhost and vhost-user:
- virtio is an efficient way to transport data for virtual environments and
guests. It is most commonly used in QEMU VMs, where the VM itself exposes a
virtual PCI device and the guest OS communicates with it using a specific virtio
PCI driver. Its diagram is:
```
+---------+------+--------+----------+--+
|         +------+-------------------+  |
|         |            +----------+  |  |
| user    |            |          |  |  |
| space   |            |  guest   |  |  |
|         |            |          |  |  |
|    +----+ qemu       | +-+------+  |  |
|    |    |            | | virtio |  |  |
|    |    |            | | driver |  |  |
|    |    |            +-+---++---+  |  |
|    |    +------+-------------------+  |
|    |       ^               |          |
|    |       |               |          |
|    v       |               v          |
+-+------+---+------------+--+-------+--+
| |block |   +------------+ kvm.ko   |  |
| |device|                |          |  |
| +------+                +--+-------+  |
|           host kernel                 |
+---------------------------------------+
```

- vhost is a protocol for devices accessible via inter-process communication. It
uses the same virtio queue layout as virtio to allow vhost devices to be mapped
directly to virtio devices. The initial vhost implementation is a part of the
Linux kernel and uses an ioctl interface to communicate with userspace
applications. Its diagram is:
```
+---------+------+--------+----------+--+
|         +------+-------------------+  |
|         |            +----------+  |  |
| user    |            |          |  |  |
| space   |            |  guest   |  |  |
|         |            |          |  |  |
|         | qemu       | +-+------+  |  |
|         |            | | virtio |  |  |
|         |            | | driver |  |  |
|         |            +-+-----++-+  |  |
|         +------+-------------------+  |
|                               |       |
|                               |       |
+-+------+--+-------------+--+--v-------+
| |block |  |vhost-scsi.ko|  | kvm.ko   |
| |device|  |             |  |          |
| +---^--+  +-v---------^-+  +--v-------+
|     |       |   host  |       |       |
|     +-------+  kernel +-------+       |
+---------------------------------------+
```

- vhost-user implements the control plane through Unix domain socket to establish
virtio queue sharing with a user space process on the same host. SPDK exposes
vhost devices via the vhost-user protocol. Its diagram is:
```
+----------------+------+--+----------+-+
|                +------+-------------+ |
| user           |      +----------+  | |
| space          |      |          |  | |
|                |      |  guest   |  | |
|  +-+-------+   | qemu | +-+------+  | |
|  | vhost   |   |      | | virtio |  | |
|  | backend |   |      | | driver |  | |
|  +-^-^---^-+   |      +-+--+-----+  | |
|    | |   |     |           |        | |
|    | |   |     +--+---+----V------+-+ |
|    | |   |        |        |      |   |
|    | |  ++--------+--+     |      |   |
|    | |  |unix sockets|     |      |   |
|    | |  +------------+     |      |   |
|    | |                     |      |   |
|    | |  +-------------+    |      |   |
|    | +--|shared memory|<---+      |   |
+----+----+-------------+---+--+----+---+
|    |                      |           |
|    +----------------------+ kvm.ko    |
|                           +--+--------+
|           host kernel                 |
+---------------------------------------+
```

SPDK vhost is a vhost-user slave server. It exposes Unix domain sockets and
allows external applications to connect. It is capable of exposing virtualized
storage devices to QEMU instances or other arbitrary processes.

Currently, the SPDK vhost-user target can exposes these types of virtualized
devices:

- `vhost-user-blk`
- `vhost-user-scsi`
- `vhost-user-nvme` (deprecated from SPDK 21.07 release)

For more information, visit [SPDK](https://spdk.io) and [SPDK vhost-user target](https://spdk.io/doc/vhost.html).

## Install and setup SPDK vhost-user target

### Get source code and build SPDK

Following the SPDK [getting started guide](https://spdk.io/doc/getting_started.html).

### Run SPDK vhost-user target

First, run the SPDK `setup.sh` script to setup some hugepages for the SPDK vhost
target application. We recommend you use a minimum of 4GiB, enough for the SPDK
vhost target and the virtual machine.
This will allocate 4096MiB (4GiB) of hugepages, and avoid binding PCI devices:

```bash
$ sudo HUGEMEM=4096 PCI_WHITELIST="none" scripts/setup.sh
```

Then, take directory `/var/run/kata-containers/vhost-user` as Kata's vhost-user
device directory. Make subdirectories for vhost-user sockets and device nodes:

```bash
$ sudo mkdir -p /var/run/kata-containers/vhost-user/
$ sudo mkdir -p /var/run/kata-containers/vhost-user/block/
$ sudo mkdir -p /var/run/kata-containers/vhost-user/block/sockets/
$ sudo mkdir -p /var/run/kata-containers/vhost-user/block/devices/
```

For more details, see section [Host setup for vhost-user devices](#host-setup-for-vhost-user-devices).

Next, start the SPDK vhost target application.  The following command will start
vhost on the first CPU core with all future socket files placed in
`/var/run/kata-containers/vhost-user/block/sockets/`:

```bash
$ sudo app/spdk_tgt/spdk_tgt -S /var/run/kata-containers/vhost-user/block/sockets/ &
```

To list all available vhost options run the following command:

```bash
$ app/spdk_tgt/spdk_tgt -h
```

Create an experimental `vhost-user-blk` device based on memory directly:

- The following RPC will create a 64MB memory block device named `Malloc0`
with 4096-byte block size:

```bash
$ sudo scripts/rpc.py bdev_malloc_create 64 4096 -b Malloc0
```

- The following RPC will create a `vhost-user-blk` device exposing `Malloc0`
block device. The device will be accessible via
`/var/run/kata-containers/vhost-user/block/sockets/vhostblk0`:

```bash
$ sudo scripts/rpc.py vhost_create_blk_controller vhostblk0 Malloc0
```

## Host setup for vhost-user devices

Considering the OCI specification and characteristics of vhost-user device,
Kata has chosen to use Linux reserved the block major range `240-254`
to map each vhost-user block type to a major. Also a specific directory is
used for vhost-user devices.

The base directory for vhost-user device is a configurable value,
with the default being `/var/run/kata-containers/vhost-user`. It can be
configured by parameter `vhost_user_store_path` in [Kata TOML configuration file](../../src/runtime/README.md#configuration).

Currently, the vhost-user storage device is not enabled by default, so
the user should enable it explicitly inside the Kata TOML configuration
file by setting `enable_vhost_user_store = true`. Since SPDK vhost-user target
requires hugepages, hugepages should also be enabled inside the Kata TOML
configuration file by setting `enable_hugepages = true`.
Here is the conclusion of parameter setting for vhost-user storage device:

```toml
enable_hugepages = true
enable_vhost_user_store = true
vhost_user_store_path = "<Path of the base directory for vhost-user device>"
```

> **Note:** These parameters are under `[hypervisor.qemu]` section in Kata
> TOML configuration file. If they are absent, users should still add them
> under `[hypervisor.qemu]` section.


For the subdirectories of `vhost_user_store_path`:
-  `block` is used for block device;
-  `block/sockets` is where we expect UNIX domain sockets for vhost-user
block devices to live;
-  `block/devices` is where simulated block device nodes for vhost-user
block devices are created.

For example, if using the default directory `/var/run/kata-containers/vhost-user`,
UNIX domain sockets for vhost-user block device are under `/var/run/kata-containers/vhost-user/block/sockets/`.
Device nodes for vhost-user block device are under `/var/run/kata-containers/vhost-user/block/devices/`.

Currently, Kata has chosen major number 241 to map to `vhost-user-blk` devices.
For `vhost-user-blk` device named `vhostblk0`, a UNIX domain socket is already
created by SPDK vhost target, and a block device node with major `241` and
minor `0` should be created for it, in order to be recognized by Kata runtime:

```bash
$ sudo mknod /var/run/kata-containers/vhost-user/block/devices/vhostblk0 b 241 0
```

## Launch a Kata container with SPDK vhost-user block device

To use `vhost-user-blk` device, use `ctr` to pass a host `vhost-user-blk`
device to the container. In your `config.json`, you should use `devices`
to pass a host device to the container.

For example (only `vhost-user-blk` listed):

```json
{
  "linux": {
    "devices": [
      {
        "path": "/dev/vda",
        "type": "b",
        "major": 241,
        "minor": 0,
        "fileMode": 420,
        "uid": 0,
        "gid": 0
      }
    ]
  }
}
```

With `rootfs` provisioned under `bundle` directory, you can run your SPDK container:

```bash
$ sudo ctr run -d --runtime io.containerd.run.kata.v2 --config bundle/config.json spdk_container
```

Example of performing I/O operations on the `vhost-user-blk` device inside
container:

```
$ sudo ctr t exec --exec-id 1 -t spdk_container sh
/ # ls -l /dev/vda
brw-r--r--    1 root     root      254,   0 Jan 20 03:54 /dev/vda
/ # dd if=/dev/vda of=/tmp/ddtest bs=4k count=20
20+0 records in
20+0 records out
81920 bytes (80.0KB) copied, 0.002996 seconds, 26.1MB/s
```
