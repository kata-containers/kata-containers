# `kata-sys-util`

System utilities and helpers for [Kata Containers](https://github.com/kata-containers/kata-containers/) components to access Linux system services.

## Overview

This crate provides safe wrappers and utility functions for interacting with various Linux system services and kernel interfaces. It is designed specifically for the Kata Containers ecosystem.

## Features

### File System Operations (`fs`)

- Path canonicalization and basename extraction
- Filesystem type detection (FUSE, OverlayFS)
- Symlink detection
- Reflink copy with fallback to regular copy

### Mount Operations (`mount`)

- Bind mount and remount operations
- Mount propagation type management (SHARED, PRIVATE, SLAVE, UNBINDABLE)
- Overlay filesystem mount option compression
- Safe mount destination creation
- Umount with timeout support
- `/proc/mounts` parsing utilities

### CPU Utilities (`cpu`)

- CPU information parsing from `/proc/cpuinfo`
- CPU flags detection and validation
- Architecture-specific support (x86_64, s390x)

### NUMA Support (`numa`)

- CPU to NUMA node mapping
- NUMA node information retrieval from sysfs
- NUMA CPU validation

### Device Management (`device`)

- Block device major/minor number detection
- Device ID resolution for cgroup operations

### Kubernetes Support (`k8s`)

- Ephemeral volume detection
- EmptyDir volume handling
- Kubernetes-specific mount type identification

### Network Namespace (`netns`)

- Network namespace switching with RAII guard pattern
- Network namespace name generation

### OCI Specification Utilities (`spec`)

- Container type detection (PodSandbox, PodContainer)
- Sandbox ID extraction from OCI annotations
- OCI spec loading utilities

### Validation (`validate`)

- Container/exec ID validation
- Environment variable validation

### Hooks (`hooks`)

- OCI hook execution and management
- Hook state tracking
- Timeout handling for hook execution

### Guest Protection (`protection`)

- Confidential computing detection (TDX, SEV, SNP, PEF, SE, ARM CCA , etc.)
- Architecture-specific protection checking (x86_64, s390x, aarch64, powerpc64)

### Random Generation (`rand`)

- Secure random byte generation
- UUID generation

### PCI Device Management (`pcilibs`)

- PCI device enumeration and management
- PCI configuration space access
- Memory resource allocation for PCI devices

## Supported Architectures

- x86_64
- aarch64
- s390x
- powerpc64 (little-endian)
- riscv64

## Supported Operating Systems

- Linux

## License

This code is licensed under [Apache-2.0](../../../LICENSE).
