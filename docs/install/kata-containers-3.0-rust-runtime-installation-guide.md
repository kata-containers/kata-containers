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

Follow the [`kata-deploy`](../../tools/packaging/kata-deploy/README.md).
### Official packages
`ToDo`
### Automatic Installation
`ToDo`
### Manual Installation
`ToDo`

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

By default, runtime-rs includes the `Dragonball` VMM. To build without the built-in `Dragonball` hypervisor, use `make USE_BUILDIN_DB=false`:
```bash
$ cd kata-containers/src/runtime-rs
$ make USE_BUILDIN_DB=false
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


