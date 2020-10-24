[![Build Status](https://travis-ci.org/kata-containers/kata-containers.svg?branch=master)](https://travis-ci.org/kata-containers/kata-containers)
[![Build Status](http://jenkins.katacontainers.io/job/kata-containers-runtime-ubuntu-18-04-master/badge/icon)](http://jenkins.katacontainers.io/job/kata-containers-runtime-ubuntu-18-04-master/)
[![Go Report Card](https://goreportcard.com/badge/github.com/kata-containers/kata-containers)](https://goreportcard.com/report/github.com/kata-containers/kata-containers)
[![GoDoc](https://godoc.org/github.com/kata-containers/runtime?status.svg)](https://godoc.org/github.com/kata-containers/runtime)

# Runtime

This repository contains the runtime for the
[Kata Containers](https://github.com/kata-containers) project.

For details of the other Kata Containers repositories, see the
[repository summary](https://github.com/kata-containers/kata-containers).

* [Introduction](#introduction)
* [License](#license)
* [Platform support](#platform-support)
    * [Hardware requirements](#hardware-requirements)
* [Download and install](#download-and-install)
* [Quick start for developers](#quick-start-for-developers)
* [Architecture overview](#architecture-overview)
* [Configuration](#configuration)
    * [Hypervisor specific configuration](#hypervisor-specific-configuration)
    * [Stateless systems](#stateless-systems)
* [Logging](#logging)
    * [Kata OCI](#kata-oci)
    * [Kata containerd shimv2](#kata-containerd-shimv2)
* [Debugging](#debugging)
* [Limitations](#limitations)
* [Community](#community)
    * [Contact](#contact)
* [Further information](#further-information)
* [Additional packages](#additional-packages)

## Introduction

`kata-runtime`, referred to as "the runtime", is the Command-Line Interface
(CLI) part of the Kata Containers runtime component. It leverages the
[virtcontainers](virtcontainers)
package to provide a high-performance standards-compliant runtime that creates
hardware-virtualized [Linux](https://www.kernel.org/) containers running on Linux hosts.

The runtime is
[OCI](https://github.com/opencontainers/runtime-spec)-compatible,
[CRI-O](https://github.com/cri-o/cri-o)-compatible, and
[Containerd](https://github.com/containerd/containerd)-compatible,
 allowing it
to work seamlessly with both Docker and Kubernetes respectively.

## License

The code is licensed under an Apache 2.0 license.

See [the license file](LICENSE) for further details.

## Platform support

Kata Containers currently works on systems supporting the following
technologies:

- [Intel](https://www.intel.com) VT-x technology.
- [ARM](https://www.arm.com) Hyp mode (virtualization extension).
- [IBM](https://www.ibm.com) Power Systems.
- [IBM](https://www.ibm.com) Z mainframes.
### Hardware requirements

The runtime has a built-in command to determine if your host system is capable
of running and creating a Kata Container:

```bash
$ kata-runtime check
```

> **Note:**
>
> - By default, only a brief success / failure message is printed.
> If more details are needed, the `--verbose` flag can be used to display the
> list of all the checks performed.
>
> - `root` permission is needed to check if the system is capable of running
> Kata containers. In this case, additional checks are performed (e.g., if another
> incompatible hypervisor is running).

## Download and install

[![Get it from the Snap Store](https://snapcraft.io/static/images/badges/en/snap-store-black.svg)](https://snapcraft.io/kata-containers)

See the [installation guides](https://github.com/kata-containers/documentation/tree/master/install/README.md)
available for various operating systems.

## Quick start for developers

See the
[developer guide](../../docs/Developer-Guide.md).

## Architecture overview

See the [architecture overview](../../docs/design/architecture.md)
for details on the Kata Containers design.

## Configuration

The runtime uses a TOML format configuration file called `configuration.toml`.
The file contains comments explaining all options.

> **Note:**
>
> The initial values in the configuration file provide a good default configuration.
> You may need to modify this file to optimise or tailor your system, or if you have
> specific requirements.

### Hypervisor specific configuration

Kata Containers supports multiple hypervisors so your `configuration.toml`
configuration file may be a symbolic link to a hypervisor-specific
configuration file. See
[the hypervisors document](../../docs/hypervisors.md) for further details.

### Stateless systems

Since the runtime supports a
[stateless system](https://clearlinux.org/about),
it checks for this configuration file in multiple locations, two of which are
built in to the runtime. The default location is
`/usr/share/defaults/kata-containers/configuration.toml` for a standard
system. However, if `/etc/kata-containers/configuration.toml` exists, this
takes priority.

The below command lists the full paths to the configuration files that the
runtime attempts to load. The first path that exists will be used:

```bash
$ kata-runtime --show-default-config-paths
```

Aside from the built-in locations, it is possible to specify the path to a
custom configuration file using the `--config` option:

```bash
$ kata-runtime --config=/some/where/configuration.toml ...
```

The runtime will log the full path to the configuration file it is using. See
the [logging](#logging) section for further details.

To see details of your systems runtime environment (including the location of
the configuration file being used), run:

```bash
$ kata-runtime env
```

## Logging

For detailed information and analysis on obtaining logs for other system
components, see the documentation for the
[`kata-log-parser`](https://github.com/kata-containers/tests/tree/master/cmd/log-parser)
tool.

For runtime logs, see the following sections for the CRI-O and containerd shimv2 based runtimes.

### Kata OCI

The Kata OCI runtime (including when used with CRI-O), provides `--log=` and `--log-format=` options.
However, the runtime also always logs to the system log (`syslog` or `journald`).

To view runtime log output:

```bash
$ sudo journalctl -t kata-runtime
```

### Kata containerd shimv2

The Kata containerd shimv2 runtime logs through `containerd`, and its logs will be sent
to wherever the `containerd` logs are directed. However, the
shimv2 runtime also always logs to the system log (`syslog` or `journald`) under the
identifier name of `kata`.

To view the `shimv2` runtime log output:

```bash
$ sudo journalctl -t kata
```

## Debugging

See the
[debugging section of the developer guide](../../docs/Developer-Guide.md#troubleshoot-kata-containers).

## Limitations

See the
[limitations file](../../docs/Limitations.md)
for further details.

## Community

See [the community repository](https://github.com/kata-containers/community).

### Contact

See [how to reach the community](https://github.com/kata-containers/community/blob/master/CONTRIBUTING.md#contact).

## Further information

See the
[project table of contents](https://github.com/kata-containers/kata-containers)
and the
[documentation repository](../../docs).

## Additional packages

For details of the other packages contained in this repository, see the
[package documentation](pkg).
