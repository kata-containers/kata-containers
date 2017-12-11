# Overview #

`Kata Containers runtime` creates a Virtual Machine to isolate a set of
container workloads. The Virtual Machine requires a operating system
operating (`Guest OS`) to boot and create containers inside the guest
environment.

This repository contains tools to create a `Guest OS` for `Kata
Containers`.

## Terms ##

This section describe the terms used as along all this document.

- `Guest OS`

 It is the collection of a `virtual disk` or `disk image` and `kernel`
 that in conjunction work as an operating system and it is different than
 the host operating system.

 - `Virtual disk` or `Guest Image`

 It is a virtual disk witch contains a `rootfs` that will be used to boot
 a Virtual Machine by for the `Kata Containers runtime`.

 - `rootfs`

  The root filesystem or rootfs is the filesystem that is contained in the
  guest root directory. It can be built from any Linux Distribution but
  must provide at least the following components:
	- Kata agent
	- A `init` system (for example `systemd`) witch allow to start
	  Kata agent at boot time.
