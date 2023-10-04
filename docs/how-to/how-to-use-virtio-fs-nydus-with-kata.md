# Kata Containers with virtio-fs-nydus

## Introduction

Refer to [kata-`nydus`-design](../design/kata-nydus-design.md) for introduction and `nydus` has supported Kata Containers with hypervisor `QEMU` and `CLH` currently.

## How to

You can use Kata Containers with `nydus` as follows,

1. Use [`nydus` latest branch](https://github.com/dragonflyoss/image-service);

2. Deploy `nydus` environment as [`Nydus` Setup for Containerd Environment](https://github.com/dragonflyoss/image-service/blob/master/docs/containerd-env-setup.md);

3. Start `nydus-snapshotter` with `enable_nydus_overlayfs` enabled;

4. Use [kata-containers](https://github.com/kata-containers/kata-containers) `latest` branch to compile and build `kata-containers.img`;

5. Update `configuration-qemu.toml` or `configuration-clh.toml`to include:

```toml
shared_fs = "virtio-fs-nydus"
virtio_fs_daemon = "<nydusd binary path>"
virtio_fs_extra_args = []
```

6. run `crictl run -r kata nydus-container.yaml nydus-sandbox.yaml`;

The `nydus-sandbox.yaml` looks like below:

```yaml
metadata:
  attempt: 1
  name: nydus-sandbox
  uid: nydus-uid
  namespace: default
log_directory: /tmp
linux:
  security_context:
    namespace_options:
      network: 2
annotations:
  "io.containerd.osfeature": "nydus.remoteimage.v1"
```

The `nydus-container.yaml` looks like below:

```yaml
metadata:
  name: nydus-container
image:
  image: localhost:5000/ubuntu-nydus:latest
command:
  - /bin/sleep
args:
  - 600
log_path: container.1.log
```
