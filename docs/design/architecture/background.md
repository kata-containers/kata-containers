# Kata Containers architecture background knowledge

The following sections explain some of the background concepts
required to understand the [architecture document](README.md).

## Root filesystem

This document uses the term _rootfs_ to refer to a root filesystem
which is mounted as the top-level directory ("`/`") and often referred
to as _slash_.

It is important to understand this term since the overall system uses
multiple different rootfs's (as explained in the
[Environments](README.md#environments) section.

## Container image

In the [example command](example-command.md) the user has specified the
type of container they wish to run via the container image name:
`ubuntu`. This image name corresponds to a _container image_ that can
be used to create a container with an Ubuntu Linux environment. Hence,
in our [example](example-command.md), the `sh(1)` command will be run
inside a container which has an Ubuntu rootfs.

> **Note:**
>
> The term _container image_ is confusing since the image in question
> is **not** a container: it is simply a set of files (_an image_)
> that can be used to _create_ a container. The term _container
> template_ would be more accurate but the term _container image_ is
> commonly used so this document uses the standard term.

For the purposes of this document, the most important part of the
[example command line](example-command.md) is the container image the
user has requested. Normally, the container manager will _pull_
(download) a container image from a remote site and store a copy
locally. This local container image is used by the container manager
to create an [OCI bundle](#oci-bundle) which will form the environment
the container will run in. After creating the OCI bundle, the
container manager launches a [runtime](README.md#runtime) which will create the
container using the provided OCI bundle.

## OCI bundle

To understand what follows, it is important to know at a high level
how an OCI ([Open Containers Initiative](https://opencontainers.org)) compatible container is created.

An OCI compatible container is created by taking a
[container image](#container-image) and converting the embedded rootfs
into an
[OCI rootfs bundle](https://github.com/opencontainers/runtime-spec/blob/main/bundle.md),
or more simply, an _OCI bundle_.

An OCI bundle is a `tar(1)` archive normally created by a container
manager which is passed to an OCI [runtime](README.md#runtime) which converts
it into a full container rootfs. The bundle contains two assets:

- A container image [rootfs](#root-filesystem)

  This is simply a directory of files that will be used to represent
  the rootfs for the container.

  For the [example command](example-command.md), the directory will
  contain the files necessary to create a minimal Ubuntu root
  filesystem.

- An [OCI configuration file](https://github.com/opencontainers/runtime-spec/blob/main/config.md)

  This is a JSON file called `config.json`.

  The container manager will create this file so that:

  - The `root.path` value is set to the full path of the specified
    container rootfs.

    In [the example](example-command.md) this value will be `ubuntu`.

  - The `process.args` array specifies the list of commands the user
    wishes to run. This is known as the [workload](README.md#workload).

    In [the example](example-command.md) the workload is `sh(1)`.
