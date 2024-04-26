# Kata Containers installation guides

The following is an overview of the different installation methods available. 

## Prerequisites

Kata Containers requires nested virtualization or bare metal. Check 
[hardware requirements](./../../README.md#hardware-requirements) to see if your system is capable of running Kata 
Containers.

## Packaged installation methods

The packaged installation method uses your distribution's native package format (such as RPM or DEB).

> **Note:**
>
> We encourage you to select an installation method that provides
> automatic updates, to ensure you get the latest security updates and
> bug fixes.

| Installation method                                  | Description                                                                                  | Automatic updates | Use case                                                                                      |
|------------------------------------------------------|----------------------------------------------------------------------------------------------|-------------------|-----------------------------------------------------------------------------------------------|
| [Using official distro packages](#official-packages) | Kata packages provided by Linux distributions official repositories                          | yes               | Recommended for most users.                                                                   |
| [Automatic](#automatic-installation)                 | Run a single command to install a full system                                                | **No!**           | For those wanting the latest release quickly.                                                 |
| [Using kata-deploy](#kata-deploy-installation)       | The preferred way to deploy the Kata Containers distributed binaries on a Kubernetes cluster | **No!**           | Best way to give it a try on kata-containers on an already up and running Kubernetes cluster. |

### Kata Deploy Installation

Kata Deploy provides a Dockerfile, which contains all of the binaries and
artifacts required to run Kata Containers, as well as reference DaemonSets,
which can be utilized to install Kata Containers on a running Kubernetes
cluster.

[Use Kata Deploy](/tools/packaging/kata-deploy/README.md) to install Kata Containers on a Kubernetes Cluster.

### Official packages

Kata packages are provided by official distribution repositories for:

| Distribution (link to installation guide)                | Minimum versions                                                               |
|----------------------------------------------------------|--------------------------------------------------------------------------------|
| [CentOS](centos-installation-guide.md)                   | 8                                                                              |
| [Fedora](fedora-installation-guide.md)                   | 34                                                                             |

### Automatic Installation

[Use `kata-manager`](/utils/README.md) to automatically install a working Kata Containers system.

## Installing on a Cloud Service Platform

* [Amazon Web Services (AWS)](aws-installation-guide.md)
* [Google Compute Engine (GCE)](gce-installation-guide.md)
* [Microsoft Azure](azure-installation-guide.md)
* [Minikube](minikube-installation-guide.md)
* [VEXXHOST OpenStack Cloud](vexxhost-installation-guide.md)

## Further information

* [upgrading document](../Upgrading.md)
* [developer guide](../Developer-Guide.md)
* [runtime documentation](../../src/runtime/README.md)

## Kata Containers 3.0 rust runtime installation
* [installation guide](../install/kata-containers-3.0-rust-runtime-installation-guide.md)
