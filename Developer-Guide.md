* [Warning](#warning)
* [Assumptions](#assumptions)
* [Build and install the Kata Containers runtime](#build-and-install-the-kata-containers-runtime)
    * [Check hardware requirements](#check-hardware-requirements)
    * [Configure to use initrd or rootfs image](#configure-to-use-initrd-or-rootfs-image)
    * [Enable full debug](#enable-full-debug)
* [Build and install Kata proxy](#build-and-install-kata-proxy)
* [Build and install Kata shim](#build-and-install-kata-shim)
* [Create and install rootfs and initrd image](#create-and-install-rootfs-and-initrd-image)
    * [Build a custom Kata agent - OPTIONAL](#build-a-custom-kata-agent---optional)
    * [Get the osbuilder](#get-the-osbuilder)
    * [Create a rootfs image](#create-a-rootfs-image)
        * [Create a local rootfs](#create-a-local-rootfs)
        * [Add a custom agent to the image - OPTIONAL](#add-a-custom-agent-to-the-image---optional)
        * [Build a rootfs image](#build-a-rootfs-image)
        * [Install the rootfs image](#install-the-rootfs-image)
    * [Create an initrd image - OPTIONAL](#create-an-initrd-image---optional)
        * [Create a local rootfs for initrd image](#create-a-local-rootfs-for-initrd-image)
        * [Build an initrd image](#build-an-initrd-image)
        * [Install the initrd image](#install-the-initrd-image)
* [Install guest kernel images](#install-guest-kernel-images)
* [Update Docker configuration](#update-docker-configuration)
* [Create a Kata Container](#create-a-kata-container)
* [Troubleshoot Kata Containers](#troubleshoot-kata-containers)
* [Appendices](#appendices)
    * [Checking Docker default runtime](#checking-docker-default-runtime)

# Warning

This document is written **specifically for developers**.

# Assumptions

- You are working on a non-critical test or development system.
- You already have the following installed:
  - [Docker](https://www.docker.com/).
  - [golang](https://golang.org/dl) version 1.8.3 or newer.
  - `make`.
  - `gcc` (required for building the shim and runtime).

- You have installed the Clear Containers `linux-container` and `qemu-lite`
  packages containing the guest kernel images and hypervisor. These packages
  automatically installed when you install Clear Containers, but can be
  installed separately:

    https://github.com/clearcontainers/runtime/wiki/Installation

# Build and install the Kata Containers runtime

```
$ go get -d -u github.com/kata-containers/runtime
$ cd $GOPATH/src/github.com/kata-containers/runtime
$ make && sudo -E PATH=$PATH make install
```

The build will create the following:

- runtime binary: `/usr/local/bin/kata-runtime`
- configuration file: `/usr/share/defaults/kata-containers/configuration.toml`

# Check hardware requirements

You can check if your system is capable of creating a Kata Container by running the following:

```
$ sudo kata-runtime kata-check
```

If your system is *not* able to run Kata Containers, the previous command will error and explain why.

## Configure to use initrd or rootfs image

Kata containers can run with either an initrd image or a rootfs image.

If you want to test with `initrd`, make sure you have `initrd = /usr/share/kata-containers/kata-containers-initrd.img`
in `/usr/share/defaults/kata-containers/configuration.toml` and remove the `image` line with the following:

```
$ sudo sed -i 's/^\(image =.*\)/# \1/g' /usr/share/defaults/kata-containers/configuration.toml
```
You can create the initrd image as shown in the [create an initrd image](#create-an-initrd-image---optional) secion.

If you test with a rootfs `image`, make sure you have `image = /usr/share/kata-containers/kata-containers.img`
in `/usr/share/defaults/kata-containers/configuration.toml` and remove the `initrd` line with the following:

```
$ sudo sed -i 's/^\(initrd =.*\)/# \1/g' /usr/share/defaults/kata-containers/configuration.toml
```
The rootfs image is created as shown in the [create a rootfs image](#create-a-rootfs-image) secion.

One of the `initrd` and `image` options in kata runtime config file **MUST** be set but **not both**.
The main difference between the options is that the size of `initrd`(10MB+) is significantly smaller than
rootfs `image`(100MB+).

## Enable full debug

Enable full debug as follows:

```
$ sudo sed -i -e 's/^# *\(enable_debug\).*=.*$/\1 = true/g' /usr/share/defaults/kata-containers/configuration.toml
$ sudo sed -i -e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 agent.log=debug"/g' /usr/share/defaults/kata-containers/configuration.toml
```

# Build and install Kata proxy

```
$ go get -d -u github.com/kata-containers/proxy
$ cd $GOPATH/src/github.com/kata-containers/proxy && make && sudo make install
```

# Build and install Kata shim

```
$ go get -d -u github.com/kata-containers/shim
$ cd $GOPATH/src/github.com/kata-containers/shim && make && sudo make install
```

# Create and install rootfs and initrd image

## Build a custom Kata agent - OPTIONAL

> **Note:**
>
> - You only do this step if you test with the latest version of the agent.

```
$ go get -d -u github.com/kata-containers/agent
$ cd $GOPATH/src/github.com/kata-containers/agent && make
```

## Get the osbuilder

```
$ go get -d -u github.com/kata-containers/osbuilder
```

## Create a rootfs image
### Create a local rootfs
```
$ export ROOTFS_DIR=${GOPATH}/src/github.com/kata-containers/osbuilder/rootfs-builder/rootfs
$ rm -rf ${ROOTFS_DIR}
$ cd $GOPATH/src/github.com/kata-containers/osbuilder/rootfs-builder
$ script -fec 'sudo -E GOPATH=$GOPATH USE_DOCKER=true ./rootfs.sh ${distro}'
```
You MUST choose one of `alpine`, `centos`, `clearlinux`, `euleros`, and `fedora` for `${distro}`.

> **Note:**
>
> - You must ensure that the *default Docker runtime* is `runc` to make use of
>   the `USE_DOCKER` variable. If that is not the case, remove the variable
>   from the previous command. See [Checking Docker default runtime](#checking-docker-default-runtime).

### Add a custom agent to the image - OPTIONAL

> **Note:**
>
> - You only do this step if you test with the latest version of the agent.

```
$ sudo install -o root -g root -m 0550 -t ${ROOTFS_DIR}/bin ../../agent/kata-agent
$ sudo install -o root -g root -m 0440 ../../agent/kata-agent.service ${ROOTFS_DIR}/usr/lib/systemd/system/
$ sudo install -o root -g root -m 0440 ../../agent/kata-containers.target ${ROOTFS_DIR}/usr/lib/systemd/system/
```

### Build a rootfs image

```
$ cd $GOPATH/src/github.com/kata-containers/osbuilder/image-builder
$ script -fec 'sudo -E USE_DOCKER=true ./image_builder.sh ${ROOTFS_DIR}'
```

> **Notes:**
> 
> - You must ensure that the *default Docker runtime* is `runc` to make use of
>   the `USE_DOCKER` variable. If that is not the case, remove the variable
>   from the previous command. See [Checking Docker default runtime](#checking-docker-default-runtime).
> - If you do *not* wish to build under Docker, remove the `USE_DOCKER`
>   variable in the previous command and ensure the `qemu-img` command is
>   available on your system.


### Install the rootfs image

```
$ commit=$(git log --format=%h -1 HEAD)
$ date=$(date +%Y-%m-%d-%T.%N%z)
$ image="kata-containers-${date}-${commit}"
$ sudo install -o root -g root -m 0640 -D kata-containers.img "/usr/share/kata-containers/${image}"
$ (cd /usr/share/kata-containers && sudo ln -sf "$image" kata-containers.img)
```

## Create an initrd image - OPTIONAL
### Create a local rootfs for initrd image
```
$ export ROOTFS_DIR="${GOPATH}/src/github.com/kata-containers/osbuilder/rootfs-builder/rootfs"
$ rm -rf ${ROOTFS_DIR}
$ cd $GOPATH/src/github.com/kata-containers/osbuilder/rootfs-builder
$ script -fec 'sudo -E GOPATH=$GOPATH AGENT_INIT=yes USE_DOCKER=true ./rootfs.sh ${distro}'
```
`AGENT_INIT` controls if the guest image uses kata agent as the guest `init` process. When you create an initrd image,
always set `AGENT_INIT` to `yes`.

You MUST choose one of `alpine`, `centos`, `clearlinux`, `euleros`, and `fedora` for `${distro}`.

Optionally, add your custom agent binary to the rootfs with the following:
```
$ sudo install -o root -g root -m 0550 -T ../../agent/kata-agent ${ROOTFS_DIR}/sbin/init
```

### Build an initrd image

```
$ cd $GOPATH/src/github.com/kata-containers/osbuilder/initrd-builder
$ script -fec 'sudo -E AGENT_INIT=yes USE_DOCKER=true ./initrd_builder.sh ${ROOTFS_DIR}'
```

### Install the initrd image

```
$ commit=$(git log --format=%h -1 HEAD)
$ date=$(date +%Y-%m-%d-%T.%N%z)
$ image="kata-containers-initrd-${date}-${commit}"
$ sudo install -o root -g root -m 0640 -D kata-containers-initrd.img "/usr/share/kata-containers/${image}"
$ (cd /usr/share/kata-containers && sudo ln -sf "$image" kata-containers-initrd.img)
```

# Install guest kernel images

```
$ sudo ln -s /usr/share/clear-containers/vmlinux.container /usr/share/kata-containers/
$ sudo ln -s /usr/share/clear-containers/vmlinuz.container /usr/share/kata-containers/
```

> **Note:**
>
> - The files in the previous commands are from the Clear Containers
>   `linux-container` package. See [Assumptions](#assumptions).

If you want to use an initrd image, install the [hyperstart](https://github.com/hyperhq/hyperstart/) kernel instead:
```
$ sudo wget -O /usr/share/kata-containers/vmlinuz.container https://github.com/hyperhq/hyperstart/raw/master/build/arch/x86_64/kernel
```

# Update Docker configuration

```
$ dir=/etc/systemd/system/docker.service.d
$ file="$dir/kata-containers.conf"
$ sudo mkdir -p "$dir"
$ sudo test -e "$file" || echo -e "[Service]\nType=simple\nExecStart=\nExecStart=/usr/bin/dockerd -D --default-runtime runc" | sudo tee "$file"
$ sudo grep -q "kata-runtime=" $file || sudo sed -i 's!^\(ExecStart=[^$].*$\)!\1 --add-runtime kata-runtime=/usr/local/bin/kata-runtime!g' "$file"
$ sudo systemctl daemon-reload
$ sudo systemctl restart docker
```

# Create a Kata Container

```
$ sudo docker run -ti --runtime kata-runtime busybox sh
```

# Troubleshoot Kata Containers

If you are unable to create a Kata Container first ensure you have
[enabled full debug](#enable-full-debug)
before attempting to create a container. Then run the
[`kata-collect-data.sh`](https://github.com/kata-containers/runtime/blob/master/data/kata-collect-data.sh.in)
script and paste its output directly into a
[github issue](https://github.com/kata-containers/kata-containers/issues/new).

> **Note:**
>
> The `kata-collect-data.sh` script is built from the
> [runtime](https://github.com/kata-containers/runtime) repository.

To perform analysis on Kata logs, use the
[`kata-log-parser`](https://github.com/kata-containers/tests/tree/master/cmd/log-parser)
tool.

# Appendices

## Checking Docker default runtime

```
$ sudo docker info 2>/dev/null | grep -i "default runtime" | cut -d: -f2- | grep -q runc  && echo "SUCCESS" || echo "ERROR: Incorrect default Docker runtime"
```
