<img src="https://object-storage-ca-ymq-1.vexxhost.net/swift/v1/6e4619c416ff4bd19e1c087f27a43eea/www-images-prod/openstack-logo/kata/SVG/kata-1.svg" width="900">

[![CI | Publish Kata Containers payload](https://github.com/kata-containers/kata-containers/actions/workflows/payload-after-push.yaml/badge.svg)](https://github.com/kata-containers/kata-containers/actions/workflows/payload-after-push.yaml) [![Kata Containers Nightly CI](https://github.com/kata-containers/kata-containers/actions/workflows/ci-nightly.yaml/badge.svg)](https://github.com/kata-containers/kata-containers/actions/workflows/ci-nightly.yaml)

# Kata Containers

Welcome to Kata Containers!

This repository is the home of the Kata Containers code for the 2.0 and newer
releases.

If you want to learn about Kata Containers, visit the main
[Kata Containers website](https://katacontainers.io).

## Introduction

Kata Containers is an open source project and community working to build a
standard implementation of lightweight Virtual Machines (VMs) that feel and
perform like containers, but provide the workload isolation and security
advantages of VMs.

## License

The code is licensed under the Apache 2.0 license.
See [the license file](LICENSE) for further details.

## Platform support

Kata Containers currently runs on 64-bit systems supporting the following
technologies:

| Architecture | Virtualization technology |
|-|-|
| `x86_64`, `amd64` | [Intel](https://www.intel.com) VT-x, AMD SVM |
| `aarch64` ("`arm64`")| [ARM](https://www.arm.com) Hyp |
| `ppc64le` | [IBM](https://www.ibm.com) Power |
| `s390x` | [IBM](https://www.ibm.com) Z & LinuxONE SIE |

### Hardware requirements

The [Kata Containers runtime](src/runtime) provides a command to
determine if your host system is capable of running and creating a
Kata Container:

```bash
$ kata-runtime check
```

> **Notes:**
>
> - This command runs a number of checks including connecting to the
>   network to determine if a newer release of Kata Containers is
>   available on GitHub. If you do not wish this to check to run, add
>   the `--no-network-checks` option.
>
> - By default, only a brief success / failure message is printed.
>   If more details are needed, the `--verbose` flag can be used to display the
>   list of all the checks performed.
>
> - If the command is run as the `root` user additional checks are
>   run (including checking if another incompatible hypervisor is running).
>   When running as `root`, network checks are automatically disabled.

## Getting started

See the [installation documentation](docs/install).

## Documentation

See the [official documentation](docs) including:

- [Installation guides](docs/install)
- [Developer guide](docs/Developer-Guide.md)
- [Design documents](docs/design)
  - [Architecture overview](docs/design/architecture)
  - [Architecture 3.0 overview](docs/design/architecture_3.0/)

## Configuration

Kata Containers uses a single
[configuration file](src/runtime/README.md#configuration)
which contains a number of sections for various parts of the Kata
Containers system including the [runtime](src/runtime), the
[agent](src/agent) and the [hypervisor](#hypervisors).

## Hypervisors

See the [hypervisors document](docs/hypervisors.md) and the
[Hypervisor specific configuration details](src/runtime/README.md#hypervisor-specific-configuration).

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

See the [developer guide](docs/Developer-Guide.md).

### Components

### Main components

The table below lists the core parts of the project:

| Component | Type | Description |
|-|-|-|
| [runtime](src/runtime) | core | Main component run by a container manager and providing a containerd shimv2 runtime implementation. |
| [runtime-rs](src/runtime-rs) | core | The Rust version runtime. |
| [agent](src/agent) | core | Management process running inside the virtual machine / POD that sets up the container environment. |
| [`dragonball`](src/dragonball) | core | An optional built-in VMM brings out-of-the-box Kata Containers experience with optimizations on container workloads |
| [documentation](docs) | documentation | Documentation common to all components (such as design and install documentation). |
| [tests](tests) | tests | Excludes unit tests which live with the main code. |

### Additional components

The table below lists the remaining parts of the project:

| Component | Type | Description |
|-|-|-|
| [packaging](tools/packaging) | infrastructure | Scripts and metadata for producing packaged binaries<br/>(components, hypervisors, kernel and rootfs). |
| [kernel](https://www.kernel.org) | kernel | Linux kernel used by the hypervisor to boot the guest image. Patches are stored [here](tools/packaging/kernel). |
| [osbuilder](tools/osbuilder) | infrastructure | Tool to create "mini O/S" rootfs and initrd images and kernel for the hypervisor. |
| [kata-debug](tools/packaging/kata-debug/README.md) | infrastructure | Utility tool to gather Kata Containers debug information from Kubernetes clusters. |
| [`agent-ctl`](src/tools/agent-ctl) | utility | Tool that provides low-level access for testing the agent. |
| [`kata-ctl`](src/tools/kata-ctl) | utility | Tool that provides advanced commands and debug facilities. |
| [`trace-forwarder`](src/tools/trace-forwarder) | utility | Agent tracing helper. |
| [`runk`](src/tools/runk) | utility | Standard OCI container runtime based on the agent. |
| [`ci`](.github/workflows) | CI | Continuous Integration configuration files and scripts. |
| [`ocp-ci`](ci/openshift-ci/README.md) | CI | Continuous Integration configuration for the OpenShift pipelines. |
| [`katacontainers.io`](https://github.com/kata-containers/www.katacontainers.io) | Source for the [`katacontainers.io`](https://www.katacontainers.io) site. |
| [`Webhook`](tools/testing/kata-webhook/README.md) | utility | Example of a simple admission controller webhook to annotate pods with the Kata runtime class |

### Packaging and releases

Kata Containers is now
[available natively for most distributions](docs/install/README.md#packaged-installation-methods).

## General tests

See the [tests documentation](tests/README.md).

## Metrics tests

See the [metrics documentation](tests/metrics/README.md).

## Glossary of Terms

See the [glossary of terms](https://github.com/kata-containers/kata-containers/wiki/Glossary) related to Kata Containers.
