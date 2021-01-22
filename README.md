<img src="https://www.openstack.org/assets/kata/kata-vertical-on-white.png" width="150">

# Kata Containers

* [Kata Containers](#kata-containers)
    * [Introduction](#introduction)
    * [Getting started](#getting-started)
    * [Documentation](#documentation)
    * [Community](#community)
    * [Getting help](#getting-help)
        * [Raising issues](#raising-issues)
            * [Kata Containers 1.x versions](#kata-containers-1x-versions)
    * [Developers](#developers)
        * [Components](#components)
            * [Kata Containers 1.x components](#kata-containers-1x-components)
        * [Common repositories](#common-repositories)
        * [Packaging and releases](#packaging-and-releases)

---

Welcome to Kata Containers!

This repository is the home of the Kata Containers code for the 2.0 and newer
releases.

If you want to learn about Kata Containers, visit the main
[Kata Containers website](https://katacontainers.io).

For further details on the older (first generation) Kata Containers 1.x
versions, see the
[Kata Containers 1.x components](#kata-containers-1x-components)
section.

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

#### Kata Containers 1.x versions

For older Kata Containers 1.x releases, please raise an issue in the
[Kata Containers 1.x component repository](#kata-containers-1x-components)
that seems most appropriate.

If in doubt, raise an issue
[in the Kata Containers 1.x runtime repository](https://github.com/kata-containers/runtime/issues).

## Developers

### Components

| Component | Type | Description |
|-|-|-|
| [agent-ctl](tools/agent-ctl) | utility | Tool that provides low-level access for testing the agent. |
| [agent](src/agent) | core | Management process running inside the virtual machine / POD that sets up the container environment. |
| [documentation](docs) | documentation | Documentation common to all components (such as design and install documentation). |
| [osbuilder](tools/osbuilder) | infrastructure | Tool to create "mini O/S" rootfs and initrd images for the hypervisor. |
| [packaging](tools/packaging) | infrastructure | Scripts and metadata for producing packaged binaries<br/>(components, hypervisors, kernel and rootfs). |
| [runtime](src/runtime) | core | Main component run by a container manager and providing a containerd shimv2 runtime implementation. |
| [trace-forwarder](src/trace-forwarder) | utility | Agent tracing helper. |

#### Kata Containers 1.x components

For the first generation of Kata Containers (1.x versions), each component was
kept in a separate repository.

For information on the Kata Containers 1.x releases, see the
[Kata Containers 1.x releases page](https://github.com/kata-containers/runtime/releases).

For further information on particular Kata Containers 1.x components, see the
individual component repositories:

| Component | Type | Description |
|-|-|-|
| [agent](https://github.com/kata-containers/agent) | core | See [components](#components). |
| [documentation](https://github.com/kata-containers/documentation) | documentation | |
| [KSM throttler](https://github.com/kata-containers/ksm-throttler) | optional core | Daemon that monitors containers and deduplicates memory to maximize container density on the host. |
| [osbuilder](https://github.com/kata-containers/osbuilder) | infrastructure | See [components](#components). |
| [packaging](https://github.com/kata-containers/packaging) | infrastructure | See [components](#components). |
| [proxy](https://github.com/kata-containers/proxy) | core | Multiplexes communications between the shims, agent and runtime. |
| [runtime](https://github.com/kata-containers/runtime) | core | See [components](#components). |
| [shim](https://github.com/kata-containers/shim) | core | Handles standard I/O and signals on behalf of the container process. |

> **Note:**
>
> - There are more components for the original Kata Containers 1.x implementation.
> - The current implementation simplifies the design significantly:
>   compare the [current](docs/design/architecture.md) and
>   [previous generation](https://github.com/kata-containers/documentation/blob/master/design/architecture.md)
>   designs.

### Common repositories

The following repositories are used by both the current and first generation Kata Containers implementations:

| Component | Description | Current | First generation | Notes |
|-|-|-|-|-|
| CI | Continuous Integration configuration files and scripts. | [Kata 2.x](https://github.com/kata-containers/ci/tree/main) | [Kata 1.x](https://github.com/kata-containers/ci/tree/master) | |
| kernel | The Linux kernel used by the hypervisor to boot the guest image. | [Kata 2.x][kernel] | [Kata 1.x][kernel] | Patches are stored in the packaging component. |
| tests | Test code. | [Kata 2.x](https://github.com/kata-containers/tests/tree/main) | [Kata 1.x](https://github.com/kata-containers/tests/tree/master) | Excludes unit tests which live with the main code. |
| www.katacontainers.io | Contains the source for the [main web site](https://www.katacontainers.io). | [Kata 2.x][github-katacontainers.io] | [Kata 1.x][github-katacontainers.io] | | |

### Packaging and releases

Kata Containers is now
[available natively for most distributions](docs/install/README.md#packaged-installation-methods).
However, packaging scripts and metadata are still used to generate snap and GitHub releases. See
the [components](#components) section for further details.

---

[kernel]: https://www.kernel.org
[github-katacontainers.io]: https://github.com/kata-containers/www.katacontainers.io
