# kata-types

Constants and data types shared by Kata Containers components.

## Overview

This crate provides common constants, data types, and configuration structures used across multiple [Kata Containers](https://github.com/kata-containers/kata-containers/) components. It includes definitions from:

- Kata Containers project
- [Containerd](https://github.com/containerd/containerd)
- [Kubelet](https://github.com/kubernetes/kubernetes)

## Modules

| Module | Description |
|--------|-------------|
| `annotations` | Annotation keys for CRI-containerd, CRI-O, dockershim, and third-party integrations |
| `capabilities` | Hypervisor capability flags (block device, multi-queue, filesystem sharing, etc.) |
| `config` | Configuration structures for agent, hypervisor (QEMU, Cloud Hypervisor, Firecracker, Dragonball), and runtime |
| `container` | Container-related constants and types |
| `cpu` | CPU resource management types |
| `device` | Device-related definitions |
| `fs` | Filesystem constants |
| `handler` | Handler-related types |
| `initdata` | Initdata specification for TEE data injection |
| `k8s` | Kubernetes-specific paths and utilities (empty-dir, configmap, secret, projected volumes) |
| `machine_type` | Machine type definitions |
| `mount` | Mount point structures and validation |
| `rootless` | Rootless VMM support utilities |

## Configuration

The `config` module supports:

- TOML-based configuration loading
- Drop-in configuration files
- Hypervisor-specific configurations (QEMU, Cloud Hypervisor, Firecracker, Dragonball, Remote)
- Agent configuration
- Runtime configuration
- Shared mount definitions

## Features

- `enable-vendor`: Enable vendor-specific extensions
- `safe-path`: Enable safe path resolution (platform-specific)

## Platform Support

- **Linux**: Fully supported

## License

Apache-2.0 - See [LICENSE](../../../LICENSE)
