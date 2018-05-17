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

- You have installed the `qemu-lite` package containing the hypervisor. This package
  is automatically installed when you install Clear Containers, but can be
  installed separately as well:

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

If your system is *not* able to run Kata Containers, the previous command will error out and explain why.

## Configure to use initrd or rootfs image

Kata containers can run with either an initrd image or a rootfs image.

If you want to test with `initrd`, make sure you have `initrd = /usr/share/kata-containers/kata-containers-initrd.img`
in `/usr/share/defaults/kata-containers/configuration.toml` and comment out the `image` line with the following:

```
$ sudo sed -i 's/^\(image =.*\)/# \1/g' /usr/share/defaults/kata-containers/configuration.toml
```
You can create the initrd image as shown in the [create an initrd image](#create-an-initrd-image---optional) section.

If you want to test with a rootfs `image`, make sure you have `image = /usr/share/kata-containers/kata-containers.img`
in `/usr/share/defaults/kata-containers/configuration.toml` and comment out the `initrd` line with the following:

```
$ sudo sed -i 's/^\(initrd =.*\)/# \1/g' /usr/share/defaults/kata-containers/configuration.toml
```
The rootfs image is created as shown in the [create a rootfs image](#create-a-rootfs-image) section.

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
> - You should only do this step if you are testing with the latest version of the agent.

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
> - You should only do this step if you are testing with the latest version of the agent.

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
$ kernel_arch="$(arch)"
$ tmpdir="$(mktemp -d)"
$ pushd "$tmpdir"
$ curl -L https://raw.githubusercontent.com/kata-containers/packaging/master/kernel/configs/x86_kata_kvm_4.14.x -o .config
$ kernel_version=$(grep "Linux/[${kernel_arch}]*" .config | cut -d' ' -f3 | tail -1)
$ kernel_tar_file="linux-${kernel_version}.tar.xz"
$ kernel_url="https://cdn.kernel.org/pub/linux/kernel/v$(echo $kernel_version | cut -f1 -d.).x/${kernel_tar_file}"
$ curl -LOk ${kernel_url}
$ tar -xf ${kernel_tar_file}
$ mv .config "linux-${kernel_version}"
$ pushd "linux-${kernel_version}"
$ curl -L https://raw.githubusercontent.com/kata-containers/packaging/master/kernel/patches/0001-NO-UPSTREAM-9P-always-use-cached-inode-to-fill-in-v9.patch | patch -p1
$ make ARCH=${kernel_arch} -j$(nproc)
$ kata_kernel_dir="/usr/share/kata-containers"
$ kata_vmlinuz="${kata_kernel_dir}/kata-vmlinuz-${kernel_version}.container"
$ sudo install -o root -g root -m 0755 -D "$(realpath arch/${kernel_arch}/boot/bzImage)" "${kata_vmlinuz}"
$ sudo ln -sf "${kata_vmlinuz}" "${kata_kernel_dir}/vmlinuz.container"
$ kata_vmlinux="${kata_kernel_dir}/kata-vmlinux-${kernel_version}"
$ sudo install -o root -g root -m 0755 -D "$(realpath vmlinux)" "${kata_vmlinux}"
$ sudo ln -sf "${kata_vmlinux}" "${kata_kernel_dir}/vmlinux.container"
$ popd
$ popd
$ rm -rf "${tmpdir}"
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
tool, which can convert the logs into formats (e.g. JSON, TOML, XML, and YAML).

To obtain a full backtrace for the agent, proxy, runtime, or shim send the
`SIGUSR1` signal to the process ID of the component. The component will send a
backtrace to the system log on the host system and continue to run without
interruption.

For example, to obtain a backtrace for `kata-proxy`:

```
$ sudo kill -USR1 $kata_proxy_pid
$ sudo journalctl -t kata-proxy
```

# Appendices

## Checking Docker default runtime

```
$ sudo docker info 2>/dev/null | grep -i "default runtime" | cut -d: -f2- | grep -q runc  && echo "SUCCESS" || echo "ERROR: Incorrect default Docker runtime"
```
