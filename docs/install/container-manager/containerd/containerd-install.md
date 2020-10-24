# Install Kata Containers with containerd

> **Note:**
>
> - If Kata Containers and / or containerd are packaged by your distribution,
>   we recommend you install these versions to ensure they are updated when
>   new releases are available.

> **Warning:**
>
> - These instructions install the **newest** versions of Kata Containers and
>   containerd from binary release packages. These versions may not have been
>   tested with your distribution version.
>
> - Since your package manager is not being used, it is **your**
>   responsibility to ensure these packages are kept up-to-date when new
>   versions are released.
>
> - If you decide to proceed and install a Kata Containers release, you can
>   still check for the latest version of Kata Containers by running
>   `kata-runtime check --only-list-releases`.
>
> - These instructions will not work for Fedora 31 and higher since those
>   distribution versions only support cgroups version 2 by default. However,
>   Kata Containers currently requires cgroups version 1 (on the host side). See
>   https://github.com/kata-containers/kata-containers/issues/927 for further
>   details.

## Install Kata Containers

> **Note:**
>
> If your distribution packages Kata Containers, we recommend you install that
> version. If it does not, or you wish to perform a manual installation,
> continue with the steps below.

- Download a release from:

  - https://github.com/kata-containers/kata-containers/releases

  Note that Kata Containers uses [semantic versioning](https://semver.org) so
  you should install a version that does *not* include a dash ("-"), since this
  indicates a pre-release version.

- Unpack the downloaded archive.

   Kata Containers packages use a `/opt/kata/` prefix so either add that to
   your `PATH`, or create symbolic links for the following commands. The
   advantage of using symbolic links is that the `systemd(1)` configuration file
   for containerd will not need to be modified to allow the daemon to find this
   binary (see the [section on installing containerd](#install-containerd) below).

   | Command | Description |
   |-|-|
   | `/opt/kata/bin/containerd-shim-kata-v2` | The main Kata 2.x binary |
   | `/opt/kata/bin/kata-collect-data.sh`    | Data collection script used for [raising issues](https://github.com/kata-containers/kata-containers/issues) |
   | `/opt/kata/bin/kata-runtime`            | Utility command |

- Check installation by showing version details:

   ```bash
   $ kata-runtime --version
   ```

## Install containerd

> **Note:**
>
> If your distribution packages containerd, we recommend you install that
> version. If it does not, or you wish to perform a manual installation,
> continue with the steps below.

- Download a release from:

  - https://github.com/containerd/containerd/releases

- Unpack the downloaded archive.

- Configure containerd

  - Download the standard `systemd(1)` service file and install to
    `/etc/systemd/system/`:

    - https://raw.githubusercontent.com/containerd/containerd/master/containerd.service

    > **Notes:**
    >
    > - You will need to reload the systemd configuration after installing this
    >   file.
    >
    > - If you have not created a symbolic link for
    >   `/opt/kata/bin/containerd-shim-kata-v2`, you will need to modify this
    >   file to ensure the containerd daemon's `PATH` contains `/opt/kata/`.
    >   See the `Environment=` command in `systemd.exec(5)` for further
    >   details.

  - Add the Kata Containers configuration to the containerd configuration file:

    ```toml
    [plugins]
        [plugins.cri]
            [plugins.cri.containerd]
            default_runtime_name = "kata"

            [plugins.cri.containerd.runtimes.kata]
            runtime_type = "io.containerd.kata.v2"
    ```

    > **Note:**
    >    
    > The containerd daemon needs to be able to find the
    > `containerd-shim-kata-v2` binary to allow Kata Containers to be created.

  - Start the containerd service.

## Test the installation

You are now ready to run Kata Containers. You can perform a simple test by
running the following commands:

```bash
$ image="docker.io/library/busybox:latest"
$ sudo ctr image pull "$image"
$ sudo ctr run --runtime "io.containerd.kata.v2" --rm -t "$image" test-kata uname -r
```

The last command above shows details of the kernel version running inside the
container, which will likely be different to the host kernel version.
