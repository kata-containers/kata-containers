# How to run Docker in Docker with Kata Containers

This document describes the why and how behind running Docker in a Kata Container.

> **Note:** While in other environments this might be described as "Docker in Docker", the new architecture of Kata 2.x means [Docker can no longer be used to create containers using a Kata Containers runtime](https://github.com/kata-containers/kata-containers/issues/722).

## Requirements

- A working Kata Containers installation

## Install and configure Kata Containers

Follow the [Kata Containers installation guide](../install/README.md) to Install Kata Containers on your Kubernetes cluster.

## Background

Docker in Docker ("DinD") is the colloquial name for the ability to run `docker` from inside a container.

You can learn more about about Docker-in-Docker at the following links:

- [The original announcement of DinD](https://www.docker.com/blog/docker-can-now-run-within-docker/)
- [`docker` image Docker Hub page](https://hub.docker.com/_/docker/) (this page lists the `-dind` releases)

While normally DinD refers to running `docker` from inside a Docker container,
Kata Containers 2.x allows only [supported runtimes][kata-2.x-supported-runtimes] (such as [`containerd`](../install/container-manager/containerd/containerd-install.md)).

Running `docker` in a Kata Container implies creating Docker containers from inside a container managed by `containerd` (or another supported container manager), as illustrated below:

```
container manager -> Kata Containers shim     -> Docker Daemon -> Docker container
(containerd)        (containerd-shim-kata-v2)    (dockerd)        (busybox sh)
```

[OverlayFS][OverlayFS] is the preferred storage driver for most container runtimes on Linux ([including Docker](https://docs.docker.com/storage/storagedriver/select-storage-driver)).

> **Note:** While in the past Kata Containers did not contain the [`overlay` kernel module (aka OverlayFS)][OverlayFS], the kernel modules have been included since the [Kata Containers v2.0.0 release][v2.0.0].

[OverlayFS]: https://www.kernel.org/doc/html/latest/filesystems/overlayfs.html
[v2.0.0]: https://github.com/kata-containers/kata-containers/releases/tag/2.0.0
[kata-2.x-supported-runtimes]: ../install/container-manager/containerd/containerd-install.md

## Why Docker in Kata Containers 2.x requires special measures

Running Docker containers Kata Containers requires care because `VOLUME`s specified in `Dockerfile`s run by Kata Containers are given the `kataShared` mount type by default, which applies to the root directory `/`:

```console
/ # mount
kataShared on / type virtiofs (rw,relatime,dax)
```

`kataShared` mount types are powered by [`virtio-fs`](https://virtio-fs.gitlab.io/), a marked improvement over `virtio-9p`, thanks to [PR #1016](https://github.com/kata-containers/runtime/pull/1016). While `virtio-fs` is normally an excellent choice, in the case of DinD workloads `virtio-fs` causes an issue -- [it *cannot* be used as a "upper layer" of `overlayfs` without a custom patch](http://lists.katacontainers.io/pipermail/kata-dev/2020-January/001216.html).

As `/var/lib/docker` is a `VOLUME` specified by DinD (i.e. the `docker` images tagged `*-dind`/`*-dind-rootless`), `docker` will fail to start (or even worse, silently pick a worse storage driver like `vfs`) when started in a Kata Container. Special measures must be taken when running DinD-powered workloads in Kata Containers.

## Workarounds/Solutions

Thanks to various community contributions (see [issue references below](#references)) the following options, with various trade-offs have been uncovered:

### Use a memory backed volume

For small workloads (small container images, without much generated filesystem load), a memory-backed volume is sufficient. Kubernetes supports a variant of  [the `EmptyDir` volume](https://kubernetes.io/docs/concepts/storage/volumes/#emptydir), which allows for memdisk-backed storage -- the the `medium: Memory`. An example of a `Pod` using such a setup [was contributed](https://github.com/kata-containers/runtime/issues/1429#issuecomment-477385283), and is reproduced below:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: dind
spec:
  runtimeClassName: kata
  containers:
  - name: dind
    securityContext:
      privileged: true
    image: docker:20.10-dind
    args: ["--storage-driver=overlay2"]
    resources:
      limits:
        memory: "3G"
    volumeMounts:
      - mountPath: /var/run/
        name: dockersock
      - mountPath: /var/lib/docker
        name: docker
  volumes:
    - name: dockersock
      emptyDir: {}
    - name: docker
      emptyDir:
        medium: Memory
```

Inside the container you can view the mount:

```console
/ # mount | grep lib\/docker
tmpfs on /var/lib/docker type tmpfs (rw,relatime)
```

As is mentioned in the comment encapsulating this code, using volatile memory for container storage backing is a risky and could be possibly wasteful on machines that do not have a lot of RAM.

### Use a loop mounted disk

Using a loop mounted disk that is provisioned shortly before starting of the container workload is another approach that yields good performance.

Contributors provided [an example in issue #1888](https://github.com/kata-containers/runtime/issues/1888#issuecomment-739057384), which is reproduced in part below:

```yaml
spec:
  containers:
    - name: docker
      image: docker:20.10-dind
      command: ["sh", "-c"]
      args:
      - if [[ $(df -PT /var/lib/docker | awk 'NR==2 {print $2}') == virtiofs ]]; then
          apk add e2fsprogs &&
          truncate -s 20G /tmp/disk.img &&
          mkfs.ext4 /tmp/disk.img &&
          mount /tmp/disk.img /var/lib/docker; fi &&
        dockerd-entrypoint.sh;
      securityContext:
        privileged: true
```

Note that loop mounted disks are often sparse, which means they *do not* take up the full amount of space that has been provisioned. This solution seems to produce the best performance and flexibility, at the expense of increased complexity and additional required setup.

### Build a custom kernel

It's possible to [modify the kernel](https://github.com/kata-containers/runtime/issues/1888#issuecomment-616872558) (in addition to applying the earlier mentioned mailing list patch) to support using `virtio-fs` as an upper. Note that if you modify your kernel and use `virtio-fs` you may require [additional changes](https://github.com/kata-containers/runtime/issues/1888#issuecomment-739057384) for decent performance and to address other issues.

> **NOTE:** A future kernel release may rectify the usability and performance issues of using `virtio-fs` as an OverlayFS upper layer.

## References

The solutions proposed in this document are an amalgamation of thoughtful contributions from the Kata Containers community.

Find links to issues & related discussion and the fruits therein below:

- [How to run Docker in Docker with Kata Containers (#2474)](https://github.com/kata-containers/kata-containers/issues/2474)
- [Does Kata-container support AUFS/OverlayFS? (#2493)](https://github.com/kata-containers/runtime/issues/2493)
- [Unable to start docker in docker with virtio-fs (#1888)](https://github.com/kata-containers/runtime/issues/1888)
- [Not using native diff for overlay2 (#1429)](https://github.com/kata-containers/runtime/issues/1429)
