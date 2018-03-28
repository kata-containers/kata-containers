* [Warning](#warning)
* [Assumptions](#assumptions)
* [Build and install a Kata Containers runtime](#build-and-install-a-kata-containers-runtime)
    * [Check hardware requirements](#check-hardware-requirements)
    * [Enable full debug](#enable-full-debug)
* [Build and install Kata proxy](#build-and-install-kata-proxy)
* [Build and install Kata shim](#build-and-install-kata-shim)
* [Create an image](#create-an-image)
    * [Build a custom Kata agent - OPTIONAL](#build-a-custom-kata-agent---optional)
    * [Get the osbuilder](#get-the-osbuilder)
    * [Create a rootfs image](#create-a-rootfs-image)
        * [Add a custom agent to the image - OPTIONAL](#add-a-custom-agent-to-the-image---optional)
    * [Build the image](#build-the-image)
* [Install the image](#install-the-image)
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

## Enable full debug

Enable full debug as follows:
```
$ sudo sed -i -e 's/^# *\(enable_debug\).*=.*$/\1 = true/g' /usr/share/defaults/kata-containers/configuration.toml
$ sudo sed -i -e 's/^kernel_params = ""/kernel_params = "agent.log=debug"/g' /usr/share/defaults/kata-containers/configuration.toml
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

# Create an image

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

```
$ cd $GOPATH/src/github.com/kata-containers/osbuilder/rootfs-builder
$ script -fec 'sudo -E GOPATH=$GOPATH USE_DOCKER=true ./rootfs.sh clearlinux'
```

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
$ sudo install -o root -g root -m 0550 -t rootfs/bin ../../agent/kata-agent
$ sudo install -o root -g root -m 0440 ../../agent/kata-agent.service rootfs/usr/lib/systemd/system/
$ sudo install -o root -g root -m 0440 ../../agent/kata-containers.target rootfs/usr/lib/systemd/system/
```

## Build the image

```
$ cd $GOPATH/src/github.com/kata-containers/osbuilder/image-builder
$ script -fec 'sudo -E USE_DOCKER=true ./image_builder.sh ../rootfs-builder/rootfs'
```

> **Notes:**
> 
> - You must ensure that the *default Docker runtime* is `runc` to make use of
>   the `USE_DOCKER` variable. If that is not the case, remove the variable
>   from the previous command. See [Checking Docker default runtime](#checking-docker-default-runtime).
> - If you do *not* wish to build under Docker, remove the `USE_DOCKER`
>   variable in the previous command and ensure the `qemu-img` command is
>   available on your system.

# Install the image

```
$ commit=$(git log --format=%h -1 HEAD)
$ date=$(date +%Y-%m-%d-%T.%N%z)
$ image="kata-containers-${date}-${commit}"
$ sudo install -o root -g root -m 0640 -D kata-containers.img "/usr/share/kata-containers/${image}"
$ (cd /usr/share/kata-containers && sudo ln -sf "$image" kata-containers.img)
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

# Update Docker configuration

```
$ dir=/etc/systemd/system/docker.service.d
$ file="$dir/kata-containers.conf"
$ sudo mkdir -p "$dir"
$ sudo test -e "$file" || echo -e "[Service]\nType=simple\nExecStart=\nExecStart=/usr/bin/dockerd -D --default-runtime runc" | sudo tee "$file"
$ sudo sed -i 's!^\(ExecStart=[^$].*$\)!\1 --add-runtime kata-runtime=/usr/local/bin/kata-runtime!g' "$file"
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
