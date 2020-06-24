* [Introduction](#introduction)
* [Unsupported scenarios](#unsupported-scenarios)
* [Maintenance Warning](#maintenance-warning)
* [Upgrade from Clear Containers](#upgrade-from-clear-containers)
    * [Stop all running Clear Container instances](#stop-all-running-clear-container-instances)
    * [Configuration migration](#configuration-migration)
    * [Remove Clear Containers packages](#remove-clear-containers-packages)
        * [Fedora](#fedora)
        * [Ubuntu](#ubuntu)
    * [Disable old container manager configuration](#disable-old-container-manager-configuration)
    * [Install Kata Containers](#install-kata-containers)
    * [Create a Kata Container](#create-a-kata-container)
* [Upgrade from runV](#upgrade-from-runv)
* [Upgrade Kata Containers](#upgrade-kata-containers)
* [Appendices](#appendices)
    * [Assets](#assets)
        * [Guest kernel](#guest-kernel)
        * [Image](#image)
        * [Determining asset versions](#determining-asset-versions)

# Introduction

This document explains how to upgrade from
[Clear Containers](https://github.com/clearcontainers) and [runV](https://github.com/hyperhq/runv) to
[Kata Containers](https://github.com/kata-containers) and how to upgrade an existing
Kata Containers system to the latest version.

# Unsupported scenarios

Upgrading a Clear Containers system on the following distributions is **not**
supported since the installation process for these distributions makes use of
unpackaged components:

- [CentOS](https://github.com/clearcontainers/runtime/blob/master/docs/centos-installation-guide.md)
- [BCLinux](https://github.com/clearcontainers/runtime/blob/master/docs/bclinux-installation-guide.md)
- [RHEL](https://github.com/clearcontainers/runtime/blob/master/docs/rhel-installation-guide.md)
- [SLES](https://github.com/clearcontainers/runtime/blob/master/docs/sles-installation-guide.md)

Additionally, upgrading
[Clear Linux](https://github.com/clearcontainers/runtime/blob/master/docs/clearlinux-installation-guide.md)
is not supported as Kata Containers packages do not yet exist.

# Maintenance Warning

The Clear Containers codebase is no longer being developed. Only new releases
will be considered for significant bug fixes.

The main development focus is now on Kata Containers. All Clear Containers
users are encouraged to switch to Kata Containers.

# Upgrade from Clear Containers

Since Kata Containers can co-exist on the same system as Clear Containers, if
you already have Clear Containers installed, the upgrade process is simply to
install Kata Containers. However, since Clear Containers is
[no longer being actively developed](#maintenance-warning),
you are encouraged to remove Clear Containers from your systems.

## Stop all running Clear Container instances

Assuming a Docker\* system, to stop all currently running Clear Containers:

```
$ for container in $(sudo docker ps -q); do sudo docker stop $container; done
```

## Configuration migration

The automatic migration of
[Clear Containers configuration](https://github.com/clearcontainers/runtime#configuration) to
[Kata Containers configuration](https://github.com/kata-containers/runtime#configuration) is
not supported.

If you have made changes to your Clear Containers configuration, you should
review those changes and decide whether to manually apply those changes to the
Kata Containers configuration.

> **Note**: This step must be completed before continuing to
> [remove the Clear Containers packages](#remove-clear-containers-packages) since doing so will
> *delete the default Clear Containers configuration file from your system*.

## Remove Clear Containers packages

> **Warning**: If you have modified your
> [Clear Containers configuration](https://github.com/clearcontainers/runtime#configuration),
> you might want to make a safe copy of the configuration file before removing the
> packages since doing so will *delete the default configuration file*

### Fedora

```
$ sudo -E dnf remove cc-runtime\* cc-proxy\* cc-shim\* linux-container clear-containers-image qemu-lite cc-ksm-throttler
$ sudo rm /etc/yum.repos.d/home:clearcontainers:clear-containers-3.repo
```

### Ubuntu

```
$ sudo apt-get purge cc-runtime\* cc-proxy\* cc-shim\* linux-container clear-containers-image qemu-lite cc-ksm-throttler
$ sudo rm /etc/apt/sources.list.d/clear-containers.list
```

## Disable old container manager configuration

Assuming a Docker installation, remove the docker configuration for Clear
Containers:

```
$ sudo rm /etc/systemd/system/docker.service.d/clear-containers.conf
```

## Install Kata Containers

Follow one of the [installation guides](https://github.com/kata-containers/documentation/tree/master/install).

## Create a Kata Container

```
$ sudo docker run -ti busybox sh
```

# Upgrade from runV

runV and Kata Containers can run together on the same system without affecting each other, as long as they are
not configured to use the same container root storage. Currently, runV defaults to `/run/runv` and Kata Containers
defaults to `/var/run/kata-containers`.

Now, to upgrade from runV you need to fresh install Kata Containers by following one of
the [installation guides](https://github.com/kata-containers/documentation/tree/master/install).

# Upgrade Kata Containers

As shown in the
[installation instructions](https://github.com/kata-containers/documentation/blob/master/install),
Kata Containers provide binaries for popular distributions in their native
packaging formats. This allows Kata Containers to be upgraded using the
standard package management tools for your distribution.

# Appendices

## Assets

Kata Containers requires additional resources to create a virtual machine
container. These resources are called
[Kata Containers assets](./design/architecture.md#assets),
which comprise a guest kernel and a root filesystem or initrd image. This
section describes when these components are updated.

Since the official assets are packaged, they are automatically upgraded when
new package versions are published.

> **Warning**: Note that if you use custom assets (by modifying the
> [Kata Runtime configuration > file](https://github.com/kata-containers/runtime/#configuration)),
> it is your responsibility to ensure they are updated as necessary.

### Guest kernel

The `kata-linux-container` package contains a Linux\* kernel based on the
latest vanilla version of the
[long-term kernel](https://www.kernel.org/)
plus a small number of
[patches](https://github.com/kata-containers/packaging/tree/master/kernel).

The `Longterm` branch is only updated with
[important bug fixes](https://www.kernel.org/category/releases.html)
meaning this package is only updated when necessary.

The guest kernel package is updated when a new long-term kernel is released
and when any patch updates are required.

### Image

The `kata-containers-image` package is updated only when critical updates are
available for the packages used to create it, such as:

- systemd
- [Kata Containers Agent](https://github.com/kata-containers/agent)

### Determining asset versions

To see which versions of the assets being used:

```
$ kata-runtime kata-env
```
