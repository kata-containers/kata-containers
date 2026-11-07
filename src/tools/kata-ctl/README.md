# Kata Containers control tool

## Overview

The `kata-ctl` tool is a rust rewrite of the
[`kata-runtime`](../../runtime/cmd/kata-runtime)
[utility program](../../../docs/design/architecture/README.md#utility-program).

The program provides a number of utility commands for:

- Using advanced Kata Containers features.
- Problem determination and debugging.

## Audience and environment

Users and administrators.

## Build the tool

```bash
$ make
```

## Install the tool

```bash
$ make install
```

If you would like to install the tool to a specific directory, then you can provide it through the `INSTALL_PATH` variable.
```bash
$ make install INSTALL_PATH=/path/to/your/custom/install/directory
```

## Run the tool

```bash
$ kata-ctl ...
```

For example, to determine if your system is capable of running Kata
Containers, run:

```bash
$ kata-ctl check all
```

### Runtime information

Use `kata-ctl env` to display the host and configured component details, or
`kata-ctl env --json` for JSON output.

The runtime section resolves `containerd-shim-kata-v2` from `PATH`, follows
symbolic links, and reports the shim's path, `variant` (`rust` or `golang`),
version and commit. The `search_method` field is `path`: this identifies the
discovery method, not necessarily the shim selected by a container manager
with an explicit runtime path. An unavailable shim or an unsuccessful or
unrecognized version response is reported as an error.

### Full details

For a usage statement, run:

```bash
$ kata-ctl --help
```

## Documentation for included tools:
| Component | Description |
| [`log-parser`](src/log_parser) | Tool that aid in analyzing logs from the kata runtime. |
