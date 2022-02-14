# Kata Containers with virtio-fs

## Introduction

Container deployments utilize explicit or implicit file sharing between host filesystem and containers. From a trust perspective, avoiding a shared file-system between the trusted host and untrusted container is recommended. This is not always feasible. In Kata Containers, block-based volumes are preferred as they allow usage of either device pass through or `virtio-blk` for access within the virtual machine.

As of the 2.0 release of Kata Containers, [virtio-fs](https://virtio-fs.gitlab.io/) is the default filesystem sharing mechanism.

virtio-fs support works out of the box for `cloud-hypervisor` and `qemu`, when Kata Containers is deployed using `kata-deploy`. Learn more about `kata-deploy` and how to use `kata-deploy` in Kubernetes [here](../../tools/packaging/kata-deploy/README.md#kubernetes-quick-start).
