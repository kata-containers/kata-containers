* [Overview](#overview)
* [Terms](#terms)

# Overview

The Kata Containers runtime creates a virtual machine (VM) to isolate a set of
container workloads. The VM requires a guest kernel and a guest operating system
("guest OS") to boot and create containers inside the guest
environment.

This repository contains tools to create a guest OS disk image.

# Terms

This section describes the terms used for all documentation in this repository.

- rootfs

  The root filesystem or "rootfs" is the set of files contained in the
  guest root directory that builds into a filesystem.

  See [the rootfs builder documentation](rootfs-builder/README.md).

- "Guest OS" (or "Guest Image")

  A "virtual disk" or "disk image" built from a rootfs. It contains a
  filesystem that is used by the VM, in conjunction with a guest kernel, to
  create an environment to host the container. Neither the guest OS nor the
  guest kernel need to be the same as the host operating system.

  See [the image builder documentation](image-builder/README.md).

- initrd (or "initramfs")

  A compressed cpio archive loaded into memory and used as part of the Linux
  startup process. During startup, the kernel unpacks it into a special
  instance of a tmpfs that becomes the initial root file system.

  See [the initrd builder documentation](initrd-builder/README.md).

- "Base OS"

  A particular version of a Linux distribution used to create a Guest OS from.
