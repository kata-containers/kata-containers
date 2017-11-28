# Building a rootfs for Kata Containers Guest OS #

The Kata Containers `rootfs` is created using `rootfs.sh`.

## Supported base OSs ##

The `rootfs.sh` script builds a `rootfs` based on a particular Linux\*
distribution. To build a `rootfs`for your chosen distribution, run:

```
$./rootfs.sh <distro>
```

To check the supported `rootfs` based OS run `$rootfs-builder/rootfs.sh
-h`, it will show the supported values of `<distro>`


## Adding support for new base OS ##

The script `rootfs.sh` will it check for immediate sub-directories
containing the following expected files structure:

- A `bash(1)` script called `rootfs_lib.sh`

  This file must contain a function called `build_rootfs()` this function
  must receive as first argument the path where the `rootfs` will be
  populated. Path: `rootfs-builder/<distro>/rootfs_lib.sh`.


- A `bash(1)` file `config.sh`

  This represents the specific configuration for `<distro>`. It must
  provide configuration specific variables for user to modify as needed.
  The `config.sh` file will be loaded before executing `build_rootfs()` to
  provide all the needed configuration to the function. Path:
  `rootfs-builder/<distro>/config.sh`.

To create a directory with the expected file structure run:

```
make -f template/Makefile  ROOTFS_BASE_NAME=my_new_awesome_rootfs
```

After run the command above, a new directory will be created in
`rootfs-builder/my_new_awesome_rootfs/`. To verify it is one of the
options to build a `rootfs` run `./rootfs.sh -h`, it will show
`my_new_awesome` as one of the options to use it for:

```
./rootfs.sh <distro>
```

Now that a new directory structure was created is need to:

- If needed , add configuration variables to `rootfs-builder/my_new_awesome_rootfs/config.sh`
- Implement the stub `build_rootfs()` function from `rootfs-builder/my_new_awesome_rootfs/rootfs_lib.sh`

### Expected `rootfs` directory content ###

After the function `build_rootfs` is called, the script expects the
`rootfs` directory to contain /sbin/init and /sbin/kata-agent binaries.

### (optional) Customise the `rootfs` ###

For development uses cases, developers may want to modify the guest OS.
To do that it is possible to use following methods:

- Use the environment variable `EXTRA_PKG` to provide a list of space
  separated packages to be installed.

  *Note: The package names may vary among Linux* distributions, the extra
  package names must exist in the base OS flavor you use to build the
  `rootfs`*

  Example:
  ```
  EXTRA_PKG="vim emacs" ./rootfs-builder/rootfs.sh \
  -r ${PWD}/myrootfs fedora

  ```

- In `rootfs-builder/<distro>/config.sh` modify the variable `PACKAGES`.
  This are the minimal set of packages needed. The configuration file must
  use the package names from the distro was created for.

- It is possible to customise the `rootfs` directory before create an
  image based in on it.


## Build `rootfs` using Docker* ##

Depending on the base OS to build the `rootfs` guest OS, it is required some
specific programs that probably are not available or installed in the system
that will build the guest image. For this case `rootfs.sh` can use
a Docker\* container to build the `rootfs`. The following requirements
must be met:

1. Docker 1.12+ installed

2. `runc` is configured as the default runtime

   To check if `runc` is the default runtime:

   ```
   $ docker info | grep 'Default Runtime: runc'
   ```

   Note:
   This requirement is specifically when using Clear Containers runtime
   see [issue](https://github.com/clearcontainers/runtime/issues/828) for
   more information.

3. Export `USE_DOCKER` variable

   ```
   $ export USE_DOCKER=true
   ```
4. Use `rootfs.sh:
   Example:
   ```
   $ export USE_DOCKER=true
   $ # build guest O/S rootfs based on fedora
   $ ./rootfs-builder/rootfs.sh -r "${PWD}/fedora_rootfs" fedora
   $ # build image based rootfs created above
   $ ./image-builder/image_builder.sh "${PWD}/fedora_rootfs"
   ```
