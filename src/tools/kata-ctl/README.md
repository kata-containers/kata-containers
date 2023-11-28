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

### Full details

For a usage statement, run:

```bash
$ kata-ctl --help
```

## Cross-builds

For developers that want to build and test the `kata-ctl` tool on various architectures,
the makefile included does have support for that. This would however, require installing 
the cross compile toolchain for the target architecture on the host along with required libraries.

[Cross](https://github.com/cross-rs/cross) is an open source tool that offers zero setup
cross compile and requires no changes to the system installation for cross-compiling
rust binaries. It makes use of docker containers for cross-compilation.

You can install cross with:
```
cargo install -f cross
```

`cross` relies on `docker` or `podman`. For dependencies take a look at: https://github.com/cross-rs/cross#dependencies

There is an included `cross` configuration file [Cross.yaml](./Cross.toml) that can be used
to compile `kata-ctl` for various targets. This configuration helps install required
dependencies inside a docker container.

For example, to compile for target `s390x-unknown-linux-gnu` included in `Cross.yaml` simple run:
```
cross build --target=s390x-unknown-linux-gnu
```

You may also need to add the target on your host system prior to the above step as:
```
rustup target add s390x-unknown-linux-gnu
``` 

## Documentation for included tools:
| Component | Description |
| [`log-parser`](src/log_parser) | Tool that aid in analyzing logs from the kata runtime. |
