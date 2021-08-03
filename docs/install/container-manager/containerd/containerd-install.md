# Install Kata Containers with containerd

> **Note:**
>
> - If Kata Containers and / or containerd are packaged by your distribution,
>   we recommend you install these versions to ensure they are updated when
>   new releases are available.
> 
> - Quick installation using this documentation by generating an script from here.
> ```
> $ curl -fsSL -O https://raw.githubusercontent.com/kata-containers/kata-containers/blob/main/docs/install/container-manager/containerd/containerd-install.md
> $ bash -c "$(curl -fsSL https://raw.githubusercontent.com/kata-containers/tests/main/.ci/kata-doc-to-script.sh) containerd-install.md installer.sh"
> # Review the generated script
> $ bash installer.sh
> ```
> Or
> ```
> $ bash -c "$(curl -fsSL https://raw.githubusercontent.com/kata-containers/kata-containers/blob/main/ci/installer/containerd.sh)"
> ```
>
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
  you *should* install a version that does *not* include a dash ("-"), since this
  indicates a pre-release version.
  ```bash
  # Export the version you want to install
  # e.g.
  # export KATA_VERSION=2.2.1
  latest_kata=$(curl -L https://raw.githubusercontent.com/kata-containers/kata-containers/main/VERSION)
  KATA_VERSION="${KATA_VERSION:-$latest_kata}"
  echo "INFO: Installing kata version $KATA_VERSION"
  KATA_TARBALL_URL="https://github.com/kata-containers/kata-containers/releases/download/${KATA_VERSION}/kata-static-${KATA_VERSION}-$(uname -m).tar.xz"
  [ -f "kata-tarball.tar.xz" ] || curl -o kata-tarball.tar.xz -L "${KATA_TARBALL_URL}"
  ```

- Unpack the downloaded archive.

    ```bash
    sudo tar xvf  kata-tarball.tar.xz -C /
    ```

   Kata Containers packages use a `/opt/kata/` prefix so either add that to
   your `PATH`, or create symbolic links for the following commands.
   ```bash
   echo "INFO: add kata to PATH using symbolic liks from /opt to /usr/local/bin"
   sudo ln -sf /opt/kata/bin/containerd-shim-kata-v2 /usr/local/bin/containerd-shim-kata-v2
   sudo ln -sf /opt/kata/bin/kata-collect-data.sh /usr/local/bin/kata-collect-data.sh
   sudo ln -sf /opt/kata/bin/kata-runtime /usr/local/bin/kata-runtime
   ```
   The advantage of using symbolic links is that the `systemd(1)` configuration
   file for containerd will not need to be modified to allow the daemon to find
   this binary (see the [section on installing containerd](#install-containerd)
   below).

   | Command | Description |
   |-|-|
   | `/opt/kata/bin/containerd-shim-kata-v2` | The main Kata 2.x binary |
   | `/opt/kata/bin/kata-collect-data.sh`    | Data collection script used for [raising issues](https://github.com/kata-containers/kata-containers/issues) |
   | `/opt/kata/bin/kata-runtime`            | Utility command |

- Check installation by showing version details:

   ```bash
   echo "INFO: Installed Kata version:"
   kata-runtime --version
   echo "INFO: Check kata can run in this host"
   sudo kata-runtime kata-check
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

      ```bash
      if command -v containerd; then
        echo "INFO: Containerd alraedy installed"
      else
        containerd_version=$(yq read versions.yaml "externals.cri-containerd.version")
        echo "INFO: Install Containerd $containerd_version"
        case "$(uname -m)" in
          aarch64) goarch="arm64" ;;
          ppc64le) goarch="ppc64le" ;;
          x86_64) goarch="amd64" ;;
          s390x) goarch="s390x" ;;
        *) echo "Unknown architecture for goarch format: $(uname -m)" ;exit 1;;
        esac
        curl -o containerd.tar.gz -L https://github.com/containerd/containerd/releases/download/${containerd_version}/cri-containerd-cni-${containerd_version#v}-linux-${goarch}.tar.gz

        sudo tar -xvf ./containerd.tar.gz -C /
      fi
      ```


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
      [plugins."io.containerd.grpc.v1.cri"]
        [plugins."io.containerd.grpc.v1.cri".containerd]
          default_runtime_name = "kata"
          [plugins."io.containerd.grpc.v1.cri".containerd.runtimes]
            [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata]
              runtime_type = "io.containerd.kata.v2"
    ```
    ```bash
    echo "INFO: Remove previous kata containerd config"
    [ -d /etc/containerd ] || sudo mkdir -p /etc/containerd/
    [ -f /etc/containerd/config.toml ] || sudo touch /etc/containerd/config.toml
    sudo sed -i '/#KATA_INSTALLER_CONFIG/{:a;N;/#END_KATA_INSTALLER_CONFIG/!ba};/#KATA_INSTALLER_CONFIG/d' /etc/containerd/config.toml
    ```

    ```bash
    echo "INFO: modifying containerd config"
    sudo tee -a /etc/containerd/config.toml <<EOF
    #KATA_INSTALLER_CONFIG
    [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata]
      runtime_type = "io.containerd.kata.v2"
    #END_KATA_INSTALLER_CONFIG
    EOF
    ```

    > **Note:**
    >    
    > The containerd daemon needs to be able to find the
    > `containerd-shim-kata-v2` binary to allow Kata Containers to be created.

  - Start the containerd service.
      ```bash
      echo "INFO: Restart containerd service"
      sudo systemctl daemon-reload
      sudo systemctl restart containerd
      ```

## Test the installation

You are now ready to run Kata Containers. You can perform a simple test by
running the following commands:

```bash
$ echo "Test containerd with kata"
$ image="docker.io/library/busybox:latest"
$ sudo ctr image pull "$image"
$ sudo ctr run --runtime "io.containerd.kata.v2" --rm -t "$image" test-kata sh -c 'echo "Hello from kata with kernel $(uname -r)"'
```

The last command above shows details of the kernel version running inside the
container, which will likely be different to the host kernel version.
