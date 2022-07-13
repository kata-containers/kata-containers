# How to build and use experimental QEMU

This document describes how to build an experimental QEMU to use features that the
main branch of QEMU does not support yet, for example, DAX with `Nydus`.

This how-to will show how to build an experimental QEMU and use it with DAX.

## Pre-requisites

You must have cloned the Kata Containers repo first.

## Build experimental QEMU

```bash
$ cd $GOPATH/github.com/kata-containers/kata-containers/tools/packaging/static-build/qemu
$ make build-experimental
```

If the build is finished successfully, you will get the build artifact like this:

```bash
$ ls -tl
total 16760
-rw-r--r-- 1 vagrant vagrant 17139024 Jul 13 06:40 kata-static-qemu-experimental.tar.gz
```

## Use the experimental QEMU

You need to use the experimental QEMU to overwrite the original QEMU that your previous
Kata Containers installation installed.

This how-to assumes that you have installed Kata Containers that downloaded from
Kata Containers [releases page](https://github.com/kata-containers/kata-containers/releases)
and extract it to `/opt` directory.


```bash
# extract to current directory
$ tar zxvf kata-static-qemu-experimental.tar.gz
$ sudo cp opt/kata/bin/qemu-system-x86_64-experimental /opt/kata/bin/qemu-system-x86_64
$ sudo cp -r opt/kata/share/kata-qemu-experimental/qemu /opt/kata/share/kata-qemu/qemu
```

## Enable DAX for containers

Edit your `configuration.toml` and ensure these configuration items:

```
# use nydus for better DAX support
shared_fs = "virtio-fs-nydus"
# cache mod should be always or auto to use DAX
virtio_fs_cache = "auto"
virtio_fs_cache_size = 256
```

After updating the `configuration.toml`, you can create new containers with DAX enabled.

**Note:**
- DAX is disabled if the `virtio_fs_cache_size` is set to 0.
