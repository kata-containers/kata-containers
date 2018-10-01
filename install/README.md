# Kata Containers installation user guides

* [Prerequisites](#prerequisites)
* [Installing Kata Containers](#installing-kata-containers)
    * [Distros](#distros)
    * [Cloud services](#cloud-services)
* [Further information](#further-information)

## Prerequisites

Kata Containers requires nested virtualization or bare metal.
See the
[hardware requirements](https://github.com/kata-containers/runtime/blob/master/README.md#hardware-requirements)
to see if your system is capable of running Kata Containers.

## Installing Kata Containers

> **Notes:**
> - [This download server](http://download.opensuse.org/repositories/home:/katacontainers:/releases:/)
> hosts the Kata Containers packages built by OBS for all the supported architectures.
> Packages are available for the latest and stable releases (more info [here](https://github.com/kata-containers/documentation/blob/master/Stable-Branch-Strategy.md)).
>
> - The following guides apply to the latest Kata Containers release
> (a.k.a. `master` release).
>
> - When choosing a stable release, replace all `master` occurrences in the URLs
> with a `stable-x.y` version available on the [download server](http://download.opensuse.org/repositories/home:/katacontainers:/releases:/).

### Distros

* [CentOS](centos-installation-guide.md)
* [Fedora](fedora-installation-guide.md)
* [Red Hat](rhel-installation-guide.md)
* [OpenSuse](opensuse-installation-guide.md)
* [Ubuntu](ubuntu-installation-guide.md)
* [SLES](sles-installation-guide.md)

### Cloud services

* [Google Compute Engine (GCE)](gce-installation-guide.md)

## Further information

* The [upgrading document](../Upgrading.md).
* The [developer guide](../Developer-Guide.md).
* The [runtime documentation](https://github.com/kata-containers/runtime/blob/master/README.md).
