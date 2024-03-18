# Release information

## Introduction

This directory contains information of the process and
tools used for creating Kata Containers releases.

## Create a Kata Containers release

See [the release documentation](../../../docs/Release-Process.md).

## Release tools

### `release.sh`

This script is used by [GitHub actions](https://github.com/features/actions) in the
[release](https://github.com/kata-containers/kata-containers/actions/workflows/release.yaml)
file from the `kata-containers/kata-containers` repository to handle the various steps of
the release process.

### `generate_vendor.sh`

This script is used by `release.sh` to generate a tarball with all the cargo vendored code.
