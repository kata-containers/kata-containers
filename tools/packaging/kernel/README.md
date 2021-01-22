# Build Kata Containers Kernel

* [Requirements](#requirements)
* [Usage](#usage)
* [Setup kernel source code](#setup-kernel-source-code)
* [Build the kernel](#build-the-kernel)
* [Install the Kernel in the default path for Kata](#install-the-kernel-in-the-default-path-for-kata)
* [Submit Kernel Changes](#submit-kernel-changes)
* [How is it tested](#how-is-it-tested)
* [Contribute](#contribute)

This document explains the steps to build a kernel recommended for use with
Kata Containers. To do this use `build-kernel.sh`, this script
automates the process to build a kernel for Kata Containers.

## Requirements

The `build-kernel.sh` script requires an installed Golang version matching the
[component build requirements](../../../docs/Developer-Guide.md#requirements-to-build-individual-components).

## Usage

```
$ ./build-kernel.sh -h
Overview:

	Build a kernel for Kata Containers

Description: This script is the *ONLY* to build a kernel for development.


Usage:

	build-kernel.sh [options] <command> <argument>

Commands:

- setup

- build

- install

Options:

	-a <arch>       : Arch target to build the kernel, such as aarch64/ppc64le/s390x/x86_64.
	-c <path>   	: Path to config file to build the kernel.
	-d          	: Enable bash debug.
	-e          	: Enable experimental kernel.
	-f          	: Enable force generate config when setup.
	-g <vendor> 	: GPU vendor, intel or nvidia.
	-h          	: Display this help.
	-k <path>   	: Path to kernel to build.
	-p <path>   	: Path to a directory with patches to apply to kernel.
	-t <hypervisor>	: Hypervisor_target.
	-v <version>	: Kernel version to use if kernel path not provided.
```

Example:
```
$ ./build-kernel.sh -v 4.19.86 -g nvidia -f -d setup
```
> **Note**
> - `-v 4.19.86`: Specify the guest kernel version.
> - `-g nvidia`: To build a guest kernel supporting Nvidia GPU.
> - `-f`: The `.config` file is forced to be generated even if the kernel directory already exists.
> - `-d`: Enable bash debug mode.


## Setup kernel source code

```bash
$ go get -d -u github.com/kata-containers/kata-containers
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kernel
$ ./build-kernel.sh setup
```

The script `./build-kernel.sh` tries to apply the patches from
`${GOPATH}/src/github.com/kata-containers/kata-containers/tools/packaging/kernel/patches/` when it
sets up a kernel. If you want to add a source modification, add a patch on this
directory.

The script also adds a kernel config file from
`${GOPATH}/src/github.com/kata-containers/kata-containers/tools/packaging/kernel/configs/` to `.config`
in the kernel source code. You can modify it as needed.

## Build the kernel

After the kernel source code is ready, it is possible to build the kernel.

```bash
$ ./build-kernel.sh build
```

## Install the Kernel in the default path for Kata

Kata Containers uses some default path to search a kernel to boot. To install
on this path, the following command will install it to the default Kata
containers path (`/usr/share/kata-containers/`).

```bash
$ ./build-kernel.sh install
```

## Submit Kernel Changes

Kata Containers packaging repository holds the kernel configs and patches. The
config and patches can work for many versions, but we only test the
kernel version defined in the [Kata Containers versions file][kata-containers-versions-file].

For further details, see [the kernel configuration documentation](configs).

## How is it tested

The Kata Containers CI scripts install the kernel from [CI cache
job][cache-job] or build from sources.

If the kernel defined in the [Kata Containers versions file][kata-containers-versions-file] is
built and cached with the latest kernel config and patches, it installs.
Otherwise, the kernel is built from source.

The Kata kernel version is a mix of the kernel version defined in the [Kata Containers
versions file][kata-containers-versions-file] and the file `kata_config_version`. This
helps to identify if a kernel build has the latest recommend
configuration.

Example:

```bash
# From https://github.com/kata-containers/kata-containers/blob/main/versions.yaml
$ kernel_version_in_versions_file=5.4.60
# From https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kernel/kata_config_version
$ kata_config_version=83
$ latest_kernel_version=${kernel_version_in_versions_file}-${kata_config_version}
```

The resulting version is 5.4.60-83, this helps identify whether or not the kernel
configs are up-to-date on a CI version.

## Contribute

In order to do Kata Kernel changes. There are places to contribute:

1. [Kata Containers versions file][kata-containers-versions-file]: This file points to the
   recommended versions to be used by Kata. To update the kernel version send a
   pull request to update that version. The Kata CI will run all the use cases
   and verify it works.

1. Kata packaging repository. This repository contains all the kernel configs
   and patches recommended for Kata Containers kernel:

- If you want to upload one new configuration (new version or architecture
  specific) make sure the config file name has the following format:

  ```bash
  # Format:
  $ ${arch}_kata_${hypervisor_target}_${major_kernel_version}.x

  # example:
  $ arch=x86_64
  $ hypervisor_target=kvm
  $ major_kernel_version=4.19

  # Resulting file
  $ name: x86_64_kata_kvm_4.19.x
  ```

- Kernel patches, the CI and packaging scripts will apply all patches in the
  [patches directory][patches-dir].

Note: The kernel version and configuration file live in different locations,
which could result in a circular dependency on your (runtime or packaging) PR.
In this case, the PR you submit needs to be tested together with a patch from
another Kata Containers repository. To do this you have to specify which
repository and which pull request [it depends on][depends-on-docs].

[kata-containers-versions-file]: ../../../versions.yaml
[patches-dir]: patches
[depends-on-docs]: https://github.com/kata-containers/tests/blob/master/README.md#breaking-compatibility
[cache-job]: http://jenkins.katacontainers.io/job/image-nightly-x86_64/
