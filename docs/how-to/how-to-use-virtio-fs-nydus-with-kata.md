# Kata Containers with virtio-fs-nydus

## Introduction

Research shows that time to take for pull operation accounts for 76% of container startup time but only 6.4% of that data is read. so if we can get data on demand(lazyload), it will speed up the container start. [Nydusd](https://github.com/dragonflyoss/image-service) is the project which build image with new format and can get data on demand when container start.

The feature is not completely finished, if you want to try, you can do as follows,

1. Modify nydus-snapshotter code as [commit](https://github.com/luodw/image-service/commit/e3499aefa3a1b5aa073d332ad3553335a93765ee);

2. Modify containerd code as [commit](https://github.com/luodw/containerd/commit/5ce208e2d79da8c14330e24c9b1253fa07b81605);

3. Depoly nydus run environment as [Nydus Setup for Containerd Environment](https://github.com/dragonflyoss/image-service/blob/master/docs/containerd-env-setup.md);

4. Use kata-containers `support_nydus` branch to compile and build kata-containers.img;

5. Update shared_fs to `virtio-fs-nydus` [here](https://github.com/luodw/kata-containers/blob/support_nydus/src/runtime/cli/config/configuration-qemu.toml.in#L134);

6. As kata-shim,nydusd,qemu are in isolated net namespace, nydusd can not connect to hub in my local env, so I run nydus image in runc first to cache the blob, and next you can run kata containers with nydusd format image as usual. `crictl run -r kata-qemu container-config.yaml pod-config.yaml`
