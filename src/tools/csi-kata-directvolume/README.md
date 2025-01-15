# CSI Direct Volume Driver

The `Direct Volume CSI driver` is heavily inspired by the [`K8s CSI HostPath driver`](https://github.com/kubernetes-csi/csi-driver-host-path). It aims to provide a production-ready implementation and a reference implementation for Kubernetes to connect to `Direct Volume`.

This repository houses the `Direct Volume CSI driver`, along with all build and dependent configuration files needed for deployment.

*WARNING* It is important to note that it is still under development.

## Pre-requisite

- K8S cluster
- Running version 1.20 or later
- Access to terminal with `kubectl` installed

## Features

The driver can provision volumes based on direct block devices, eliminating the need for loop devices and relying solely on single files stored on the host.

## Deployment

[Deployment for K8S 1.20+](docs/deploy-csi-kata-directvol.md)

## Building the Binary

If you want to build the driver yourself, you can do so with the following command from `csi-kata-directvolume` path:

```shell
cd tools/csi-kata-directvolume/ && make
```

## Building the Container Image

If you want to build the container image yourself, you can do so with the following command:

```shell
$ cd src/tools/csi-kata-directvolume
$ docker build -t localhost/kata-directvolume:v1.0.18 .
```
