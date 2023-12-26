# A new way for Kata Containers to use Kinds of Block Volumes

> **Note:** This guide is only available for runtime-rs with default Hypervisor Dragonball.
> Now, other hypervisors are still ongoing, and it'll be updated when they're ready.


## Background

Currently, there is no widely applicable and convenient method available for users to use some kinds of backend storages, such as File on host based block volume, SPDK based volume or VFIO device based volume for Kata Containers, so we adopt [Proposal: Direct Block Device Assignment](https://github.com/kata-containers/kata-containers/blob/main/docs/design/direct-blk-device-assignment.md) to address it.

## Solution

According to the proposal, it requires to use the `kata-ctl direct-volume` command to add a direct assigned block volume device to the Kata Containers runtime. 

And then with the help of method [get_volume_mount_info](https://github.com/kata-containers/kata-containers/blob/099b4b0d0e3db31b9054e7240715f0d7f51f9a1c/src/libs/kata-types/src/mount.rs#L95), get information from JSON file: `(mountinfo.json)` and parse them into structure [Direct Volume Info](https://github.com/kata-containers/kata-containers/blob/099b4b0d0e3db31b9054e7240715f0d7f51f9a1c/src/libs/kata-types/src/mount.rs#L70) which is used to save device-related information. 

We only fill the `mountinfo.json`, such as `device` ,`volume_type`, `fs_type`, `metadata` and `options`, which correspond to the fields in [Direct Volume Info](https://github.com/kata-containers/kata-containers/blob/099b4b0d0e3db31b9054e7240715f0d7f51f9a1c/src/libs/kata-types/src/mount.rs#L70), to describe a device. 

The JSON file `mountinfo.json` placed in a sub-path `/kubelet/kata-test-vol-001/volume001` which under fixed path `/run/kata-containers/shared/direct-volumes/`. 
And the full path looks like: `/run/kata-containers/shared/direct-volumes/kubelet/kata-test-vol-001/volume001`, But for some security reasons. it is 
encoded as `/run/kata-containers/shared/direct-volumes/L2t1YmVsZXQva2F0YS10ZXN0LXZvbC0wMDEvdm9sdW1lMDAx`.

Finally, when running a Kata Containers with `ctr run --mount type=X, src=Y, dst=Z,,options=rbind:rw`, the `type=X` should be specified a proprietary type specifically designed for some kind of volume. 

Now, supported types: 

- `directvol` for direct volume
- `vfiovol` for VFIO device based volume
- `spdkvol` for SPDK/vhost-user based volume


## Setup Device and Run a Kata-Containers

### Direct Block Device Based Volume

#### create raw block based backend storage

> **Tips:** raw block based backend storage MUST be formatted with `mkfs`.

```bash
$ sudo dd if=/dev/zero of=/tmp/stor/rawdisk01.20g bs=1M count=20480
$ sudo mkfs.ext4 /tmp/stor/rawdisk01.20g
```

#### setup direct block device for kata-containers

```json
{
  "device": "/tmp/stor/rawdisk01.20g", 
  "volume_type": "directvol", 
  "fs_type": "ext4", 
  "metadata":"{}", 
  "options": []
}
```

```bash
$ sudo kata-ctl direct-volume add /kubelet/kata-direct-vol-002/directvol002 "{\"device\": \"/tmp/stor/rawdisk01.20g\", \"volume_type\": \"directvol\", \"fs_type\": \"ext4\", \"metadata\":"{}", \"options\": []}"
$# /kubelet/kata-direct-vol-002/directvol002 <==> /run/kata-containers/shared/direct-volumes/W1lMa2F0ZXQva2F0YS10a2F0DAxvbC0wMDEvdm9sdW1lMDAx
$ cat W1lMa2F0ZXQva2F0YS10a2F0DAxvbC0wMDEvdm9sdW1lMDAx/mountInfo.json 
{"volume_type":"directvol","device":"/tmp/stor/rawdisk01.20g","fs_type":"ext4","metadata":{},"options":[]}
```

#### Run a Kata container with direct block device volume

```bash
$ # type=disrectvol,src=/kubelet/kata-direct-vol-002/directvol002,dst=/disk002,options=rbind:rw
$ sudo ctr run -t --rm --runtime io.containerd.kata.v2 --mount type=directvol,src=/kubelet/kata-direct-vol-002/directvol002,dst=/disk002,options=rbind:rw "$image" kata-direct-vol-xx05302045 /bin/bash
```


### VFIO Device Based Block Volume

#### create VFIO device based backend storage

> **Tip:** It only supports `vfio-pci` based PCI device passthrough mode.

In this scenario, the device's host kernel driver will be replaced by `vfio-pci`, and IOMMU group ID generated.
And either device's BDF or its VFIO IOMMU group ID in `/dev/vfio/` is fine for "device" in `mountinfo.json`.

```bash
$ lspci -nn -k -s 45:00.1 
45:00.1 SCSI storage controller 
...
Kernel driver in use: vfio-pci
...
$ ls /dev/vfio/110
/dev/vfio/110
$ ls /sys/kernel/iommu_groups/110/devices/
0000:45:00.1
```

#### setup VFIO device for kata-containers

First, configure the `mountinfo.json`, as below:

- (1) device with `BB:DD:F`

```json
{
  "device": "45:00.1",
  "volume_type": "vfiovol", 
  "fs_type": "ext4", 
  "metadata":"{}", 
  "options": []
}
```

- (2) device with `DDDD:BB:DD:F`

```json
{
  "device": "0000:45:00.1",
  "volume_type": "vfiovol", 
  "fs_type": "ext4", 
  "metadata":"{}", 
  "options": []
}
```

- (3) device with `/dev/vfio/X`

```json
{
  "device": "/dev/vfio/110", 
  "volume_type": "vfiovol", 
  "fs_type": "ext4", 
  "metadata":"{}", 
  "options": []
}
```

Second, run kata-containers with device(`/dev/vfio/110`) as an example:

```bash
$ sudo kata-ctl direct-volume add /kubelet/kata-vfio-vol-003/vfiovol003 "{\"device\": \"/dev/vfio/110\", \"volume_type\": \"vfiovol\", \"fs_type\": \"ext4\", \"metadata\":"{}", \"options\": []}"
$ # /kubelet/kata-vfio-vol-003/directvol003 <==> /run/kata-containers/shared/direct-volumes/F0va22F0ZvaS12F0YS10a2F0DAxvbC0F0ZXvdm9sdF0Z0YSx
$ cat F0va22F0ZvaS12F0YS10a2F0DAxvbC0F0ZXvdm9sdF0Z0YSx/mountInfo.json 
{"volume_type":"vfiovol","device":"/dev/vfio/110","fs_type":"ext4","metadata":{},"options":[]}
```

#### Run a Kata container with VFIO block device based volume

```bash
$ # type=disrectvol,src=/kubelet/kata-vfio-vol-003/vfiovol003,dst=/disk003,options=rbind:rw
$ sudo ctr run -t --rm --runtime io.containerd.kata.v2 --mount type=vfiovol,src=/kubelet/kata-vfio-vol-003/vfiovol003,dst=/disk003,options=rbind:rw "$image" kata-vfio-vol-xx05302245 /bin/bash
```


### SPDK Device Based Block Volume

SPDK vhost-user devices in runtime-rs, unlike runtime (golang version), there is no need to `mknod` device node under `/dev/` any more.
Just using the `kata-ctl direct-volume add ..` to make a mount info config is enough.

#### Run SPDK vhost target and Expose vhost block device

Run a SPDK vhost target and get vhost-user block controller as an example:

First, run SPDK vhost target:

> **Tips:** If driver `vfio-pci` supported, you can run SPDK with `DRIVER_OVERRIDE=vfio-pci`
> Otherwise, Just run without it `sudo HUGEMEM=4096 ./scripts/setup.sh`.

```bash
$ SPDK_DEVEL=/xx/spdk
$ VHU_UDS_PATH=/tmp/vhu-targets
$ RAW_DISKS=/xx/rawdisks
$ # Reset first
$ ${SPDK_DEVEL}/scripts/setup.sh reset
$ sudo sysctl -w vm.nr_hugepages=2048
$ #4G Huge Memory for spdk
$ sudo HUGEMEM=4096 DRIVER_OVERRIDE=vfio-pci ${SPDK_DEVEL}/scripts/setup.sh
$ sudo ${SPDK_DEVEL}/build/bin/spdk_tgt -S $VHU_UDS_PATH -s 1024 -m 0x3 &
```

Second, create a vhost controller:

```bash
$ sudo dd if=/dev/zero of=${RAW_DISKS}/rawdisk01.20g bs=1M count=20480
$ sudo ${SPDK_DEVEL}/scripts/rpc.py bdev_aio_create ${RAW_DISKS}/rawdisk01.20g vhu-rawdisk01.20g 512
$ sudo ${SPDK_DEVEL}/scripts/rpc.py vhost_create_blk_controller vhost-blk-rawdisk01.sock vhu-rawdisk01.20g
```

Here, a vhost controller `vhost-blk-rawdisk01.sock` is created, and the controller will
be passed to Hypervisor, such as Dragonball, Cloud-Hypervisor, Firecracker or QEMU.


#### setup vhost-user block device for kata-containers


First, `mkdir` a sub-path `kubelet/kata-test-vol-001/` under `/run/kata-containers/shared/direct-volumes/`.

Second, fill fields in `mountinfo.json`, it looks like as below:
```json
{
  "device": "/tmp/vhu-targets/vhost-blk-rawdisk01.sock",
  "volume_type": "spdkvol",
  "fs_type": "ext4",
  "metadata":"{}",
  "options": []
}
```

Third, with the help of `kata-ctl direct-volume` to add block device to generate `mountinfo.json`, and run a kata container with `--mount`.

```bash
$ # kata-ctl direct-volume add
$ sudo kata-ctl direct-volume add /kubelet/kata-test-vol-001/volume001 "{\"device\": \"/tmp/vhu-targets/vhost-blk-rawdisk01.sock\", \"volume_type\":\"spdkvol\", \"fs_type\": \"ext4\", \"metadata\":"{}", \"options\": []}"
$ # /kubelet/kata-test-vol-001/volume001 <==> /run/kata-containers/shared/direct-volumes/L2t1YmVsZXQva2F0YS10ZXN0LXZvbC0wMDEvdm9sdW1lMDAx
$ cat L2t1YmVsZXQva2F0YS10ZXN0LXZvbC0wMDEvdm9sdW1lMDAx/mountInfo.json
$ {"volume_type":"spdkvol","device":"/tmp/vhu-targets/vhost-blk-rawdisk01.sock","fs_type":"ext4","metadata":{},"options":[]}
```

As `/run/kata-containers/shared/direct-volumes/` is a fixed path , we will be able to run a kata pod with `--mount` and set
`src` sub-path. And the `--mount` argument looks like: `--mount type=spdkvol,src=/kubelet/kata-test-vol-001/volume001,dst=/disk001`.


#### Run a Kata container with SPDK vhost-user block device


In the case, `ctr run --mount type=X, src=source, dst=dest`, the X will be set `spdkvol` which is a proprietary type specifically designed for SPDK volumes.

```bash
$ # ctr run with --mount type=spdkvol,src=/kubelet/kata-test-vol-001/volume001,dst=/disk001
$ sudo ctr run -t --rm --runtime io.containerd.kata.v2 --mount type=spdkvol,src=/kubelet/kata-test-vol-001/volume001,dst=/disk001,options=rbind:rw "$image" kata-spdk-vol-xx0530 /bin/bash
```

## Integrate Direct Volume with K8S

Details see [`csi-kata-directvolume`](../../src/tools/csi-kata-directvolume/README.md)
