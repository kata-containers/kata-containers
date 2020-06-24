# Installing with `kata-doc-to-script`

* [Introduction](#introduction)
* [Packages Installation](#packages-installation)
* [Docker Installation and Setup](#docker-installation-and-setup)

## Introduction
Use [these installation instructions](README.md#supported-distributions) together with
[`kata-doc-to-script`](https://github.com/kata-containers/tests/blob/master/.ci/kata-doc-to-script.sh)
to generate installation bash scripts.

> Note:
> - Only the Docker container manager installation can be scripted. For other setups you must
> install and configure the container manager manually.

## Packages Installation

```bash
$ source /etc/os-release
$ curl -fsSL -O https://raw.githubusercontent.com/kata-containers/documentation/master/install/${ID}-installation-guide.md
$ bash -c "$(curl -fsSL https://raw.githubusercontent.com/kata-containers/tests/master/.ci/kata-doc-to-script.sh) ${ID}-installation-guide.md ${ID}-install.sh"
```

For example, if your distribution is CentOS, the previous example will generate a runnable shell script called `centos-install.sh`.
To proceed with the installation, run:

```bash
$ source /etc/os-release
$ bash "./${ID}-install.sh"
```

## Docker Installation and Setup

```bash
$ source /etc/os-release
$ curl -fsSL -O https://raw.githubusercontent.com/kata-containers/documentation/master/install/docker/${ID}-docker-install.md
$ bash -c "$(curl -fsSL https://raw.githubusercontent.com/kata-containers/tests/master/.ci/kata-doc-to-script.sh) ${ID}-docker-install.md ${ID}-docker-install.sh"
```

For example, if your distribution is CentOS, this will generate a runnable shell script called `centos-docker-install.sh`.

To proceed with the Docker installation, run:

```bash
$ source /etc/os-release
$ bash "./${ID}-docker-install.sh"
```
