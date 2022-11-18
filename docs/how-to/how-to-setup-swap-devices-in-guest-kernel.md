# Setup swap device in guest kernel

## Introduction

Setup swap device in guest kernel can help to increase memory capacity, handle some memory issues and increase file access speed sometimes.
Kata Containers can insert a raw file to the guest as the swap device.

## Requisites

The swap config of the containers should be set by [annotations](how-to-set-sandbox-config-kata.md#container-options).  So [extra configuration is needed for containerd](how-to-set-sandbox-config-kata.md#containerd-configuration).

Kata Containers just supports setup swap device in guest kernel with QEMU.
Install and setup Kata Containers as shown [here](../install/README.md).

Enable setup swap device in guest kernel as follows:
```
$ sudo sed -i -e 's/^#enable_guest_swap.*$/enable_guest_swap = true/g' /etc/kata-containers/configuration.toml
```

## Run a Kata Containers utilizing swap device

Use following command to start a Kata Containers with swappiness 60 and 1GB swap device (swap_in_bytes - memory_limit_in_bytes).
```
$ pod_yaml=pod.yaml
$ container_yaml=container.yaml
$ image="quay.io/prometheus/busybox:latest"
$ cat << EOF > "${pod_yaml}"
metadata:
  name: busybox-sandbox1
  uid: $(uuidgen)
  namespace: default
EOF
$ cat << EOF > "${container_yaml}"
metadata:
  name: busybox-test-swap
annotations:
  io.katacontainers.container.resource.swappiness: "60"
  io.katacontainers.container.resource.swap_in_bytes: "2147483648"
linux:
  resources:
    memory_limit_in_bytes: 1073741824
image:
  image: "$image"
command:
- top
EOF
$ sudo crictl pull $image
$ podid=$(sudo crictl runp --runtime kata $pod_yaml)
$ cid=$(sudo crictl create $podid $container_yaml $pod_yaml)
$ sudo crictl start $cid
```

Kata Containers setups swap device for this container only when `io.katacontainers.container.resource.swappiness` is set.

The following table shows the swap size how to decide if `io.katacontainers.container.resource.swappiness` is set.
|`io.katacontainers.container.resource.swap_in_bytes`|`memory_limit_in_bytes`|swap size|
|---|---|---|
|set|set| `io.katacontainers.container.resource.swap_in_bytes` - `memory_limit_in_bytes`|
|not set|set| `memory_limit_in_bytes`|
|not set|not set| `io.katacontainers.config.hypervisor.default_memory`|
|set|not set|cgroup doesn't support this usage|
