[![Go Report Card](https://goreportcard.com/badge/github.com/kata-containers/kata-containers)](https://goreportcard.com/report/github.com/kata-containers/kata-containers)

# Runtime

## Binary names

This repository contains the following components:

| Binary name | Description |
|-|-|
| `containerd-shim-kata-v2` | The [shimv2 runtime](../../docs/design/architecture/README.md#runtime) |
| `kata-runtime` | [utility program](../../docs/design/architecture/README.md#utility-program) |
| `kata-monitor` | [metrics collector daemon](cmd/kata-monitor/README.md) |

For details of the other Kata Containers repositories, see the
[repository summary](https://github.com/kata-containers/kata-containers).

## Introduction

The `containerd-shim-kata-v2` [binary](#binary-names) is the Kata
Containers [shimv2](../../docs/design/architecture/README.md#shim-v2-architecture) runtime. It leverages the
[virtcontainers](virtcontainers)
package to provide a high-performance standards-compliant runtime that creates
hardware-virtualized [Linux](https://www.kernel.org) containers running on Linux hosts.

The runtime is
[OCI](https://github.com/opencontainers/runtime-spec)-compatible,
[CRI-O](https://github.com/cri-o/cri-o)-compatible, and
[Containerd](https://github.com/containerd/containerd)-compatible,
 allowing it
to work seamlessly with both Docker and Kubernetes respectively.

## Download and install

See the [installation guides](../../docs/install/README.md)
available for various operating systems.

## Architecture overview

See the [architecture overview](../../docs/design/architecture)
for details on the Kata Containers design.

## Configuration

The runtime uses a TOML format configuration file called `configuration.toml`.
The file is divided into sections for settings related to various
parts of the system including the runtime itself, the [agent](../agent) and
the [hypervisor](#hypervisor-specific-configuration).

Each option has a comment explaining its use.

> **Note:**
>
> The initial values in the configuration file provide a good default configuration.
> You may need to modify this file to optimise or tailor your system, or if you have
> specific requirements.

### Configuration file location

#### Runtime configuration file location

The shimv2 runtime looks for its configuration in the following places (in order):

- The `io.data containers.config.config_path` annotation specified
  in the OCI configuration file (`config.json` file) used to create the pod sandbox.

- The containerd
  [shimv2](/docs/design/architecture/README.md#shim-v2-architecture)
  options passed to the runtime.

- The value of the `KATA_CONF_FILE` environment variable.

- The [default configuration paths](#stateless-systems).

#### Utility program configuration file location

The `kata-runtime` utility program looks for its configuration in the
following locations (in order):

- The path specified by the `--config` command-line option.

- The value of the `KATA_CONF_FILE` environment variable.

- The [default configuration paths](#stateless-systems).

> **Note:** For both binaries, the first path that exists will be used.

#### Drop-in configuration file fragments

To enable changing configuration without changing the configuration file
itself, drop-in configuration file fragments are supported.  Once a
configuration file is parsed, if there is a subdirectory called `config.d` in
the same directory as the configuration file its contents will be loaded
in alphabetical order and each item will be parsed as a config file.  Settings
loaded from these configuration file fragments override settings loaded from
the main configuration file and earlier fragments.  Users are encouraged to use
familiar naming conventions to order the fragments (e.g. `config.d/10-this`,
`config.d/20-that` etc.).

Non-existent or empty `config.d` directory is not an error (in other words, not
using configuration file fragments is fine).  On the other hand, if fragments
are used, they must be valid - any errors while parsing fragments (unreadable
fragment files, contents not valid TOML) are treated the same as errors
while parsing the main configuration file.  A `config.d` subdirectory affects
only the `configuration.toml` _in the same directory_.  For fragments in
`config.d` to be parsed, there has to be a valid main configuration file _in
that location_ (it can be empty though).

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
[`kata-log-parser`](../tools/log-parser)
tool.

### Kata containerd shimv2

The Kata containerd shimv2 runtime logs through `containerd`, and its logs will be sent
to wherever the `containerd` logs are directed. However, the
shimv2 runtime also always logs to the system log (`syslog` or `journald`) using the `kata` identifier.

> **Note:** Kata logging [requires containerd debug to be enabled](../../docs/Developer-Guide.md#enabling-full-containerd-debug).

To view the `shimv2` runtime logs:

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

See [how to reach the community](https://github.com/kata-containers/community/blob/main/CONTRIBUTING.md#contact).

## Further information

See the
[project table of contents](https://github.com/kata-containers/kata-containers)
and the
[documentation repository](../../docs).

## Additional packages

For details of the other packages contained in this repository, see the
[package documentation](pkg).
