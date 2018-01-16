* [Creating a guest OS image](#creating-a-guest-os-image)
* [Further information](#further-information)

# Kata Containers image generation

A Kata Containers disk image is generated using the `image_builder.sh` script.
This uses a rootfs directory created by the `rootfs-builder/rootfs.sh` script.

## Creating a guest OS image

To create a guest OS image run:

```
$ ./image_builder.sh path/to/rootfs
```

Where `path/to/rootfs` is the directory populated by `rootfs.sh`.

## Further information

For more information about usage (including how to adjust the size of the
image), run:

```
$ ./image_builder.sh -h
```
