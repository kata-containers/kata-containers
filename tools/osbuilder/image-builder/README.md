# Kata Containers image generation

A Kata Containers disk image is generated using the `image_builder.sh` script.
This uses a rootfs directory created by the `rootfs-builder/rootfs.sh` script.

## Creating a guest OS image

To create a guest OS image run:

```
$ sudo ./image_builder.sh path/to/rootfs
```

Where `path/to/rootfs` is the directory populated by `rootfs.sh`.

The guest disk images built by `kata-deploy` use compressed EROFS. When
invoking `image_builder.sh` directly, select EROFS with `-f erofs` or
`FS_TYPE=erofs`.

For systemd guests, the builder supplies an empty `/etc/machine-id` for a
transient machine ID and mounts a temporary filesystem on `/var/lib/systemd`.
This writable state is discarded when the VM stops; the guest root filesystem
remains read-only.

When using a custom EROFS image, set the matching hypervisor's `rootfs_type`
to `"erofs"`. For QEMU, also set `disable_image_nvdimm = true`. With
`runtime-rs`, set `vm_rootfs_driver = "virtio-blk-pci"` (`virtio-blk-ccw` on
`s390x`). The generated configurations already select these settings.

The guest root filesystem format is independent of the container snapshotter.

> **Note**: If you are building an image from an Alpine rootfs, see
> the important note [here](/tools/osbuilder/rootfs-builder/README.md#rootfs-requirements).

## Further information

For more information about usage (including how to adjust the size of the
image), run:

```
$ ./image_builder.sh -h
```
