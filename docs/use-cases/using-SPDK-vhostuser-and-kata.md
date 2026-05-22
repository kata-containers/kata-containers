# Setup to run SPDK vhost-user devices with Kata Containers

> **Note:** This guide applies to both **runtime-rs with Dragonball** and **QEMU** hypervisors. For runtime-rs, the procedure is simplified as there is no need to manually create device nodes.

## SPDK vhost-user Target Overview

The Storage Performance Development Kit (SPDK) provides a set of tools and libraries for writing high performance, scalable, user-mode storage applications.

virtio, vhost and vhost-user:

- virtio is an efficient way to transport data for virtual environments and guests. It is most commonly used in QEMU VMs, where the VM itself exposes a virtual PCI device and the guest OS communicates with it using a specific virtio PCI driver. Its diagram is:

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

- vhost-user implements the control plane through Unix domain socket to establish virtio queue sharing with a user space process on the same host. SPDK exposes vhost devices via the vhost-user protocol. Its diagram is:

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

SPDK vhost is a vhost-user slave server. It exposes Unix domain sockets and allows external applications to connect. It is capable of exposing virtualized storage devices to QEMU instances or other arbitrary processes.

Currently, the SPDK vhost-user target can expose several types of virtualized devices, but the most commonly used one in Kata Containers is the block device, which is supported by both runtime-rs with Dragonball and QEMU hypervisors:

- `vhost-user-blk`

A block device that can be used as a regular block device in the guest. It is suitable for workloads that require high performance and low latency, such as databases or high I/O applications.

For more information, visit [SPDK](https://spdk.io) and [SPDK vhost-user target](https://spdk.io/doc/vhost.html).

## Prerequisites

- A Kubernetes cluster with Kata Containers enabled (runtime-rs with Dragonball or QEMU)
- SPDK built and `spdk_tgt` available
- For Kubernetes CSI integration: `csi-kata-directvolume` deployed

## Method 1: Using CSI Driver (Recommended for Kubernetes)

This is the recommended method for Kubernetes environments, leveraging the `csi-kata-directvolume` CSI driver.

### 1. Start SPDK Service

```sh
export SPDK_DEVEL=<path-to-your-spdk>
export VHU_UDS_PATH=/var/lib/spdk/vhost

# Reset and allocate hugepages
$ cd $SPDK_DEVEL
$ sudo ./scripts/setup.sh reset
$ sudo sysctl -w vm.nr_hugepages=2048
$ sudo HUGEMEM=4096 ./scripts/setup.sh

# Start SPDK vhost target
$ sudo mkdir -p $VHU_UDS_PATH
$ sudo $SPDK_DEVEL/build/bin/spdk_tgt -S $VHU_UDS_PATH -s 1024 -m 0x3 &
```

> **Notes:**

> - `-s 1024`: size of the hugepage memory pool in MB.
> - `-m 0x3`: CPU mask specifying which cores SPDK will use.
> - If `vfio-pci` driver is supported, use `DRIVER_OVERRIDE=vfio-pci` with setup.sh.

### 2. Deploy CSI Driver and Kubernetes Resources

Deploy the CSI driver following the [deployment guide](../../src/tools/csi-kata-directvolume/docs/deploy-csi-kata-directvol.md).

Create StorageClass, PVC, and Pod:

```sh
$ cd kata-containers/src/tools/csi-kata-directvolume/examples/pod-with-spdkvol
$ kubectl apply -f csi-storageclass.yaml
$ kubectl apply -f csi-pvc.yaml
$ kubectl apply -f csi-app.yaml
```

This creates:

- Storage Class `spdk-test-adapted` with `volumetype=spdkvol`
- PVC `kata-spdk-directvolume-pvc`
- Pod `spdk-pod-test`

### 3. Verify the Volume

Check the mounted block device inside the pod:

```sh
$ kubectl exec -it spdk-pod-test -- /bin/sh

$ lsblk
NAME   MAJ:MIN RM  SIZE RO TYPE MOUNTPOINTS
vda    254:0    0  256M  1 disk
└─vda1 254:1    0  253M  1 part
vdb    254:16   0    2G  0 disk /data

$ echo "hello spdk" > /data/test.txt
$ cat /data/test.txt
hello spdk
```

The SPDK-backed volume `/dev/vdb` is mounted to `/data` inside the container.

### 4. Cleanup

```sh
$ kubectl delete -f csi-app.yaml
$ kubectl delete -f csi-pvc.yaml
$ kubectl delete -f csi-storageclass.yaml
```

## Method 2: Using kata-ctl direct-volume (For Manual Setup)

This method is suitable for manual testing or non-Kubernetes environments using containerd.

### 1. Start SPDK vhost target and Create Block Device

```bash
$ export SPDK_DEVEL=<path-to-your-spdk>
$ export VHU_UDS_PATH=/tmp/vhu-targets
$ export RAW_DISKS=<your-rawdisk-path> # e.g., export RAW_DISKS=/tmp/rawdisks

# Reset and setup hugepages
$ sudo ${SPDK_DEVEL}/scripts/setup.sh reset
$ sudo sysctl -w vm.nr_hugepages=2048
$ sudo HUGEMEM=4096 DRIVER_OVERRIDE=vfio-pci ${SPDK_DEVEL}/scripts/setup.sh

# Start SPDK vhost target
$ sudo ${SPDK_DEVEL}/build/bin/spdk_tgt -S $VHU_UDS_PATH -s 1024 -m 0x3 &
```

Create a vhost controller:

```bash
# Create raw disk
$ mkdir -p "${RAW_DISKS}" # ensure the directory exists
$ sudo dd if=/dev/zero of=${RAW_DISKS}/rawdisk01.20g bs=1M count=20480

# Create AIO bdev
$ sudo ${SPDK_DEVEL}/scripts/rpc.py bdev_aio_create ${RAW_DISKS}/rawdisk01.20g vhu-rawdisk01.20g 512

# Create vhost-user-blk controller
$ sudo ${SPDK_DEVEL}/scripts/rpc.py vhost_create_blk_controller vhost-blk-rawdisk01.sock vhu-rawdisk01.20g
```

A vhost controller `vhost-blk-rawdisk01.sock` is created under `$VHU_UDS_PATH/`.

### 2. Configure Direct Volume with kata-ctl

For runtime-rs with Dragonball, there is no need to manually create device nodes. Use `kata-ctl direct-volume add`:

```bash
# Add direct volume
$ sudo kata-ctl direct-volume add /kubelet/kata-test-vol-001/volume001 "{\"device\": \"${VHU_UDS_PATH}/vhost-blk-rawdisk01.sock\", \"volume_type\":\"spdkvol\", \"fs_type\": \"ext4\", \"metadata\":{}, \"options\": []}"
```

The volume info is stored at `/run/kata-containers/shared/direct-volumes/` with encoded path.

### 3. Run a Kata Container

```bash
# For runtime-rs with Dragonball
# IMAGE=docker.io/library/ubuntu:latest
$ sudo ctr run -t --rm --runtime io.containerd.kata.v2 \
  --mount type=spdkvol,src=/kubelet/kata-test-vol-001/volume001,dst=/disk001,options=rbind:rw \
  "$IMAGE" kata-spdk-vol-test /bin/bash
```

Inside the container, the SPDK volume will be available at `/disk001`.

## Additional Resources

- [How to run Kata Containers with Kinds of Block Volumes](../how-to/how-to-run-kata-containers-with-kinds-of-Block-Volumes.md)
- [CSI Direct Volume Driver README](../../src/tools/csi-kata-directvolume/README.md)
- [SPDK Usage Guide for CSI](../../src/tools/csi-kata-directvolume/docs/spdk-usage.md)
- [Direct Block Device Assignment Design](../design/direct-blk-device-assignment.md)

