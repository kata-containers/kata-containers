# Kata Containers installation user guides

* [Prerequisites](#prerequisites)
* [Packaged installation methods](#packaged-installation-methods)
   * [Supported Distributions](#supported-distributions)
      * [Official packages](#official-packages)
   * [Automatic Installation](#automatic-installation)
   * [Snap Installation](#snap-installation)
   * [Scripted Installation](#scripted-installation)
   * [Manual Installation](#manual-installation)
* [Build from source installation](#build-from-source-installation)
* [Installing on a Cloud Service Platform](#installing-on-a-cloud-service-platform)
* [Further information](#further-information)

The following is an overview of the different installation methods available. All of these methods equally result
in a system configured to run Kata Containers.

## Prerequisites
Kata Containers requires nested virtualization or bare metal.
See the
[hardware requirements](https://github.com/kata-containers/runtime/blob/master/README.md#hardware-requirements)
to see if your system is capable of running Kata Containers.

## Packaged installation methods

> **Notes:**
>
> - Packaged installation methods uses your distribution's native package format (such as RPM or DEB).

| Installation method                                  | Description                                                                             | Distributions supported              |
|------------------------------------------------------|-----------------------------------------------------------------------------------------|--------------------------------------|
| [Automatic](#automatic-installation)                 |Run a single command to install a full system                                            |[see table](#supported-distributions) |
| [Using snap](#snap-installation)                     |Easy to install and automatic updates                                                    |any distro that supports snapd        |
| [Using official distro packages](#official-packages) |Kata packages provided by Linux distributions official repositories                      |[see table](#supported-distributions) |
| [Scripted](#scripted-installation)                   |Generates an installation script which will result in a working system when executed     |[see table](#supported-distributions) |
| [Manual](#manual-installation)                       |Allows the user to read a brief document and execute the specified commands step-by-step |[see table](#supported-distributions) |

### Supported Distributions

Kata is packaged by the Kata community for:

|Distribution (link to installation guide)                        | Versions                                                                                                          |
|-----------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|
|[CentOS](centos-installation-guide.md)                           | 7                                                                                                                 |
|[Debian](debian-installation-guide.md)                           | 9                                                                                                                 |
|[Fedora](fedora-installation-guide.md)                           | 28, 29, 30                                                                                                        |
|[openSUSE](opensuse-installation-guide.md)                       | [Leap](opensuse-leap-installation-guide.md) (15, 15.1)<br>[Tumbleweed](opensuse-tumbleweed-installation-guide.md) |
|[Red Hat Enterprise Linux (RHEL)](rhel-installation-guide.md)    | 7                                                                                                                 |
|[SUSE Linux Enterprise Server (SLES)](sles-installation-guide.md)| SLES 12 SP3                                                                                                       |
|[Ubuntu](ubuntu-installation-guide.md)                           | 16.04, 18.04                                                                                                      |

#### Official packages

Kata packages are provided by official distribution repositories for:

|Distribution (link to packages)                                  | Versions   |
|-----------------------------------------------------------------|------------|
|[openSUSE](https://software.opensuse.org/package/katacontainers) | Tumbleweed |


### Automatic Installation

[Use `kata-manager`](installing-with-kata-manager.md) to automatically install Kata packages.

### Snap Installation

[![Get it from the Snap Store](https://snapcraft.io/static/images/badges/en/snap-store-black.svg)](https://snapcraft.io/kata-containers)

[Use snap](snap-installation-guide.md) to install Kata Containers from https://snapcraft.io.

### Scripted Installation
[Use `kata-doc-to-script`](installing-with-kata-doc-to-script.md) to generate installation scripts that can be reviewed before they are executed.

### Manual Installation
Manual installation instructions are available for [these distributions](#supported-distributions) and document how to:
1. Add the Kata Containers repository to your distro package manager, and import the packages signing key.
2. Install the Kata Containers packages.
3. Install a supported container manager.
4. Configure the container manager to use `kata-runtime` as the default OCI runtime. Or, for Kata Containers 1.5.0 or above, configure the
   `io.containerd.kata.v2` to be the runtime shim (see [containerd runtime v2 (shim API)](https://github.com/containerd/containerd/tree/master/runtime/v2)
   and [How to use Kata Containers and CRI (containerd plugin) with Kubernetes](https://github.com/kata-containers/documentation/blob/master/how-to/how-to-use-k8s-with-cri-containerd-and-kata.md)).

> **Notes on upgrading**:
> - If you are installing Kata Containers on a system that already has Clear Containers or `runv` installed,
>  first read [the upgrading document](../Upgrading.md).

> **Notes on releases**:
> - [This download server](http://download.opensuse.org/repositories/home:/katacontainers:/releases:/)
> hosts the Kata Containers packages built by OBS for all the supported architectures.
> Packages are available for the latest and stable releases (more info [here](https://github.com/kata-containers/documentation/blob/master/Stable-Branch-Strategy.md)).
>
> - The following guides apply to the latest Kata Containers release
> (a.k.a. `master` release).
>
> - When choosing a stable release, replace all `master` occurrences in the URLs
> with a `stable-x.y` version available on the [download server](http://download.opensuse.org/repositories/home:/katacontainers:/releases:/).

> **Notes on packages source verification**:
> - The Kata packages hosted on the download server are signed with GPG to ensure integrity and authenticity.
>
> - The public key used to sign packages is available [at this link](https://raw.githubusercontent.com/kata-containers/tests/master/data/rpm-signkey.pub); the fingerprint is `9FDC0CB6 3708CF80 3696E2DC D0B37B82 6063F3ED`.
>
> - Only trust the signing key and fingerprint listed in the previous bullet point. Do not disable GPG checks,
> otherwise packages source and authenticity is not guaranteed.

## Build from source installation
> **Notes:**
>
> - Power users who decide to build from sources should be aware of the
>   implications of using an unpackaged system which will not be automatically
>   updated as new [releases](../Releases.md) are made available.

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
* The [runtime documentation](https://github.com/kata-containers/runtime/blob/master/README.md).
