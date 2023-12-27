# CSI Direct Volume Driver

The `Direct Volume CSI driver` is heavily inspired by the [`K8s CSI HostPath driver`](https://github.com/kubernetes-csi/csi-driver-host-path). It aims to provide a production-ready implementation and a reference implementation for Kubernetes to connect to `Direct Volume`.

This repository houses the `Direct Volume CSI driver`, along with all build and dependent configuration files needed for deployment.

*WARNING* It is important to note that it is still under development.

## Pre-requisite

- K8S cluster
- Running version 1.20 or later
- Access to terminal with `kubectl` installed

## Features

The driver can provision volumes based on direct block devices, eliminating the need for loop devices and relying solely on single files stored on the host.

## Deployment

[Deployment for K8S 1.20+](docs/deploy-csi-kata-directvol.md)

## Building the Binary

If you want to build the driver yourself, you can do so with the following command from `csi-kata-directvolume` path:

```shell
cd tools/csi-kata-directvolume/ && make
```

## Building the Container Image

If you want to build the container image yourself, you can do so with the following command from a specified path.
Here, we just use `buildah/podman` as an example:

```shell
$ tree -L 2 buildah-directv/
buildah-directv/
├── bin
│   └── directvolplugin
└── Dockerfile

$ buildah bud -t kata-directvolume:v1.0.19
STEP 1/7: FROM alpine
STEP 2/7: LABEL maintainers="Kata Containers Authors"
STEP 3/7: LABEL description="Kata DirectVolume Driver"
STEP 4/7: ARG binary=./bin/directvolplugin
STEP 5/7: RUN apk add util-linux coreutils e2fsprogs xfsprogs xfsprogs-extra btrfs-progs && apk update && apk upgrade
fetch https://dl-cdn.alpinelinux.org/alpine/v3.19/main/x86_64/APKINDEX.tar.gz
fetch https://dl-cdn.alpinelinux.org/alpine/v3.19/community/x86_64/APKINDEX.tar.gz
(1/66) Installing libblkid (2.39.3-r0)
...
(66/66) Installing xfsprogs-extra (6.5.0-r0)
Executing busybox-1.36.1-r15.trigger
OK: 64 MiB in 81 packages
fetch https://dl-cdn.alpinelinux.org/alpine/v3.19/main/x86_64/APKINDEX.tar.gz
fetch https://dl-cdn.alpinelinux.org/alpine/v3.19/community/x86_64/APKINDEX.tar.gz
v3.19.0-19-ga0ddaee500e [https://dl-cdn.alpinelinux.org/alpine/v3.19/main]
v3.19.0-18-gec62a609516 [https://dl-cdn.alpinelinux.org/alpine/v3.19/community]
OK: 22983 distinct packages available
OK: 64 MiB in 81 packages
STEP 6/7: COPY ${binary} /kata-directvol-plugin
STEP 7/7: ENTRYPOINT ["/kata-directvol-plugin"]
COMMIT kata-directvolume:v1.0.19
Getting image source signatures
Copying blob 5af4f8f59b76 skipped: already exists
Copying blob a55645705de3 done
Copying config 244001cc51 done
Writing manifest to image destination
Storing signatures
--> 244001cc51d
Successfully tagged localhost/kata-directvolume:v1.0.19
244001cc51d77302c4ed5e1a0ec347d12d85dec4576ea1313f700f66e2a7d36d
$ podman save localhost/kata-directvolume:v1.0.19 -o kata-directvolume-v1.0.19.tar
$ ctr -n k8s.io image import kata-directvolume-v1.0.19.tar
unpacking localhost/kata-directvolume:v1.0.19 (sha256:1bdc33ff7f9cee92e74cbf77a9d79d00dce6dbb9ba19b9811f683e1a087f8fbf)...done
$ crictl images |grep 1.0.19
localhost/kata-directvolume                                          v1.0.19             244001cc51d77       83.8MB
```
