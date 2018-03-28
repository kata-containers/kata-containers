[![Build Status](https://travis-ci.org/kata-containers/runtime.svg?branch=master)](https://travis-ci.org/kata-containers/runtime)
[![Build Status](http://kata-jenkins-ci.westus2.cloudapp.azure.com/job/kata-containers-runtime-ubuntu-16-04-master/badge/icon)](http://kata-jenkins-ci.westus2.cloudapp.azure.com/job/kata-containers-runtime-ubuntu-16-04-master/)
[![Go Report Card](https://goreportcard.com/badge/github.com/kata-containers/runtime)](https://goreportcard.com/report/github.com/kata-containers/runtime)

# Runtime

This repository contains the runtime for the
[Kata Containers](https://github.com/kata-containers) project.

For details of the other Kata Containers repositories, see the
[repository summary](https://github.com/kata-containers/kata-containers).

* [Introduction](#introduction)
* [License](#license)
* [Platform support](#platform-support)
    * [Hardware requirements](#hardware-requirements)
* [Quick start for developers](#quick-start-for-developers)
* [Configuration](#configuration)
* [Logging](#logging)
* [Debugging](#debugging)
* [Community](#community)

## Introduction

`kata-runtime`, referred to as "the runtime", is the Command-Line Interface
(CLI) part of the Kata Containers runtime component. It leverages the
[virtcontainers](https://github.com/kata-containers/runtime/tree/master/virtcontainers)
package to provide a high-performance standards-compliant runtime that creates
hardware-virtualized containers.

The runtime is both
[OCI](https://github.com/opencontainers/runtime-spec)-compatible and
[CRI-O](https://github.com/kubernetes-incubator/cri-o)-compatible, allowing it
to work seamlessly with both Docker and Kubernetes respectively.

## License

The code is licensed under an Apache 2.0 license.

See [the license file](LICENSE) for further details.

## Platform support

Kata Containers currently works on systems supporting the following
technologies:

- [Intel](https://www.intel.com)'s VT-x technology.
- [ARM](https://www.arm.com)'s Hyp mode (virtualization extension).

### Hardware requirements

The runtime has a built-in command to determine if your host system is capable
of running a Kata Container:

```bash
$ kata-runtime kata-check
```

> **Note:**
>
> If you run the previous command as the `root` user, further checks will be
> performed (e.g. it will check if another incompatible hypervisor is running):
>
> ```bash
> $ sudo kata-runtime kata-check
> ```

## Quick start for developers

See the
[developer guide](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md).

## Configuration

The runtime uses a TOML format configuration file called `configuration.toml`.
The file contains comments explaining all options.

> **Note:**
>
> The initial values in the configuration file provide a good default configuration.
> You might need to modify this file if you have specialist needs.

Since the runtime supports a
[stateless system](https://clearlinux.org/features/stateless),
it checks for this configuration file in multiple locations, two of which are
built in to the runtime. The default location is
`/usr/share/defaults/kata-containers/configuration.toml` for a standard
system. However, if `/etc/kata-containers/configuration.toml` exists, this
takes priority.

The command below lists the full paths to the configuration files that the
runtime attempts to load. The first path that exists is used:

```bash
$ kata-runtime --kata-show-default-config-paths
```

Aside from the built-in locations, it is possible to specify the path to a
custom configuration file using the `--kata-config` option:

```bash
$ kata-runtime --kata-config=/some/where/configuration.toml ...
```

The runtime will log the full path to the configuration file it is using. See
the [logging](#Logging) section for further details.

To see details of your systems runtime environment (including the location of
the configuration file being used), run:

```bash
$ kata-runtime kata-env
```

## Logging

The runtime provides `--log=` and `--log-format=` options. However, the
runtime always logs to the system log (`syslog` or `journald`).

To view runtime log output:

```bash
$ sudo journalctl -t kata-runtime
```

For detailed information and analysis on obtaining logs for other system
components, see the documentation for the
[kata-log-parser](https://github.com/kata-containers/tests/tree/master/cmd/log-parser)
tool.

## Debugging

See the
[debugging section of the developer guide](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md#enable-full-debug).

## Community

See [the community repository](https://github.com/kata-containers/community).
