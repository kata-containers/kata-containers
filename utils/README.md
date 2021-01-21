# Utilities

# Kata Manager

> **Warning:**
>
> - Kata Manager will not work for Fedora 31 and higher since those
>   distribution versions only support cgroups version 2 by default. However,
>   Kata Containers currently requires cgroups version 1 (on the host side). See
>   https://github.com/kata-containers/kata-containers/issues/927 for further
>   details.

> **Note:**
>
> We recommend users install Kata Containers using
> [official distribution packages](../docs/install/README.md#official-packages), where available.

The [`kata-manager.sh`](kata-manager.sh) script automatically installs and
configures Kata Containers and containerd.

This scripted method installs the latest versions of Kata Containers and
containerd. However, be aware of the following before proceeding:

- Packages will **not** be automatically updated

  Since a package manager is not being used, it is **your** responsibility
  to ensure these packages are kept up-to-date when new versions are released
  to ensure you are using a version that includes the latest security and bug fixes.

- Potentially untested versions or version combinations

  This script installs the *newest* versions of Kata Containers
  and containerd from binary release packages. These versions may
  not have been tested with your distribution version.

If you still wish to continue, but prefer a manual installation, see
[the containerd installation guide](/docs/install/container-manager/containerd/containerd-install.md).

## Install a minimal Kata Containers system

To install and configure a system with Kata Containers and containerd, run:

```bash
$ bash -c "$(curl -fsSL https://raw.githubusercontent.com/kata-containers/kata-containers/main/utils/kata-manager.sh)"
```

> **Notes:**
>
> - The script must be run on a system that does not have Kata Containers or
>   containerd already installed on it.
>
> - The script accepts up to two parameters which can be used to test
>   pre-release versions (a Kata Containers version, and a containerd
>   version). If either version is unspecified or specified as `""`, the
>   latest official version will be installed.
