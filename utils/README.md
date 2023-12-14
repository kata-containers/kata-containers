# Utilities

# Kata Manager

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

By default, the script will attempt to install Kata Containers and
containerd, and then configure containerd to use Kata Containers. However,
the script provides a number of options to allow you to change its
behaviour.

> **Note:**
>
> Before running the script to install Kata Containers, we recommend
> that you [review the available options](#show-available-options).

### Show available options

To show the available options without installing anything, run:

```sh
$ bash -c "$(curl -fsSL https://raw.githubusercontent.com/kata-containers/kata-containers/main/utils/kata-manager.sh) -h"
```

### To install Kata Containers only

If your system already has containerd installed, to install Kata Containers and only configure containerd, run:

```sh
$ bash -c "$(curl -fsSL https://raw.githubusercontent.com/kata-containers/kata-containers/main/utils/kata-manager.sh) -o"
```

### To install Kata Containers and containerd

To install and configure a system with Kata Containers and containerd, run:

```bash
$ bash -c "$(curl -fsSL https://raw.githubusercontent.com/kata-containers/kata-containers/main/utils/kata-manager.sh)"
```
