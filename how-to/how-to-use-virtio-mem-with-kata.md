# Kata Containers with `virtio-mem`

- [Introduction](#introduction)
- [Requisites](#requisites)
- [Run a Kata Container utilizing `virtio-mem`](#run-a-kata-container-utilizing-virtio-mem)

## Introduction

The basic idea of `virtio-mem` is to provide a flexible, cross-architecture memory hot plug and hot unplug solution that avoids many limitations imposed by existing technologies, architectures, and interfaces.
More details can be found in https://lkml.org/lkml/2019/12/12/681.

Kata Containers with `virtio-mem` supports memory resize.

## Requisites

Kata Containers with `virtio-mem` requires Linux and the QEMU that support `virtio-mem`.
The Linux kernel and QEMU upstream version still not support `virtio-mem`.  @davidhildenbrand is working on them.
Please use following unofficial version of the Linux kernel and QEMU that support `virtio-mem` with Kata Containers.

The Linux kernel is at https://github.com/davidhildenbrand/linux/tree/virtio-mem-rfc-v4.
The Linux kernel config that can work with Kata Containers is at https://gist.github.com/teawater/016194ee84748c768745a163d08b0fb9.

The QEMU is at https://github.com/teawater/qemu/tree/kata-virtio-mem. (The original source is at https://github.com/davidhildenbrand/qemu/tree/virtio-mem.  Its base version of QEMU cannot work with Kata Containers.  So merge the commit of `virtio-mem` to upstream QEMU.)

Set Linux and the QEMU that support `virtio-mem` with following line in the Kata Containers QEMU configuration `configuration-qemu.toml`:
```toml
[hypervisor.qemu]
path = "qemu-dir"
kernel = "vmlinux-dir"
```

Enable `virtio-mem` with following line in the Kata Containers configuration:
```toml
enable_virtio_mem = true
```

## Run a Kata Container utilizing `virtio-mem`

Use following command to enable memory overcommitment of a Linux kernel.  Because QEMU `virtio-mem` device need to allocate a lot of memory.
```
$ echo 1 | sudo tee /proc/sys/vm/overcommit_memory
```

Use following command start a Kata Container.
```
$ docker run --rm -it --runtime=kata --name test busybox
```

Use following command set the memory size of test to default_memory + 512m.
```
$ docker update -m 512m --memory-swap -1 test
```

