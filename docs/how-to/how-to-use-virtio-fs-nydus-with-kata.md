# Kata Containers with virtio-fs-nydus

## Introduction

Research shows that time to take for pull operation accounts for 76% of container startup time but only 6.4% of that data is read. so if we can get data on demand(lazyload), it will speed up the container start. [Nydusd](https://github.com/dragonflyoss/image-service) is the project which build image with new format and can get data on demand when container start.

The feature is not completely finished, if you want to try, you can do as follows,

1. Use nydus-snapshotter branch [https://github.com/dragonflyoss/image-service/pull/184](https://github.com/dragonflyoss/image-service/pull/184)

2. Depoly nydus run environment as [Nydus Setup for Containerd Environment](https://github.com/dragonflyoss/image-service/blob/master/docs/containerd-env-setup.md);

3. Use kata-containers `support_nydus` branch to compile and build kata-containers.img;

4. Update shared_fs to `virtio-fs-nydus` [here](https://github.com/luodw/kata-containers/blob/support_nydus/src/runtime/cli/config/configuration-qemu.toml.in#L134);

5. As kata-shim,nydusd,qemu are in isolated net namespace, nydusd can not connect to hub in my local env, so I run nydus image in runc first to cache the blob, and next you can run kata containers with nydusd format image as usual. `crictl run -r kata-qemu container-config.yaml pod-config.yaml`
