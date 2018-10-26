# Kata Containers installation user guides

- [Prerequisites](#prerequisites)
- [Installing on a Linux System](#installing-on-a-linux-system)
    * [Automatic Installation](#automatic-installation)
    * [Scripted Installation](#scripted-installation)
    * [Manual Installation](#manual-installation)
        + [Supported Distributions](#supported-distributions)
- [Installing on a Cloud Service Platform](#installing-on-a-cloud-service-platform)
- [Further information](#further-information)

## Prerequisites
Kata Containers requires nested virtualization or bare metal.
See the
[hardware requirements](https://github.com/kata-containers/runtime/blob/master/README.md#hardware-requirements)
to see if your system is capable of running Kata Containers.

## Installing on a Linux System
The following is an overview of the different installation methods available. All of these methods equally result
in a system configured to run Kata Containers.


| Installation method                                        | Suggested for               | Description                                                                                                                                 | Packaged install | Distributions supported               |
|------------------------------------------------------------|-----------------------------|---------------------------------------------------------------------------------------------------------------------------------------------|------------------|---------------------------------------|
| [Automatic](#automatic-installation)                       | Quick start for new users   | Run a single command to install a full system.                                                                                              | yes              | [see table](#supported-distributions) |
| [Manual](#manual-installation)                             | Self paced user install     | Allows the user to read a brief document and exectute the specified commands step-by-step.                                                  | yes              | [see table](#supported-distributions) |
| [Scripted](#scripted-installation)                         | Administrators              | Generates an installation script which will result in a working system when executed.                                                       | yes              | [see table](#supported-distributions) |
| [Build from sources](../Developer-Guide.md#initial-setup) | Developers and hackers only | Allows power users who are comfortable building software from source to use the latest component versions. Not recommended for normal users. | no               | any distro                            |

> **Notes:**
>
> - The "Packaged install" column shows if the resulting installation
>   uses your distribution's native package format (such as RPM or DEB).
>
> - Power users who decide to build from sources should be aware of the
>   implications of using an unpackaged system which will not be automatically
>   updated as new [releases](../Releases.md) are made available.

### Automatic Installation
[Use kata-manager](installing-with-kata-manager.md) to automatically install Kata packages.

### Scripted Installation
[Use kata-doc-to-script](installing-with-kata-doc-to-script.md) to generate installation scripts that can be reviewed before they are executed.

### Manual Installation
Manual installation instructions are available for [these distributions](#supported-distributions) and document how to:
1. Add the Kata Containers repository to your distro package manager, and import the packages signing key.
2. Install the Kata Containers packages.
3. Install a supported container manager.
4. Configure the container manager to use `kata-runtime` as the default OCI runtime.

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

#### Supported Distributions
|Distro specific installation instructions                          | Versions        |
|-------------------------------------------------------------------|-----------------|
|[CentOS](centos-installation-guide.md)                             | 7               |
|[Fedora](fedora-installation-guide.md)                             | 27, 28          |
|[openSUSE](opensuse-installation-guide.md)                         | Leap (42.3)     |
|[Red Hat Enterprise Linux (RHEL)](rhel-installation-guide.md)      | 7               |
|[SUSE Linux Enterprise Server (SLES)](sles-installation-guide.md)  | SLES 12 SP3     |
|[Ubuntu](ubuntu-installation-guide.md)                             | 16.04, 18.04    |

## Installing on a Cloud Service Platform
* [Amazon Web Services (AWS)](aws-installation-guide.md)
* [Google Compute Engine (GCE)](gce-installation-guide.md)

## Further information
* The [upgrading document](../Upgrading.md).
* The [developer guide](../Developer-Guide.md).
* The [runtime documentation](https://github.com/kata-containers/runtime/blob/master/README.md).
