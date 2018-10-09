[![Build Status](https://travis-ci.org/kata-containers/osbuilder.svg?branch=master)](https://travis-ci.org/kata-containers/osbuilder)

# osbuilder

* [Introduction](#introduction)
* [Terms](#terms)
* [Usage](#usage)
    * [Rootfs creation](#rootfs-creation)
        * [Rootfs with systemd as init](#rootfs-with-systemd-as-init)
        * [Rootfs with the agent as init](#rootfs-with-the-agent-as-init)
    * [Image creation](#image-creation)
        * [Image with systemd as init](#image-with-systemd-as-init)
        * [Image with the agent as init](#image-with-the-agent-as-init)
    * [Initrd creation](#initrd-creation)
    * [Tests](#tests)
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

## Usage

The top-level `Makefile` contains an example of how to use the available components.

By default, components will run on the host system. However, some components
offer the ability to run from within Docker (for ease of setup) by setting the
`USE_DOCKER=true` variable.

For more detailed information, consult the documentation for a particular component.

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

### Initrd creation

To create an initrd from the already-created rootfs with the agent acting as the init daemon:

```
$ sudo -E PATH=$PATH make AGENT_INIT=yes initrd
```

For further details,
see [the initrd builder documentation](initrd-builder/README.md).

### Tests

```
$ make test
```

For further details, see [the tests documentation](tests/README.md).

## Platform-Distro Compatibility Matrix

|           |Alpine            |CentOS            |ClearLinux        |Debian/Ubuntu     |EulerOS           |Fedora            |openSUSE          |
|--         |--                |--                |--                |--                |--                |--                |--                |
|**ARM64**  |:heavy_check_mark:|:heavy_check_mark:|                  |                  |:heavy_check_mark:|:heavy_check_mark:|                  |
|**PPC64le**|:heavy_check_mark:|:heavy_check_mark:|                  |:heavy_check_mark:|:heavy_check_mark:|:heavy_check_mark:|:heavy_check_mark:|
|**x86_64** |:heavy_check_mark:|:heavy_check_mark:|:heavy_check_mark:|:heavy_check_mark:|:heavy_check_mark:|:heavy_check_mark:|:heavy_check_mark:|
