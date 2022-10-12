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
