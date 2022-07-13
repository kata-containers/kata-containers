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
| [Using kata-deploy](#kata-deploy-installation)       | The preferred way to deploy the Kata Containers distributed binaries on a Kubernetes cluster | **No!**           | Best way to give it a try on kata-containers on an already up and running Kubernetes cluster. | No |
| [Using official distro packages](#official-packages) | Kata packages provided by Linux distributions official repositories                          | yes               | Recommended for most users. | No |                                                                   
| [Using snap](#snap-installation)                     | Easy to install                                                                              | yes               | Good alternative to official distro packages.                                                 | No |
| [Automatic](#automatic-installation)                 | Run a single command to install a full system                                                | **No!**           | For those wanting the latest release quickly.                                                 | No |
| [Manual](#manual-installation)                       | Follow a guide step-by-step to install a working system                                      | **No!**           | For those who want the latest release with more control.                                      | No |
| [Build from source](#build-from-source-installation) | Build the software components manually                                                       | **No!**           | Power users and developers only.  | Yes |              

### Kata Deploy Installation
`ToDo`
### Official packages
`ToDo`
### Snap Installation
`ToDo`
### Automatic Installation
`ToDo`
### Manual Installation
`ToDo`

## Build from source installation

### Install Kata 3.0 Rust Runtime Shim

```
$ git clone https://github.com/kata-containers/kata-containers.git
$ cd kata-containers/src/runtime-rs
$ make && make install
```
After running the command above, the default config file `configuration.toml` will be installed under `/usr/share/defaults/kata-containers/`,  the binary file `containerd-shim-kata-v2` will be installed under `/user/local/bin` .

### Build Kata Containers Kernel
Follow the [Kernel installation guide](/tools/packaging/kernel/README.md).

### Build Kata Rootfs
Follow the [Rootfs installation guide](../../tools/osbuilder/rootfs-builder/README.md).

### Build Kata Image
Follow the [Image installation guide](../../tools/osbuilder/image-builder/README.md).

### Install Containerd

Follow the [Containerd installation guide](container-manager/containerd/containerd-install.md).


