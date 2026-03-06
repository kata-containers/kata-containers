# Kata Containers 3.0 rust runtime installation
The following is an overview of the different installation methods available.

## Prerequisites

Kata Containers 3.0 rust runtime requires nested virtualization or bare metal. Check
[hardware requirements](/src/runtime/README.md#hardware-requirements) to see if your system is capable of running Kata
Containers.

### Platform support

Kata Containers 3.0 rust runtime currently runs on 64-bit systems supporting the following
architectures:

> **Notes:**
> For other architectures, see https://github.com/kata-containers/kata-containers/issues/4320

| Architecture | Virtualization technology |
|-|-|
| `x86_64`| [Intel](https://www.intel.com) VT-x |
| `aarch64` ("`arm64`")| [ARM](https://www.arm.com) Hyp |

## Packaged installation methods

| Installation method                                  | Description                                                                                  | Automatic updates | Use case                                                                                      | Availability
|------------------------------------------------------|----------------------------------------------------------------------------------------------|-------------------|-----------------------------------------------------------------------------------------------|----------- |
| [Using kata-deploy](#kata-deploy-installation)       | The preferred way to deploy the Kata Containers distributed binaries on a Kubernetes cluster | **No!**           | Best way to give it a try on kata-containers on an already up and running Kubernetes cluster. | Yes |
| [Using official distro packages](#official-packages) | Kata packages provided by Linux distributions official repositories                          | yes               | Recommended for most users. | No |
| [Automatic](#automatic-installation)                 | Run a single command to install a full system                                                | **No!**           | For those wanting the latest release quickly.                                                 | No |
| [Manual](#manual-installation)                       | Follow a guide step-by-step to install a working system                                      | **No!**           | For those who want the latest release with more control.                                      | No |
| [Build from source](#build-from-source-installation) | Build the software components manually                                                       | **No!**           | Power users and developers only.  | Yes |

### Kata Deploy Installation

Follow the [`kata-deploy`](../../tools/packaging/kata-deploy/helm-chart/README.md).

### Official packages

`ToDo`

### Automatic Installation

`ToDo`

### Manual Installation

Given that the Kata Containers release packages are designed to be self-contained, the manual installation process is straightforward and involves downloading the appropriate release package, extracting it, and configuring containerd to use the Kata runtime-rs. Before starting, ensure that you have the following prerequisites in place:

- containerd installed and running (containerd v2.2.1).
- nerdctl installed for testing (nerdctl version 2.2.1).
- zstd installed (to extract the release package) or a newer version of `tar` installed.

Step 1: Download the Kata Containers Release

First, define the version and download the static binary package tailored for your architecture.
We fetch a "static" release containing all necessary components (kernel, rootfs, hypervisors like QEMU/Dragonball, and the runtime binaries) in a single bundle, ensuring compatibility between components.

```bash
# Define version, just an example, you can set it to the latest version or the version you want to install
export KATA_RELEASE="3.26.0"

# Download the static release package
wget https://github.com/kata-containers/kata-containers/releases/download/${KATA_RELEASE}/kata-static-${KATA_RELEASE}-amd64.tar.zst
```

Step 2: Extract the Binaries

Extract the package to a specific directory, typically `/opt/` or you can specify a different location as your preference.

```bash
# Create target directory and extract the package
sudo tar --zstd -xvf kata-static-${KATA_RELEASE}-amd64.tar.zst -C /
```

> **Note:**

> - Using the --zstd flag handles the high-compression format used by Kata releases. Extracting to the root / (with the package internal structure starting at opt/kata) places the files into /opt/kata/, keeping the installation isolated from system-managed binaries.

Step 3: Create a Shim Wrapper Script

To ensure containerd uses the correct configuration and hypervisor (Dragonball in this example), create a wrapper script for the Shim. This script will set the necessary environment variables and invoke the actual Shim binary.

```bash
# Create a wrapper script for the Shim
sudo tee /usr/local/bin/containerd-shim-kata-v2 << 'EOF'
#!/bin/bash
KATA_CONF_FILE=/opt/kata/share/defaults/kata-containers/runtime-rs/configuration-dragonball.toml \
/opt/kata/runtime-rs/bin/containerd-shim-kata-v2 $@
EOF

# Grant execution permissions
sudo chmod +x /usr/local/bin/containerd-shim-kata-v2
```

> **Note:**

> - The Shim is the component that bridges containerd and the Kata Containers.
> - By creating this wrapper, we explicitly point to the runtime-rs (Rust-based runtime) and force it to use the dragonball hypervisor configuration via the `KATA_CONF_FILE` environment variable.
> - Placing it in `/usr/local/bin` makes it accessible in the system PATH.
> - This approach allows us to maintain the original Shim binary in its location while ensuring that containerd uses the correct configuration when invoking the Shim.
> - If you want to use a different hypervisor (e.g., QEMU), you can modify the `KATA_CONF_FILE` path to point to the corresponding configuration file (e.g., `/opt/kata/share/defaults/kata-containers/runtime-rs/configuration-qemu-runtime-rs.toml`).


Step 4: Configure Containerd

Modify the containerd configuration to register Kata as a valid runtime. Edit `/etc/containerd/config.toml` and add the following under the `[plugins.'io.containerd.cri.v1.runtime'.containerd.runtimes]` section:

```toml
[plugins.'io.containerd.cri.v1.runtime'.containerd.runtimes.kata]
  runtime_type = "io.containerd.kata.v2"
```

and a more precise example with all the necessary fields:

```toml
$ cat /etc/containerd/config.toml
version = 3
...
[plugins]
  [plugins.'io.containerd.cri.v1.runtime']
    ...
    [plugins.'io.containerd.cri.v1.runtime'.containerd]
      ...
      [plugins.'io.containerd.cri.v1.runtime'.containerd.runtimes]
        [plugins.'io.containerd.cri.v1.runtime'.containerd.runtimes.kata]
          runtime_type = 'io.containerd.kata.v2'
          runtime_path = ''
          pod_annotations = ["io.katacontainers.*"]
          container_annotations = ["io.katacontainers.*"]
          privileged_without_host_devices = false
          privileged_without_host_devices_all_devices_allowed = false
          cgroup_writable = false
          base_runtime_spec = ''
          cni_conf_dir = ''
          cni_max_conf_num = 0
          snapshotter = ''
          sandboxer = 'podsandbox'
          io_type = ''
```

> **Note:**

> - This tells containerd that when a user requests the kata runtime, it should look for a binary named containerd-shim-kata-v2 in the system PATH to manage the container lifecycle.
> - The `runtime_type` must match the name of the Shim binary we created in Step 3 (without the "containerd-shim-" prefix and "-v2" suffix).
> - After making these changes, restart the containerd service to apply the new configuration.
> - pod_annotations and container_annotations are used to specify which annotations should be passed to the runtime. In this case, we are passing all annotations that start with `"io.katacontainers."` to ensure that any relevant configuration or metadata is available to the Kata runtime when managing containers. You can adjust these annotations based on your specific needs or requirements.
> - sandboxer is set to "podsandbox" to indicate that this runtime should be used for managing pod sandboxes, which is a common use case for Kata Containers in Kubernetes environments.


Step 5: Verify the Installation

Run a test container using nerdctl to verify that the workload is running inside a Kata.

```bash
# Run a test container with Kata runtime
$ sudo nerdctl run --runtime io.containerd.kata.v2 --net=none --rm -it docker.io/library/fedora:42
[root@5db05bd2aca5 /]# ls
afs  bin  boot  dev  etc  home  lib  lib64  lost+found  media  mnt  opt  proc  root  run  sbin  srv  sys  tmp  usr  var
[root@5db05bd2aca5 /]# uname -r
6.18.5
[root@5db05bd2aca5 /]# cat /etc/os-release
NAME="Fedora Linux"
VERSION="42 (Container Image)"
RELEASE_TYPE=stable
ID=fedora
VERSION_ID=42
...
```

> **Note:**

> - The `--runtime io.containerd.kata.v2` flag explicitly tells nerdctl to use the Kata runtime we just configured.
> - The `--net=none` flag is used to disable networking for the test container, which is a common practice when testing runtimes to isolate the environment and ensure that the container is running with the expected configuration.


## Build from source installation

### Rust Environment Set Up

* Download `Rustup` and install  `Rust`
    > **Notes:**
    > For Rust version, please set `RUST_VERSION` to the value of `languages.rust.meta.newest-version key` in [`versions.yaml`](../../versions.yaml) or, if `yq` is available on your system, run `export RUST_VERSION=$(yq read versions.yaml languages.rust.meta.newest-version)`.

    Example for `x86_64`
    ```
    $ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    $ source $HOME/.cargo/env
    $ rustup install ${RUST_VERSION}
    $ rustup default ${RUST_VERSION}-x86_64-unknown-linux-gnu
    ```

* Musl support for fully static binary

    Example for `x86_64`
    ```
    $ rustup target add x86_64-unknown-linux-musl
    ```
* [Musl `libc`](http://musl.libc.org/) install

    Example for musl 1.2.3
    ```
    $ curl -O https://git.musl-libc.org/cgit/musl/snapshot/musl-1.2.3.tar.gz
    $ tar vxf musl-1.2.3.tar.gz
    $ cd musl-1.2.3/
    $ ./configure --prefix=/usr/local/
    $ make && sudo make install
    ```


### Install Kata 3.0 Rust Runtime Shim

```
$ git clone https://github.com/kata-containers/kata-containers.git
$ cd kata-containers/src/runtime-rs
$ make && sudo make install
```
After running the command above, the default config file `configuration.toml` will be installed under `/usr/share/defaults/kata-containers/`,  the binary file `containerd-shim-kata-v2` will be installed under `/usr/local/bin/` .

### Install Shim Without Builtin Dragonball VMM

By default, runtime-rs includes the `Dragonball` VMM. To build without the built-in `Dragonball` hypervisor, use `make USE_BUILTIN_DB=false`:
```bash
$ cd kata-containers/src/runtime-rs
$ make USE_BUILTIN_DB=false
```
After building, specify the desired hypervisor during installation using `HYPERVISOR`. For example, to use `qemu` or `cloud-hypervisor`:

```
sudo make install HYPERVISOR=qemu
```
or
```
sudo make install HYPERVISOR=cloud-hypervisor
```

### Build Kata Containers Kernel
Follow the [Kernel installation guide](/tools/packaging/kernel/README.md).

### Build Kata Rootfs
Follow the [Rootfs installation guide](../../tools/osbuilder/rootfs-builder/README.md).

### Build Kata Image
Follow the [Image installation guide](../../tools/osbuilder/image-builder/README.md).

### Install Containerd

Follow the [Containerd installation guide](container-manager/containerd/containerd-install.md).


