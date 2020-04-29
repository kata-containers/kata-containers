* [Creating a guest OS initrd image](#creating-a-guest-os-initrd-image)
* [Further information](#further-information)

# Kata Containers initrd image generation

A Kata Containers initrd image is generated using the `initrd_builder.sh` script.
This script uses a rootfs directory created by the `rootfs-builder/rootfs.sh` script.

## Creating a guest OS initrd image

To create a guest OS initrd image run:

```
$ sudo ./initrd_builder.sh path/to/rootfs
```

The `rootfs.sh` script populates the `path/to/rootfs` directory.

## Further information

For more information on how to use the `initrd_builder.sh` script, run:

```
$ ./initrd_builder.sh -h
```
