# Kata Containers installation user guides

* [Kata Containers installation user guides](#kata-containers-installation-user-guides)
    * [Prerequisites](#prerequisites)
    * [Legacy installation](#legacy-installation)
    * [Packaged installation methods](#packaged-installation-methods)
        * [Official packages](#official-packages)
        * [Snap Installation](#snap-installation)
        * [Automatic Installation](#automatic-installation)
        * [Manual Installation](#manual-installation)
    * [Build from source installation](#build-from-source-installation)
    * [Installing on a Cloud Service Platform](#installing-on-a-cloud-service-platform)
    * [Further information](#further-information)

The following is an overview of the different installation methods available. All of these methods equally result
in a system configured to run Kata Containers.

## Prerequisites

Kata Containers requires nested virtualization or bare metal.
See the
[hardware requirements](/src/runtime/README.md#hardware-requirements)
to see if your system is capable of running Kata Containers.

## Legacy installation

If you wish to install a legacy 1.x version of Kata Containers, see
[the Kata Containers 1.x installation documentation](https://github.com/kata-containers/documentation/tree/master/install/).

## Packaged installation methods

> **Notes:**
>
> - Packaged installation methods uses your distribution's native package format (such as RPM or DEB).
> - You are strongly encouraged to choose an installation method that provides
>   automatic updates, to ensure you benefit from security updates and bug fixes.

| Installation method                                  | Description                                                         | Automatic updates | Use case                                                 |
|------------------------------------------------------|---------------------------------------------------------------------|-------------------|----------------------------------------------------------|
| [Using official distro packages](#official-packages) | Kata packages provided by Linux distributions official repositories | yes               | Recommended for most users.                              |
| [Using snap](#snap-installation)                     | Easy to install                                                     | yes               | Good alternative to official distro packages.            |
| [Automatic](#automatic-installation)                 | Run a single command to install a full system                       | **No!**           | For those wanting the latest release quickly.            |
| [Manual](#manual-installation)                       | Follow a guide step-by-step to install a working system             | **No!**           | For those who want the latest release with more control. |
| [Build from source](#build-from-source-installation) | Build the software components manually                              | **No!**           | Power users and developers only.                         |

### Official packages

Kata packages are provided by official distribution repositories for:

| Distribution (link to installation guide)                | Minimum versions                                                               |
|----------------------------------------------------------|--------------------------------------------------------------------------------|
| [CentOS](centos-installation-guide.md)                   | 8                                                                              |
| [Fedora](fedora-installation-guide.md)                   | 34                                                                             |

> **Note::**
>
> All users are encouraged to uses the official distribution versions of Kata
> Containers unless they understand the implications of alternative methods.

### Snap Installation

> **Note:** The snap installation is available for all distributions which support `snapd`.

[![Get it from the Snap Store](https://snapcraft.io/static/images/badges/en/snap-store-black.svg)](https://snapcraft.io/kata-containers)

[Use snap](snap-installation-guide.md) to install Kata Containers from https://snapcraft.io.

### Automatic Installation

[Use `kata-manager`](/utils/README.md) to automatically install a working Kata Containers system.

### Manual Installation

Follow the [containerd installation guide](container-manager/containerd/containerd-install.md).

## Build from source installation

> **Notes:**
>
> - Power users who decide to build from sources should be aware of the
>   implications of using an unpackaged system which will not be automatically
>   updated as new [releases](../Stable-Branch-Strategy.md) are made available.

[Building from sources](../Developer-Guide.md#initial-setup)  allows power users
who are comfortable building software from source to use the latest component
versions. This is not recommended for normal users.

## Installing on a Cloud Service Platform

* [Amazon Web Services (AWS)](aws-installation-guide.md)
* [Google Compute Engine (GCE)](gce-installation-guide.md)
* [Microsoft Azure](azure-installation-guide.md)
* [Minikube](minikube-installation-guide.md)
* [VEXXHOST OpenStack Cloud](vexxhost-installation-guide.md)

## Further information

* The [upgrading document](../Upgrading.md).
* The [developer guide](../Developer-Guide.md).
* The [runtime documentation](../../src/runtime/README.md).
