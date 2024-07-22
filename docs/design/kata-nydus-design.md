# Background

[Research](https://www.usenix.org/conference/fast16/technical-sessions/presentation/harter) shows that time to take for pull operation accounts for 76% of container startup time but only 6.4% of that data is read. So if we can get data on demand (lazy load), it will speed up the container start. [`Nydus`](https://github.com/dragonflyoss/image-service) is a project which build image with new format and can get data on demand when container start.

The following benchmarking result shows the performance improvement compared with the OCI image for the container cold startup elapsed time on containerd. As the OCI image size increases, the container startup time of using `nydus` image remains very short. [Click here](https://github.com/dragonflyoss/image-service/blob/master/docs/nydus-design.md) to see `nydus` design.

![`nydus`-performance](arch-images/nydus-performance.png)

## Proposal - Bring `lazyload` ability to Kata Containers

`Nydusd` is a fuse/`virtiofs` daemon which is provided by `nydus` project and it supports `PassthroughFS` and [RAFS](https://github.com/dragonflyoss/image-service/blob/master/docs/nydus-design.md) (Registry Acceleration File System) natively, so in Kata Containers, we can use `nydusd` in place of `virtiofsd` and mount `nydus` image to guest in the meanwhile.

The process of creating/starting Kata Containers with `virtiofsd`,

1. When creating sandbox, the Kata Containers Containerd v2 [shim](https://github.com/kata-containers/kata-containers/blob/main/docs/design/architecture/README.md#runtime) will launch `virtiofsd` before VM starts and share directories with VM.
2. When creating container, the Kata Containers Containerd v2 shim will mount rootfs to `kataShared`(/run/kata-containers/shared/sandboxes/\<SANDBOX\>/mounts/\<CONTAINER\>/rootfs), so it can be seen at the path `/run/kata-containers/shared/containers/shared/\<CONTAINER\>/rootfs` in the guest and used as container's rootfs.

The process of creating/starting Kata Containers with `nydusd`,

![kata-`nydus`](arch-images/kata-nydus.png)

1. When creating sandbox, the Kata Containers Containerd v2 shim will launch `nydusd` daemon before VM starts.
After VM starts, `kata-agent` will mount `virtiofs` at the path `/run/kata-containers/shared` and Kata Containers Containerd v2 shim mount `passthroughfs` filesystem to path `/run/kata-containers/shared/containers` when the VM starts.

```bash
# start nydusd
$ sandbox_id=my-test-sandbox
$ sudo /usr/local/bin/nydusd --log-level info --sock /run/vc/vm/${sandbox_id}/vhost-user-fs.sock --apisock /run/vc/vm/${sandbox_id}/api.sock
```

```bash
# source: the host sharedir which will pass through to guest
$ sudo curl -v --unix-socket /run/vc/vm/${sandbox_id}/api.sock \
    -X POST "http://localhost/api/v1/mount?mountpoint=/containers" -H "accept: */*" \
    -H "Content-Type: application/json" \
    -d '{
            "source":"/path/to/sharedir",
            "fs_type":"passthrough_fs",
            "config":""
    }'
```

2. When creating normal container, the Kata Containers Containerd v2 shim send request to `nydusd` to mount `rafs` at the path `/run/kata-containers/shared/rafs/<container_id>/lowerdir` in guest.

```bash
# source: the metafile of nydus image
# config: the config of this image
$ sudo curl --unix-socket /run/vc/vm/${sandbox_id}/api.sock \
    -X POST "http://localhost/api/v1/mount?mountpoint=/rafs/<container_id>/lowerdir" -H "accept: */*" \
    -H "Content-Type: application/json" \
    -d '{
            "source":"/path/to/bootstrap",
            "fs_type":"rafs",
            "config":"config":"{\"device\":{\"backend\":{\"type\":\"localfs\",\"config\":{\"dir\":\"blobs\"}},\"cache\":{\"type\":\"blobcache\",\"config\":{\"work_dir\":\"cache\"}}},\"mode\":\"direct\",\"digest_validate\":true}",
    }'
```

The Kata Containers Containerd v2 shim will also bind mount `snapshotdir` which `nydus-snapshotter` assigns to `sharedir`。
So in guest, container rootfs=overlay(`lowerdir=rafs`, `upperdir=snapshotdir/fs`, `workdir=snapshotdir/work`)

> how to transfer the `rafs` info from `nydus-snapshotter` to the Kata Containers Containerd v2 shim?

By default, when creating `OCI` image container, `nydus-snapshotter` will return [`struct` Mount slice](https://github.com/containerd/containerd/blob/main/core/mount/mount.go#L30) below to containerd and containerd use them to mount rootfs

```
[
    {
        Type: "overlay",
        Source: "overlay",
        Options: [lowerdir=/var/lib/containerd/io.containerd.snapshotter.v1.nydus/snapshots/<snapshot_A>/mnt,upperdir=/var/lib/containerd/io.containerd.snapshotter.v1.nydus/snapshots/<snapshot_B>/fs,workdir=/var/lib/containerd/io.containerd.snapshotter.v1.nydus/snapshots/<snapshot_B>/work],
    }
]
```

Then, we can append `rafs` info into `Options`, but if do this, containerd will mount failed, as containerd can not identify `rafs` info. Here, we can refer to [containerd mount helper](https://github.com/containerd/containerd/blob/main/core/mount/mount_linux.go#L81) and provide a binary called `nydus-overlayfs`. The `Mount` slice which `nydus-snapshotter` returned becomes

```
[
    {
        Type: "fuse.nydus-overlayfs",
        Source: "overlay",
        Options: [lowerdir=/var/lib/containerd/io.containerd.snapshotter.v1.nydus/snapshots/<snapshot_A>/mnt,upperdir=/var/lib/containerd/io.containerd.snapshotter.v1.nydus/snapshots/<snapshot_B>/fs,workdir=/var/lib/containerd/io.containerd.snapshotter.v1.nydus/snapshots/<snapshot_B>/work,extraoption=base64({source:xxx,config:xxx,snapshotdir:xxx})],
    }
]
```

When containerd find `Type` is `fuse.nydus-overlayfs`,

1. containerd will call `mount.fuse` command;
2. in `mount.fuse`, it will call `nydus-overlayfs`.
3. in `nydus-overlayfs`, it will ignore the `extraoption` and do the overlay mount.

Finally, in the Kata Containers Containerd v2 shim, it parse `extraoption` and get the `rafs` info to mount the image in guest.
