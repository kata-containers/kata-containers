<img src="https://www.openstack.org/assets/kata/kata-vertical-on-white.png" width="150">

# Kata Containers

Welcome to Kata Containers! DO NOT MERGE - TEST

This repository is the home of the Kata Containers code for the 2.0 and newer
releases.

If you want to learn about Kata Containers, visit the main
[Kata Containers website](https://katacontainers.io).

## Introduction

Kata Containers is an open source project and community working to build a
standard implementation of lightweight Virtual Machines (VMs) that feel and
perform like containers, but provide the workload isolation and security
advantages of VMs.

## Getting started

See the [installation documentation](docs/install).

## Documentation

See the [official documentation](docs)
(including [installation guides](docs/install),
[the developer guide](docs/Developer-Guide.md),
[design documents](docs/design) and more).

## Community

To learn more about the project, its community and governance, see the
[community repository](https://github.com/kata-containers/community). This is
the first place to go if you wish to contribute to the project.

## Getting help

See the [community](#community) section for ways to contact us.

### Raising issues

Please raise an issue
[in this repository](https://github.com/kata-containers/kata-containers/issues).

> **Note:**
> If you are reporting a security issue, please follow the [vulnerability reporting process](https://github.com/kata-containers/community#vulnerability-handling)

## Developers

### Components

### Main components

The table below lists the core parts of the project:

| Component | Type | Description |
|-|-|-|
| [runtime](src/runtime) | core | Main component run by a container manager and providing a containerd shimv2 runtime implementation. |
| [agent](src/agent) | core | Management process running inside the virtual machine / POD that sets up the container environment. |
| [documentation](docs) | documentation | Documentation common to all components (such as design and install documentation). |
| [tests](https://github.com/kata-containers/tests) | tests | Excludes unit tests which live with the main code. |

### Additional components

The table below lists the remaining parts of the project:

| Component | Type | Description |
|-|-|-|
| [packaging](tools/packaging) | infrastructure | Scripts and metadata for producing packaged binaries<br/>(components, hypervisors, kernel and rootfs). |
| [kernel](https://www.kernel.org) | kernel | Linux kernel used by the hypervisor to boot the guest image. Patches are stored [here](tools/packaging/kernel). |
| [osbuilder](tools/osbuilder) | infrastructure | Tool to create "mini O/S" rootfs and initrd images and kernel for the hypervisor. |
| [`agent-ctl`](tools/agent-ctl) | utility | Tool that provides low-level access for testing the agent. |
| [`trace-forwarder`](src/trace-forwarder) | utility | Agent tracing helper. |
| [`ci`](https://github.com/kata-containers/ci) | CI | Continuous Integration configuration files and scripts. |
| [`katacontainers.io`](https://github.com/kata-containers/www.katacontainers.io) | Source for the [`katacontainers.io`](https://www.katacontainers.io) site. |

### Packaging and releases

Kata Containers is now
[available natively for most distributions](docs/install/README.md#packaged-installation-methods).
However, packaging scripts and metadata are still used to generate snap and GitHub releases. See
the [components](#components) section for further details.

## Glossary of Terms

See the [glossary of terms](Glossary.md) related to Kata Containers.
---

[kernel]: https://www.kernel.org
[github-katacontainers.io]: https://github.com/kata-containers/www.katacontainers.io
