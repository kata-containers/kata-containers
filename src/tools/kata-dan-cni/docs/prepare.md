# Prerequisites

kata-dan-cni is a CNI plugin that provides Directly Attachable Network (DAN) support for Kata Containers. It reuses Kube-OVN's network allocations to create network endpoints for virtual machines and generate DAN configuration.

## Environment requirements

- A working Kube-OVN network. See the [Kube-OVN prerequisites](https://github.com/kubeovn/docs/blob/master/docs/start/prepare.md).
- A runtime-rs installation with DAN support. See the [Kata installation guide](https://github.com/kata-containers/kata-containers/blob/main/docs/installation.md). By default, the plugin uses Kata's `kata-qemu-runtime-rs` RuntimeClass to distinguish between runtimes. The DAN configuration directory uses Kata's default path, `/run/kata-containers/dans`.
- containerd [1.5.0+](https://github.com/containerd/containerd/releases/tag/v1.5.0).
- Docker [23.0+](https://docs.docker.com/engine/release-notes/23.0/) is recommended.
- Node hosts must use glibc **2.30 or later**.
