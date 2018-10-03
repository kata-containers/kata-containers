# Installing with kata-manager

* [Introduction](#Introduction)
* [Full Installation](#full-installation)
* [Install the Kata packages only](#install-the-kata-packages-only)
* [Further Information](#further-information)

## Introduction
`kata-manager` automates the Kata Containers installation procedure documented for [these Linux distributions](README.md#supported-distributions).

> Note:
> - Full installation mode is only available for Docker container manager. For other setups, you
> can still use `kata-manager` to [install Kata package](#install-kata-packages-only), and then setup your container manager manually.

## Full Installation
This command does the following:
1. Installs Kata Containers packages
2. Installs Docker
3. Configure Docker to use the Kata OCI runtime by default

```
$ bash -c "$(curl -fsSL \
$ https://raw.githubusercontent.com/kata-containers/tests/master/cmd/kata-manager/kata-manager.sh) \
 install-docker-system"
```

## Install the Kata packages only
Use the following command to only install Kata Containers packages.

```
$ bash -c "$(curl -fsSL \
$ https://raw.githubusercontent.com/kata-containers/tests/master/cmd/kata-manager/kata-manager.sh) \
 install-packages"
```

## Further Information
For more information on what `kata-manager` can do, refer to the [`kata-manager` page](https://github.com/kata-containers/tests/blob/master/cmd/kata-manager).
