[![Build Status](https://travis-ci.org/kata-containers/osbuilder.svg?branch=master)](https://travis-ci.org/kata-containers/osbuilder)

# osbuilder

* [osbuilder](#osbuilder)
    * [Introduction](#introduction)
    * [Terms](#terms)
    * [Building](#building)
        * [Rootfs creation](#rootfs-creation)
            * [Rootfs with systemd as init](#rootfs-with-systemd-as-init)
            * [Rootfs with the agent as init](#rootfs-with-the-agent-as-init)
            * [dracut based rootfs](#dracut-based-rootfs)
        * [Image creation](#image-creation)
            * [Image with systemd as init](#image-with-systemd-as-init)
            * [Image with the agent as init](#image-with-the-agent-as-init)
            * [dracut based image](#dracut-based-image)
        * [Initrd creation](#initrd-creation)
            * [Rootfs based initrd](#rootfs-based-initrd)
            * [dracut based initrd](#dracut-based-initrd)
        * [dracut options](#dracut-options)
            * [Add kernel modules](#add-kernel-modules)
        * [Custom images](#custom-images)
            * [Intel® QuickAssist Technology (QAT) customized kernel and rootfs](#intel-quickassist-technology-qat-customized-kernel-and-rootfs)
    * [Testing](#testing)
    * [Platform-Distro Compatibility Matrix](#platform-distro-compatibility-matrix)

## Introduction

The Kata Containers runtime creates a virtual machine (VM) to isolate a set of
container workloads. The VM requires a guest kernel and a guest operating system
("guest OS") to boot and create containers inside the guest
environment.

This repository contains tools to create a guest OS disk image.

## Terms

This section describes the terms used for all documentation in this repository.

- rootfs

  The root filesystem or "rootfs" is a slight misnomer as it is not a true filesystem. It is a tree of files contained in a particular directory, which represents the root disk layout. A rootfs can be turned into either an image or an initrd.

  See the [rootfs creation](#rootfs-creation) section.

- "Guest OS" (or "Guest Image")

  A "virtual disk" or "disk image" built from a rootfs. It contains a
  filesystem that is used by the VM, in conjunction with a guest kernel, to
  create an environment to host the container. Neither the guest OS nor the
  guest kernel need to be the same as the host operating system.

  See the [image creation](#image-creation) section.

- initrd (or "initramfs")

  A compressed `cpio(1)` archive, created from a rootfs which is loaded into memory and used as part of the Linux startup process. During startup, the kernel unpacks it into a special instance of a `tmpfs` that becomes the initial root filesystem.

  See the [initrd creation](#initrd-creation) section.

- "Base OS"

  A particular version of a Linux distribution used to create a rootfs from.

- dracut

  A guest OS build method where the building host is used as the Base OS.
  For more information refer to the [dracut homepage](https://dracut.wiki.kernel.org/index.php/Main_Page).

- Agent init

  The Guest OS should have the Kata Containers agent started on boot time.

  That is achieved by using a system manager (for example, systemd) which
  will evoke the agent binary; or having the agent itself as the init process.

## Building

The top-level `Makefile` contains an example of how to use the available components.
Set `DEBUG=true` to execute build scripts in debug mode.

Two build methods are available, `distro` and `dracut`.
By default, the `distro` build method is used, and this creates a rootfs using
distro specific commands (e.g.: `debootstrap` for Debian or `yum` for CentOS).
The `dracut` build method uses the distro-agnostic tool `dracut` to obtain the same goal.

By default components are run on the host system. However, some components
offer the ability to run from within a container (for ease of setup) by setting the
`USE_DOCKER=true` or `USE_PODMAN=true` variable. If both are set, `USE_DOCKER=true`
takes precedence over `USE_PODMAN=true`.

For more detailed information, consult the documentation for a particular component.

When invoking the appropriate make target as showed below, a single command is used
to generate an initrd or an image. This is what happens in details:
1. A rootfs is generated based on the specified target distribution.
2. The rootfs is provisioned with Kata-specific components and configuration files.
3. The rootfs is used as a base to generate an initrd or an image.

When using the dracut build method however, the build sequence is different:
1. An overlay directory is populated with Kata-specific components.
2. dracut is instructed to merge the overlay directory with the required host-side
filesystem components to generate an initrd.
3. When generating an image, the initrd is extracted to obtain the base rootfs for
the image.

CentOS is the default distro for building the rootfs, to use a different one, you can set `DISTRO=<your_distro>`.
For example `make USE_DOCKER=true DISTRO=ubuntu rootfs` will make Ubuntu rootfs using Docker.

### Rootfs creation

This section shows how to build a basic rootfs using the default distribution.
For further details, see
[the rootfs builder documentation](rootfs-builder/README.md).

#### Rootfs with systemd as init

```
$ sudo -E PATH=$PATH make USE_DOCKER=true rootfs
```

#### Rootfs with the agent as init

```
$ sudo -E PATH=$PATH make USE_DOCKER=true AGENT_INIT=yes rootfs
```

#### dracut based rootfs

> **Note**: the dracut build method does not need a rootfs as a base for an image or initrd.
However, a rootfs can be generated by extracting the generated initrd.

```
$ sudo -E PATH=$PATH make BUILD_METHOD=dracut rootfs
```

### Image creation

This section shows how to create an image from the already-created rootfs. For
further details, see
[the image builder documentation](image-builder/README.md).

#### Image with systemd as init

```
$ sudo -E PATH=$PATH make USE_DOCKER=true image
```

#### Image with the agent as init

```
$ sudo -E PATH=$PATH make USE_DOCKER=true AGENT_INIT=yes image
```

#### dracut based image

> Note: the dracut build method generates an image by first building an initrd,
and then using the rootfs extracted from it.

```
$ sudo -E PATH=$PATH make BUILD_METHOD=dracut image
```

### Initrd creation

#### Rootfs based initrd

Create an initrd from the already-created rootfs and with the agent acting as the init daemon
using:

```
$ sudo -E PATH=$PATH make AGENT_INIT=yes initrd
```

#### dracut based initrd

Create an initrd using the dracut build method with:

```
$ sudo -E PATH=$PATH make BUILD_METHOD=dracut AGENT_INIT=yes initrd
```

For further details,
see [the initrd builder documentation](initrd-builder/README.md).

### dracut options

#### Add kernel modules

If the initrd or image needs to contain kernel modules, this can be done by:

1. Specify the name of the modules (as reported by `modinfo MODULE-NAME`) in
`dracut/dracut.conf.d/10-drivers.conf`. For example this file can contain:
```
drivers="9p 9pnet 9pnet_virtio"
```
2. Set the `DRACUT_KVERSION` make variable to the release name of the kernel that
is paired with the built image or initrd, using the `uname -r` format. For example:
```
$ make BUILD_METHOD=dracut DRACUT_KVERSION=5.2.1-23-kata AGENT_INIT=yes initrd
```

### Custom images

The Kata Containers kernel and rootfs images are by design "minimal". If advanced, 
site specific, or customized features are required, then building a customized 
kernel and/or rootfs may be required.

The below are some examples which may help or be useful for generating a 
customized system.

#### Intel® QuickAssist Technology (QAT) customized kernel and rootfs

As documented in the
[Intel® QAT Kata use-case documentation](../../docs/use-cases/using-Intel-QAT-and-kata.md),
enabling this hardware requires a customized kernel and rootfs to work with Kata. 
To ease building of the kernel and rootfs, a [Dockerfile](./dockerfiles/QAT) is 
supplied, that when run, generates the required kernel and rootfs binaries.

## Testing

```
$ make test
```

For further details, see [the tests documentation](tests/README.md).

## Platform-Distro Compatibility Matrix

The following table illustrates what target architecture is supported for each
of the the osbuilder distributions.

> Note: this table is not relevant for the dracut build method, since it supports
any Linux distribution and architecture where dracut is available.

|           |Alpine            |CentOS            |Clear Linux       |Debian/Ubuntu     |Fedora            |openSUSE          |
|--         |--                |--                |--                |--                |--                |--                |
|**ARM64**  |:heavy_check_mark:|:heavy_check_mark:|                  |                  |:heavy_check_mark:|:heavy_check_mark:|
|**PPC64le**|:heavy_check_mark:|:heavy_check_mark:|                  |:heavy_check_mark:|:heavy_check_mark:|:heavy_check_mark:|
|**s390x**  |:heavy_check_mark:|                  |                  |:heavy_check_mark:|:heavy_check_mark:|                  |
|**x86_64** |:heavy_check_mark:|:heavy_check_mark:|:heavy_check_mark:|:heavy_check_mark:|:heavy_check_mark:|:heavy_check_mark:|
