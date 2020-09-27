# Kata Containers snap image

* [Initial setup](#initial-setup)
* [Install snap](#install-snap)
* [Build and install snap image](#build-and-install-snap-image)
* [Configure Kata Containers](#configure-kata-containers)
* [Integration with docker and Kubernetes](#integration-with-docker-and-kubernetes)
* [Remove snap](#remove-snap)
* [Limitations](#limitations)

This directory contains the resources needed to build the Kata Containers
[snap][1] image.

## Initial setup

Kata Containers can be installed in any Linux distribution that supports
[snapd](https://docs.snapcraft.io/installing-snapd). For this example, we
assume Ubuntu as your base distro.
```sh
$ sudo apt-get --no-install-recommends install -y apt-utils ca-certificates snapd snapcraft
```

## Install snap

You can install the Kata Containers snap from the [snapcraft store][8] or by running the following command:

```sh
$ sudo snap install kata-containers --classic
```

## Build and install snap image

Run next command at the root directory of the packaging repository.

```sh
$ make snap
```

To install the resulting snap image, snap must be put in [classic mode][3] and the
security confinement must be disabled (*--classic*). Also since the resulting snap
has not been signed the verification of signature must be omitted (*--dangerous*).

```sh
$ sudo snap install --classic --dangerous kata-containers_[VERSION]_[ARCH].snap
```

Replace `VERSION` with the current version of Kata Containers and `ARCH` with
the system architecture.

## Configure Kata Containers

By default Kata Containers snap image is mounted at `/snap/kata-containers` as a
read-only file system, therefore default configuration file can not be edited.
Fortunately [`kata-runtime`][4] supports loading a configuration file from another
path than the default.

```sh
$ sudo mkdir -p /etc/kata-containers
$ sudo cp /snap/kata-containers/current/usr/share/defaults/kata-containers/configuration.toml /etc/kata-containers/
$ $EDITOR /etc/kata-containers/configuration.toml
```

## Integration with docker and Kubernetes

The path to the runtime provided by the Kata Containers snap image is
`/snap/kata-containers/current/usr/bin/kata-runtime`. You should use it to
run Kata Containers with [docker][9] and [Kubernetes][10].

## Remove snap

You can remove the Kata Containers snap by running the following command:

```sh
$ sudo snap remove kata-containers
```

## Limitations

The [miniOS image][2] is not included in the snap image as it is not possible for
QEMU to open a guest RAM backing store on a read-only filesystem. Fortunately,
you can start Kata Containers with a Linux initial RAM disk (initrd) that is
included in the snap image. If you want to use the miniOS image instead of initrd,
then a new configuration file can be [created](#configure-kata-containers)
and [configured][7].

[1]: https://docs.snapcraft.io/snaps/intro
[2]: ../docs/design/architecture.md#root-filesystem-image
[3]: https://docs.snapcraft.io/reference/confinement#classic
[4]: https://github.com/kata-containers/runtime#configuration
[5]: https://docs.docker.com/engine/reference/commandline/dockerd
[6]: ../docs/install/docker/ubuntu-docker-install.md
[7]: ../docs/Developer-Guide.md#configure-to-use-initrd-or-rootfs-image
[8]: https://snapcraft.io/kata-containers
[9]: ../docs/Developer-Guide.md#run-kata-containers-with-docker
[10]: ../docs/Developer-Guide.md#run-kata-containers-with-kubernetes
