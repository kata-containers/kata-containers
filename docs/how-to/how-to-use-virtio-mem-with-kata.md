# Kata Containers with `virtio-mem`

- [Introduction](#introduction)
- [Requisites](#requisites)
- [Run a Kata Container utilizing `virtio-mem`](#run-a-kata-container-utilizing-virtio-mem)

## Introduction

The basic idea of `virtio-mem` is to provide a flexible, cross-architecture memory hot plug and hot unplug solution that avoids many limitations imposed by existing technologies, architectures, and interfaces.
More details can be found in https://lkml.org/lkml/2019/12/12/681.

Kata Containers with `virtio-mem` supports memory resize.

## Requisites

Kata Containers just supports `virtio-mem` with QEMU.
Install and setup Kata Containers as shown [here](../install/README.md).

### With x86_64
The `virtio-mem` config of the x86_64 Kata Linux kernel is open.
Enable `virtio-mem` as follows:
```
$ sudo sed -i -e 's/^#enable_virtio_mem.*$/enable_virtio_mem = true/g' /etc/kata-containers/configuration.toml
```

### With other architectures
The `virtio-mem` config of the others Kata Linux kernel is not open.
You can open `virtio-mem` config as follows:
```
CONFIG_VIRTIO_MEM=y
```
Then you can build and install the guest kernel image as shown [here](../../tools/packaging/kernel/README.md#build-kata-containers-kernel).

## Run a Kata Container utilizing `virtio-mem`

Use following command to enable memory overcommitment of a Linux kernel.  Because QEMU `virtio-mem` device need to allocate a lot of memory.
```
$ echo 1 | sudo tee /proc/sys/vm/overcommit_memory
```

Use following command to start a Kata Container.
```
$ pod_yaml=pod.yaml
$ container_yaml=${REPORT_DIR}/container.yaml
$ image="quay.io/prometheus/busybox:latest"
$ cat << EOF > "${pod_yaml}"
metadata:
  name: busybox-sandbox1
EOF
$ cat << EOF > "${container_yaml}"
metadata:
  name: busybox-killed-vmm
image:
  image: "$image"
command:
- top
EOF
$ sudo crictl pull $image
$ podid=$(sudo crictl runp $pod_yaml)
$ cid=$(sudo crictl create $podid $container_yaml $pod_yaml)
$ sudo crictl start $cid
```

Use the following command to set the container memory limit to 2g and the memory size of the VM to its default_memory + 2g.
```
$ sudo crictl update --memory $((2*1024*1024*1024)) $cid
```

Use the following command to set the container memory limit to 1g and the memory size of the VM to its default_memory + 1g.
```
$ sudo crictl update --memory $((1*1024*1024*1024)) $cid
```
