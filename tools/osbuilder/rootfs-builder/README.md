* [Building a Guest OS rootfs for Kata Containers](#building-a-guest-os-rootfs-for-kata-containers)
  * [Supported base OSs](#supported-base-oss)
     * [Extra features](#extra-features)
        * [Supported distributions list](#supported-distributions-list)
        * [Generate Kata specific files](#generate-kata-specific-files)
  * [Rootfs requirements](#rootfs-requirements)
  * [Creating a rootfs](#creating-a-rootfs)
  * [Creating a rootfs with kernel modules](#creating-a-rootfs-with-kernel-modules)
  * [Build a rootfs using Docker](#build-a-rootfs-using-docker)
  * [Adding support for a new guest OS](#adding-support-for-a-new-guest-os)
     * [Create template files](#create-template-files)
     * [Modify template files](#modify-template-files)
     * [Expected rootfs directory content](#expected-rootfs-directory-content)
     * [Optional - Customize the rootfs](#optional---customize-the-rootfs)
        * [Adding extra packages](#adding-extra-packages)
        * [Arbitrary rootfs changes](#arbitrary-rootfs-changes)

# Building a Guest OS rootfs for Kata Containers

The Kata Containers rootfs is created using the `rootfs.sh` script.

## Supported base OSs

The `rootfs.sh` script builds a rootfs based on a particular Linux\*
distribution. The script supports multiple distributions and can be extended
to add further ones.

### Extra features

#### Supported distributions list

List the supported distributions by running the following:
```
$ ./rootfs.sh -l
```

#### Generate Kata specific files
The `rootfs.sh` script can be used to populate a directory with only Kata specific files and
components, without creating a full usable rootfs.
This feature is used to create a rootfs based on a distribution not officially
supported by osbuilder, and when building an image using the dracut build method.

To achieve this, simply invoke `rootfs.sh` without specifying a target rootfs, e.g.:
```
$ mkdir kata-overlay
$ ./rootfs.sh -r "$PWD/kata-overlay"
```

## Rootfs requirements

The rootfs must provide at least the following components:

- [Kata agent](https://github.com/kata-containers/kata-containers/tree/main/src/agent)

  Path: `/bin/kata-agent` - Kata Containers guest.

- An `init` system (e.g. `systemd`) to start the Kata agent
  when the guest OS boots.

  Path: `/sbin/init` - init binary called by the kernel.

When the `AGENT_INIT` environment variable is set to `yes`, use Kata agent as `/sbin/init`.

> **Note**: `AGENT_INIT=yes` **must** be used for the Alpine distribution
> since it does not use `systemd` as its init daemon.

## Creating a rootfs

To build a rootfs for your chosen distribution, run:

```
$ sudo ./rootfs.sh <distro>
```

## Creating a rootfs with kernel modules

To build a rootfs with additional kernel modules, run:
```
$ sudo KERNEL_MODULES_DIR=${kernel_mod_dir} ./rootfs.sh <distro>
```
Where `kernel_mod_dir` points to the kernel modules directory to be put under the
`/lib/modules/` directory of the created rootfs.

## Build a rootfs using Docker

Depending on the base OS to build the rootfs guest OS, it is required some
specific programs that probably are not available or installed in the system
that will build the guest image. For this case `rootfs.sh` can use
a Docker\* container to build the rootfs. The following requirements
must be met:

1. Docker 1.12+ installed.

2. `runc` is configured as the default runtime.

   To check if `runc` is the default runtime:

   ```
   $ docker info | grep 'Default Runtime: runc'
   ```

   Note:

   This requirement is specific to the Clear Containers runtime.
   See [issue](https://github.com/clearcontainers/runtime/issues/828) for
   more information.

3. Export `USE_DOCKER` variable.

   ```
   $ export USE_DOCKER=true
   ```

4. Use `rootfs.sh`:

   Example:
   ```
   $ export USE_DOCKER=true
   $ # build guest O/S rootfs based on fedora
   $ ./rootfs-builder/rootfs.sh -r "${PWD}/fedora_rootfs" fedora
   $ # build image based rootfs created above
   $ ./image-builder/image_builder.sh "${PWD}/fedora_rootfs"
   ```

## Adding support for a new guest OS

The `rootfs.sh` script will check for immediate sub-directories
containing the following expected files:

- A `bash(1)` script called `config.sh`

  This represents the specific configuration for `<distro>`. It must
  provide configuration specific variables for the user to modify as needed.
  The `config.sh` file will be loaded before executing `build_rootfs()` to
  provide all the needed configuration to the function.

  Path: `rootfs-builder/<distro>/config.sh`.

- (OPTIONAL) A `bash(1)` script called `rootfs_lib.sh`

  This file must contain a function called `build_rootfs()`, which must
  receive the path to where the rootfs is created, as its first argument.
  Normally, this file is needed if a new distro with a special requirement
  is needed. This function will override the `build_rootfs()` function in
  `scripts/lib.sh`.

  Path: `rootfs-builder/<distro>/rootfs_lib.sh`.

### Create template files

To create a directory with the expected file structure run:

```
$ make -f template/Makefile  ROOTFS_BASE_NAME=my_new_awesome_rootfs
```

After running the previous command, a new directory is created in
`rootfs-builder/my_new_awesome_rootfs/`.


To verify the directory can be used to build a rootfs, run `./rootfs.sh -h`.
Running this script shows `my_new_awesome_rootfs` as one of the options for
use. To use the new guest OS, follow the instructions in [Creating a rootfs](#creating-a-rootfs).

### Modify template files

After the new directory structure is created:

- If needed, add configuration variables to
  `rootfs-builder/my_new_awesome_rootfs/config.sh`.

- Implement the stub `build_rootfs()` function from
  `rootfs-builder/my_new_awesome_rootfs/rootfs_lib.sh`.

### Expected rootfs directory content

After the function `build_rootfs` is called, the script expects the
rootfs directory to contain `/sbin/init` and `/sbin/kata-agent` binaries.

### Optional - Customize the rootfs

For particular use cases developers might want to modify the guest OS.

#### Adding extra packages

To add additional packages, use one of the following methods:

- Use the environment variable `EXTRA_PKGS` to provide a list of space-separated
  packages to install.

  Note:

  The package names might vary among Linux distributions, the extra
  package names must exist in the base OS flavor you use to build the
  rootfs from.

  Example:

  ```
  $ EXTRA_PKGS="vim emacs" ./rootfs-builder/rootfs.sh -r ${PWD}/myrootfs fedora
  ```

- Modify the variable `PACKAGES` in `rootfs-builder/<distro>/config.sh`.

  This variable specifies the minimal set of packages needed. The
  configuration file must use the package names from the distro for which they
  were created.

#### Arbitrary rootfs changes

Once the rootfs directory is created, you can add and remove files as
needed. Changes affect the files included in the final guest image.
