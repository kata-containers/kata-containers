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

Finally, when running a Kata Containers witch `ctr run --mount type=X, src=Y, dst=Z,,options=rbind:rw`, the `type=X` should be specified a proprietary type specifically designed for some kind of volume. 

Now, supported types: 

- `directvol` for direct volume
- `spdkvol` for SPDK volume (TBD)
- `vfiovol` for VFIO device based volume (TBD)


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
$ sudo ./kata-ctl direct-volume add /kubelet/kata-direct-vol-002/directvol002 "{\"device\": \"/tmp/stor/rawdisk01.20g\", \"volume_type\": \"directvol\", \"fs_type\": \"ext4\", \"metadata\":"{}", \"options\": []}"
$# /kubelet/kata-direct-vol-002/directvol002 <==> /run/kata-containers/shared/direct-volumes/W1lMa2F0ZXQva2F0YS10a2F0DAxvbC0wMDEvdm9sdW1lMDAx
$ cat W1lMa2F0ZXQva2F0YS10a2F0DAxvbC0wMDEvdm9sdW1lMDAx/mountInfo.json 
{"volume_type":"directvol","device":"/tmp/stor/rawdisk01.20g","fs_type":"ext4","metadata":{},"options":[]}
```

#### Run a Kata container with direct block device volume

```bash
$ # type=disrectvol,src=/kubelet/kata-direct-vol-002/directvol002,dst=/disk002,options=rbind:rw
$sudo ctr run -t --rm --runtime io.containerd.kata.v2 --mount type=directvol,src=/kubelet/kata-direct-vol-002/directvol002,dst=/disk002,options=rbind:rw "$image" kata-direct-vol-xx05302045 /bin/bash
```


### SPDK Device Based Volume

TBD

### VFIO Device Based Volume

TBD