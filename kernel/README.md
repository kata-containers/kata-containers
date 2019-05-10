* [Build Kata Containers Kernel](#build-kata-containers-kernel)
    * [Requirements](#requirements)
    * [Setup kernel source code](#setup-kernel-source-code)
* [Build the kernel](#build-the-kernel)
    * [Install the Kernel in the default path for Kata](#install-the-kernel-in-the-default-path-for-kata)
    * [Submit Kernel Changes](#submit-kernel-changes)
    * [How is it tested](#how-is-it-tested)
* [Contribute](#contribute)

---

# Build Kata Containers Kernel

This document explains the steps to build a kernel recommended for use with
Kata Containers. To do this use `build-kernel.sh`, this script
automates the process to build a kernel for Kata Containers.

## Requirements

The `build-kernel.sh` script requires an installed Golang version matching the
[component build requirements](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md#requirements-to-build-individual-components).

## Setup kernel source code

```bash
$ ./build-kernel.sh setup
```

The script `./build-kernel.sh` tries to apply the patches from
`${GOPATH}/src/github.com/kata-containers/packaging/kernel/patches/` when it
sets up a kernel. If you want to add a source modification, add a patch on this
directory.

The script also adds a kernel config file from
`${GOPATH}/src/github.com/kata-containers/packaging/kernel/configs/` to `.config`
in the kernel source code. You can modify it as needed.

# Build the kernel

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
kernel version defined in the [runtime versions file][runtime-versions-file].

## How is it tested

The Kata Containers CI scripts install the kernel from [CI cache
job][cache-job] or build from sources.

If the kernel defined in the [runtime versions file][runtime-versions-file] is
built and cached with the latest kernel config and patches, it installs.
Otherwise, the kernel is built from source.

The Kata kernel version is a mix of the kernel version defined in the [runtime
versions file][runtime-versions-file] and the file `kata_config_version`. This
helps to identify if a kernel build has the latest recommend
configuration.

Example:

```bash
# From https://github.com/kata-containers/runtime/blob/master/versions.yaml
$ kernel_version_in_versions_file=4.10.1
# From https://github.com/kata-containers/packaging/blob/master/kernel/kata_config_version
$ kata_config_version=25
$ latest_kernel_version=${kernel_version_in_versions_file}-${kata_config_version}
```

The resulting version is 4.10.1-25, this helps identify whether or not the kernel
configs are up-to-date on a CI version.

# Contribute

In order to do Kata Kernel changes. There are places to contribute:

1. [Kata runtime versions file][runtime-versions-file]: This file points to the
   recommended to use by Kata. To update the kernel version send a pull request
   to update that version. The Kata CI will run all the use cases and verify it
   works.

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
  $ major_kernel_versio=4.14

  # Resulting file
  $ name: x86_64_kata_kvm_4.19.x
  ```

- Kernel patches, the CI and packaging scripts will apply all patches in the
  [patches directory][patches-dir].

Note: The kernel version and configuration file live in different locations,
which could result in a circular dependency on your (runtime or packaging) PR.
In this case, the PR you submit needs to be tested together with a patch from
another kata-containers repository. To do this you have to specify which
repository and which pull request [it depends on][depends-on-docs].

[runtime-versions-file]: https://github.com/kata-containers/runtime/blob/master/versions.yaml
[patches-dir]: https://github.com/kata-containers/packaging/tree/master/kernel/patches
[depends-on-docs]: https://github.com/kata-containers/tests/blob/master/README.md#breaking-compatibility
[cache-job]: http://jenkins.katacontainers.io/job/image-nightly-x86_64/
