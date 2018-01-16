* [Supported base OSs](#supported-base-oss)
* [Creating a rootfs](#creating-a-rootfs)
* [Build a rootfs using Docker*](#build-a-rootfs-using-docker*)
* [Adding support for a new guest OS](#adding-support-for-a-new-guest-os)
    * [Create template files](#create-template-files)
    * [Modify template files](#modify-template-files)
    * [Expected rootfs directory content](#expected-rootfs-directory-content)
    * [(optional) Customise the rootfs](#(optional)-customise-the-rootfs)
        * [Adding extra packages](#adding-extra-packages)
        * [Arbitary rootfs changes](#arbitary-rootfs-changes)

# Building a Guest OS rootfs for Kata Containers

The Kata Containers rootfs is created using the `rootfs.sh` script.

## Supported base OSs

The `rootfs.sh` script builds a rootfs based on a particular Linux\*
distribution. The script supports multiple distributions and can be extended
to add further ones.

To list the supported distributions, run:

```
$ ./rootfs.sh -h
```

## Rootfs requirements

The rootfs must provide at least the following components:

- [Kata agent](https://github.com/kata-containers/agent)

  Path: `/bin/kata-agent` - Kata Containers guest.

- An `init` system (e.g. `systemd`) to start the Kata agent
  when the guest OS boots.

  Path: `/sbin/init` - init binary called by the kernel.

## Creating a rootfs

To build a rootfs for your chosen distribution, run:

```
$ sudo ./rootfs.sh <distro>
```

## Build a rootfs using Docker*

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

- A `bash(1)` script called `rootfs_lib.sh`

  This file must contain a function called `build_rootfs()`, which must
  receive the path to where the rootfs is created, as its first argument.

  Path: `rootfs-builder/<distro>/rootfs_lib.sh`.


- A `bash(1)` script called `config.sh`

  This represents the specific configuration for `<distro>`. It must
  provide configuration specific variables for the user to modify as needed.
  The `config.sh` file will be loaded before executing `build_rootfs()` to
  provide all the needed configuration to the function.

  Path: `rootfs-builder/<distro>/config.sh`.

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

### (optional) Customise the rootfs

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

#### Arbitary rootfs changes

Once the rootfs directory is created, you can add and remove files as
needed. Changes affect the files included in the final guest image.
