# Kata Containers packaging

## Introduction

Kata Containers currently supports packages for many distributions. Tooling to
aid in creating these packages are contained within this repository.

## Build in a container

Kata build artifacts are available within a container image, created by a
[Dockerfile](kata-deploy/Dockerfile). Reference DaemonSets are provided in
[`kata-deploy`](kata-deploy), which make installation of Kata Containers in a
running Kubernetes Cluster very straightforward.

## Build static binaries

See [the static build documentation](static-build).

## Build Kata Containers Kernel

See [the kernel documentation](kernel).

## Build QEMU

See [the QEMU documentation](qemu).

## Create a Kata Containers release

See [the release documentation](release).

## Packaging scripts

See the [scripts documentation](scripts).

## Credits

Kata Containers packaging uses [packagecloud](https://packagecloud.io) for
package hosting.
