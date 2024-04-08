# Utilities

## Kata Manager

> **Notes:**
>
> - We recommend users install Kata Containers using
>   [official distribution packages](../docs/install/README.md#official-packages), where available.
>
> - These instructions only apply to the current default (golang) Kata runtime.
>   See https://github.com/kata-containers/kata-containers/issues/9060 for further details.
>
> - If you get a "command not found" error when you try to run the `kata-manager` command,
>   your version of Kata Containers is too old. Please consider
>   [updating to version 3.3.0 or newer](https://github.com/kata-containers/kata-containers/releases).

### Permissions

The permissions of certain Kata binaries are currently overly
restrictive. To allow you to call the `kata-manager` script directly
as shown below, run the following command:

```bash
$ sudo chmod 755 /opt/kata/bin/kata-manager*
```

Alternatively, call the `kata-manager` script using `sudo(1)` by
specifying its full path in all the examples below. For example,
to show the usage statement:

```bash
$ sudo /opt/kata/bin/kata-manager -h
```

> **Note:**
>
> For further details, see:
> https://github.com/kata-containers/kata-containers/issues/9373.

The [`kata-manager.sh`](kata-manager.sh) script automatically installs and
configures Kata Containers and a container manager (such as containerd, Docker and `nerdctl`).

By default, the script installs the latest versions of Kata Containers and
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

### Install a minimal Kata Containers system

By default, the script will attempt to install Kata Containers and
containerd, and then configure containerd to use Kata Containers. However,
the script provides a number of options to allow you to change its
behaviour.

> **Notes:**
>
> - Before running the script to install Kata Containers, we recommend
>   that you [review the available options](#show-available-options).
>
> - The `kata-manager.sh` script is
>   [now packaged](https://github.com/kata-containers/kata-containers/pull/9091)
>   as part of the Kata Containers release and can be called as either
>   `kata-manager.sh` or simply `kata-manager`. Some of the sections
>   below give two examples of how to run the script: running the
>   local version, and by downloading and running the latest version
>   in the Kata Containers GitHub repository. If your version of Kata
>   Containers includes the `kata-manager.sh` script, you can run
>   either version although we would suggest you use the local version
>   since: (a) it has been tested with the Kata Containers release you
>   are using; and (b) it is simpler to run the command directly.

#### Show available options

To show the available options without installing anything, run:

```sh
$ bash -c "$(curl -fsSL https://raw.githubusercontent.com/kata-containers/kata-containers/main/utils/kata-manager.sh) -h"
```

#### To install Kata Containers only

If your system already has containerd installed, to install Kata Containers and only configure containerd, run:

```sh
$ bash -c "$(curl -fsSL https://raw.githubusercontent.com/kata-containers/kata-containers/main/utils/kata-manager.sh) -o"
```

#### To install Kata Containers and containerd

To install and configure a system with Kata Containers and containerd, run:

```bash
$ bash -c "$(curl -fsSL https://raw.githubusercontent.com/kata-containers/kata-containers/main/utils/kata-manager.sh)"
```

### Choose a Hypervisor

Kata works with different [hypervisors](../docs/hypervisors.md). When you install a Kata system, the default hypervisor
will be configured, but all the available hypervisors are installed.
This means you can switch between hypervisors whenever you wish.

#### List available hypervisors

Run the following command on an installed system:

```bash
$ kata-manager -L
```

#### Show the default packaged hypervisor

To show the default packaged hypervisor, run the following
command on an installed system:

```bash
$ kata-manager -L | grep default
```

#### Show the locally configured hypervisor

`kata-manager.sh` will create a "local" copy of the packaged Kata configuration
file.

To show details of the _local_ copy of the configuration files, run
the following command on an installed system:

```bash
$ kata-manager -e
```

> **Note:** This command can only be run once Kata has been installed.

> See the [configuration documentation](https://github.com/kata-containers/kata-containers#configuration)
> for further information.

### Switch hypervisor

> **Note:**
>
> If you create your own local configuration files, you should ensure
> they are backed up safely before switching hypervisor configuration
> since the script will overwrite any files that it needs to create in
> the Kata configuration directory (`/etc/kata-containers/` and
> sub-directories).

#### To install Kata Containers and containerd and configure it for a specific hypervisor

To specify that Kata be installed and configured to use a specific
hypervisor, use the `-H` option. For example, to select Cloud Hypervisor, run:

```bash
$ bash -c "$(curl -fsSL https://raw.githubusercontent.com/kata-containers/kata-containers/main/utils/kata-manager.sh) -H clh"
```

> **Note:** See the [List available hypervisors](#list-available-hypervisors) section
> for details of how to obtain the list of available hypervisor names.

#### To switch the locally installed hypervisor

To switch the local hypervisor config on an installed system use the
`-S` option. For example, to switch to the Cloud Hypervisor hypervisor,
run the following command on an installed system:

```bash
$ kata-manager -S clh
```

> **Note:** See the [List available hypervisors](#list-available-hypervisors) section
> for details of how to obtain the list of available hypervisor names.

#### Switch to the default packaged hypervisor

To undo your changes and switch back to the default Kata hypervisor,
specify the hypervisor name as `default`. For example, run the following command on an installed system:

```bash
$ kata-manager -S default
```
