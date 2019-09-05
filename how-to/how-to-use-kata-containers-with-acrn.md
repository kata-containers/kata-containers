# Kata Containers with ACRN

This document provides an overview on how to run Kata containers with ACRN hypervisor and device model.

- [Introduction](#introduction)
- [Pre-requisites](#pre-requisites)
- [Install and Configure Docker](#install-and-configure-docker)
- [Configure Kata Containers with ACRN](#configure-kata-containers-with-acrn)

## Introduction

ACRN is a flexible, lightweight Type-1 reference hypervisor built with real-time and safety-criticality in mind. ACRN uses an open source platform making it optimized to streamline embedded development.

Some of the key features being:

- Small footprint - Approx. 25K lines of code (LOC).
- Real Time - Low latency, faster boot time, improves overall responsiveness with hardware.
- Adaptability - Multi-OS support for guest operating systems like Linux, Android, RTOSes.
- Rich I/O mediators - Allows sharing of various I/O devices across VMs.
- Optimized for a variety of IoT (Internet of Things) and embedded device solutions.

Please refer to ACRN [documentation](https://projectacrn.github.io/latest/index.html) for more details on ACRN hypervisor and device model.

## Pre-requisites

This document requires the presence of the ACRN hypervisor and Kata Containers on your system. Install using the instructions available through the following links:

- ACRN supported [Hardware](https://projectacrn.github.io/latest/hardware.html#supported-hardware).
- ACRN [software](https://projectacrn.github.io/latest/getting-started/apl-nuc.html#use-the-script-to-set-up-acrn-automatically) setup.
- Kata Containers installation: Automated installation does not seem to be supported for Clear Linux, so please use [manual installation](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md) steps.

> **Note:** Create rootfs image and not initrd image.

In order to run Kata with ACRN, your container stack must provide block-based storage, such as device-mapper.

> **Note:** Currently, you can only launch one VM from Kata Containers using ACRN hypervisor (SDC scenario) due to [this issue](https://github.com/kata-containers/runtime/issues/1785).

## Install and Configure Docker

Install Docker 18.06 (as Docker 18.09 does not support device-mapper). To configure Docker for device-mapper and Kata,

1. Stop Docker daemon if it is already running.

```bash
$ sudo systemctl stop docker
```

2. Set `/etc/docker/daemon.json` with the following contents.

```
{
  "storage-driver": "devicemapper"
}
```

3. Restart docker.

```bash
$ sudo systemctl daemon-reload
$ sudo systemctl restart docker
```

4. Configure [Docker](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md#update-the-docker-systemd-unit-file) to use `kata-runtime`.

## Configure Kata Containers with ACRN

To configure Kata Containers with ACRN, copy the generated `configuration-acrn.toml` file when building the `kata-runtime` to either `/etc/kata-containers/configuration.toml` or `/usr/share/defaults/kata-containers/configuration.toml`.

The following command shows full paths to the `configuration.toml` files that the runtime loads. It will use the first path that exists. (Please make sure the kernel and image paths are set correctly in the `configuration.toml` file)

```bash
$ sudo kata-runtime --kata-show-default-config-paths
```

>**Warning:** Please offline CPUs using [this](offline_cpu.sh) script, else VM launches will fail.

```bash
$ sudo ./offline_cpu.sh
```

Start an ACRN based Kata Container,

```bash
$ sudo docker run -ti --runtime=kata-runtime busybox sh
```

You will see ACRN(`acrn-dm`) is now running on your system, as well as a `kata-shim`, `kata-proxy`. You should obtain an interactive shell prompt. Verify that all the Kata processes terminate once you exit the container.

```bash
$ ps -ef | grep -E "kata|acrn"
```

Validate ACRN hypervisor by using `kata-runtime kata-env`,

```sh
$ kata-runtime kata-env | awk -v RS= '/\[Hypervisor\]/'
[Hypervisor]
  MachineType = ""
  Version = "DM version is: 1.2-unstable-254577a6-dirty (daily tag:acrn-2019w27.4-140000p)
  Path = "/usr/bin/acrn-dm"
  BlockDeviceDriver = "virtio-blk"
  EntropySource = "/dev/urandom"
  Msize9p = 0
  MemorySlots = 10
  Debug = false
  UseVSock = false
  SharedFS = ""
```
