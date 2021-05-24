- [Warning](#warning)
- [Assumptions](#assumptions)
- [Initial setup](#initial-setup)
- [Requirements to build individual components](#requirements-to-build-individual-components)
- [Build and install the Kata Containers runtime](#build-and-install-the-kata-containers-runtime)
- [Check hardware requirements](#check-hardware-requirements)
  - [Configure to use initrd or rootfs image](#configure-to-use-initrd-or-rootfs-image)
  - [Enable full debug](#enable-full-debug)
    - [debug logs and shimv2](#debug-logs-and-shimv2)
      - [Enabling full `containerd` debug](#enabling-full-containerd-debug)
      - [Enabling just `containerd shim` debug](#enabling-just-containerd-shim-debug)
      - [Enabling `CRI-O` and `shimv2` debug](#enabling-cri-o-and-shimv2-debug)
    - [journald rate limiting](#journald-rate-limiting)
      - [`systemd-journald` suppressing messages](#systemd-journald-suppressing-messages)
      - [Disabling `systemd-journald` rate limiting](#disabling-systemd-journald-rate-limiting)
- [Create and install rootfs and initrd image](#create-and-install-rootfs-and-initrd-image)
  - [Build a custom Kata agent - OPTIONAL](#build-a-custom-kata-agent---optional)
  - [Get the osbuilder](#get-the-osbuilder)
  - [Create a rootfs image](#create-a-rootfs-image)
    - [Create a local rootfs](#create-a-local-rootfs)
    - [Add a custom agent to the image - OPTIONAL](#add-a-custom-agent-to-the-image---optional)
    - [Build a rootfs image](#build-a-rootfs-image)
    - [Install the rootfs image](#install-the-rootfs-image)
  - [Create an initrd image - OPTIONAL](#create-an-initrd-image---optional)
    - [Create a local rootfs for initrd image](#create-a-local-rootfs-for-initrd-image)
    - [Build an initrd image](#build-an-initrd-image)
    - [Install the initrd image](#install-the-initrd-image)
- [Install guest kernel images](#install-guest-kernel-images)
- [Install a hypervisor](#install-a-hypervisor)
  - [Build a custom QEMU](#build-a-custom-qemu)
    - [Build a custom QEMU for aarch64/arm64 - REQUIRED](#build-a-custom-qemu-for-aarch64arm64---required)
- [Run Kata Containers with Containerd](#run-kata-containers-with-containerd)
- [Run Kata Containers with Kubernetes](#run-kata-containers-with-kubernetes)
- [Troubleshoot Kata Containers](#troubleshoot-kata-containers)
- [Appendices](#appendices)
  - [Checking Docker default runtime](#checking-docker-default-runtime)
  - [Set up a debug console](#set-up-a-debug-console)
    - [Simple debug console setup](#simple-debug-console-setup)
      - [Enable agent debug console](#enable-agent-debug-console)
      - [Start `kata-monitor` - ONLY NEEDED FOR 2.0.x](#start-kata-monitor---only-needed-for-20x)
      - [Connect to debug console](#connect-to-debug-console)
    - [Traditional debug console setup](#traditional-debug-console-setup)
      - [Create a custom image containing a shell](#create-a-custom-image-containing-a-shell)
      - [Build the debug image](#build-the-debug-image)
      - [Configure runtime for custom debug image](#configure-runtime-for-custom-debug-image)
      - [Create a container](#create-a-container)
      - [Connect to the virtual machine using the debug console](#connect-to-the-virtual-machine-using-the-debug-console)
        - [Enabling debug console for QEMU](#enabling-debug-console-for-qemu)
        - [Enabling debug console for cloud-hypervisor / firecracker](#enabling-debug-console-for-cloud-hypervisor--firecracker)
        - [Connecting to the debug console](#connecting-to-the-debug-console)
  - [Obtain details of the image](#obtain-details-of-the-image)
  - [Capturing kernel boot logs](#capturing-kernel-boot-logs)

# Warning

This document is written **specifically for developers**: it is not intended for end users.

# Assumptions

- You are working on a non-critical test or development system.

# Initial setup

The recommended way to create a development environment is to first
[install the packaged versions of the Kata Containers components](install/README.md)
to create a working system.

The installation guide instructions will install all required Kata Containers
components, plus *Docker*, the hypervisor, and the Kata Containers image and
guest kernel.

# Requirements to build individual components

You need to install the following to build Kata Containers components:

- [golang](https://golang.org/dl)

  To view the versions of go known to work, see the `golang` entry in the
  [versions database](../versions.yaml).

- [rust](https://www.rust-lang.org/tools/install)

  To view the versions of rust known to work, see the `rust` entry in the
  [versions database](../versions.yaml).

- `make`.
- `gcc` (required for building the shim and runtime).

# Build and install the Kata Containers runtime

```
$ go get -d -u github.com/kata-containers/kata-containers
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/src/runtime
$ make && sudo -E PATH=$PATH make install
```

The build will create the following:

- runtime binary: `/usr/local/bin/kata-runtime` and `/usr/local/bin/containerd-shim-kata-v2`
- configuration file: `/usr/share/defaults/kata-containers/configuration.toml`

# Check hardware requirements

You can check if your system is capable of creating a Kata Container by running the following:

```
$ sudo kata-runtime check
```

If your system is *not* able to run Kata Containers, the previous command will error out and explain why.

## Configure to use initrd or rootfs image

Kata containers can run with either an initrd image or a rootfs image.

If you want to test with `initrd`, make sure you have `initrd = /usr/share/kata-containers/kata-containers-initrd.img`
in your configuration file, commenting out the `image` line:

`/usr/share/defaults/kata-containers/configuration.toml` and comment out the `image` line with the following. For example:

```
$ sudo mkdir -p /etc/kata-containers/
$ sudo install -o root -g root -m 0640 /usr/share/defaults/kata-containers/configuration.toml /etc/kata-containers
$ sudo sed -i 's/^\(image =.*\)/# \1/g' /etc/kata-containers/configuration.toml
```
You can create the initrd image as shown in the [create an initrd image](#create-an-initrd-image---optional) section.

If you want to test with a rootfs `image`, make sure you have `image = /usr/share/kata-containers/kata-containers.img`
in your configuration file, commenting out the `initrd` line. For example:

```
$ sudo mkdir -p /etc/kata-containers/
$ sudo install -o root -g root -m 0640 /usr/share/defaults/kata-containers/configuration.toml /etc/kata-containers
$ sudo sed -i 's/^\(initrd =.*\)/# \1/g' /etc/kata-containers/configuration.toml
```
The rootfs image is created as shown in the [create a rootfs image](#create-a-rootfs-image) section.

One of the `initrd` and `image` options in Kata runtime config file **MUST** be set but **not both**.
The main difference between the options is that the size of `initrd`(10MB+) is significantly smaller than
rootfs `image`(100MB+).

## Enable full debug

Enable full debug as follows:

```
$ sudo mkdir -p /etc/kata-containers/
$ sudo install -o root -g root -m 0640 /usr/share/defaults/kata-containers/configuration.toml /etc/kata-containers
$ sudo sed -i -e 's/^# *\(enable_debug\).*=.*$/\1 = true/g' /etc/kata-containers/configuration.toml
$ sudo sed -i -e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 agent.log=debug initcall_debug"/g' /etc/kata-containers/configuration.toml
```

### debug logs and shimv2

If you are using `containerd` and the Kata `containerd-shimv2` to launch Kata Containers, and wish
to enable Kata debug logging, there are two ways this can be enabled via the `containerd` configuration file,
detailed below.

The Kata logs appear in the `containerd` log files, along with logs from `containerd` itself.

For more information about `containerd` debug, please see the
[`containerd` documentation](https://github.com/containerd/containerd/blob/master/docs/getting-started.md).

#### Enabling full `containerd` debug

Enabling full `containerd` debug also enables the shimv2 debug. Edit the `containerd` configuration file
to include the top level debug option such as:

```toml
[debug]
        level = "debug"
```

#### Enabling just `containerd shim` debug

If you only wish to enable debug for the `containerd` shims themselves, just enable the debug
option in the `plugins.linux` section of the `containerd` configuration file, such as:

```toml
  [plugins.linux]
    shim_debug = true
```

#### Enabling `CRI-O` and `shimv2` debug

Depending on the CRI-O version being used one of the following configuration files can
be found: `/etc/crio/crio.conf` or `/etc/crio/crio.conf.d/00-default`.

If the latter is found, the change must be done there as it'll take precedence, overriding
`/etc/crio/crio.conf`.

```toml
# Changes the verbosity of the logs based on the level it is set to. Options
# are fatal, panic, error, warn, info, debug and trace. This option supports
# live configuration reload.
log_level = "info"
```

Switching the default `log_level` from `info` to `debug` enables shimv2 debug logs.
CRI-O logs can be found by using the `crio` identifier, and Kata specific logs can
be found by using the `kata` identifier.

### journald rate limiting

Enabling [full debug](#enable-full-debug) results in the Kata components generating
large amounts of logging, which by default is stored in the system log. Depending on
your system configuration, it is possible that some events might be discarded by the
system logging daemon. The following shows how to determine this for `systemd-journald`,
and offers possible workarounds and fixes.

> **Note** The method of implementation can vary between Operating System installations.
> Amend these instructions as necessary to your system implementation,
> and consult with your system administrator for the appropriate configuration.

#### `systemd-journald` suppressing messages

`systemd-journald` can be configured to rate limit the number of journal entries
it stores. When messages are suppressed, it is noted in the logs. This can be checked
for by looking for those notifications, such as:

```sh
$ sudo journalctl --since today | fgrep Suppressed
Jun 29 14:51:17 mymachine systemd-journald[346]: Suppressed 4150 messages from /system.slice/docker.service
```

This message indicates that a number of log messages from the `docker.service` slice were
suppressed. In such a case, you can expect to have incomplete logging information
stored from the Kata Containers components.

#### Disabling `systemd-journald` rate limiting

In order to capture complete logs from the Kata Containers components, you
need to reduce or disable the `systemd-journald` rate limit. Configure
this at the global `systemd-journald` level, and it will apply to all system slices.

To disable `systemd-journald` rate limiting at the global level, edit the file
`/etc/systemd/journald.conf`, and add/uncomment the following lines:

```
RateLimitInterval=0s
RateLimitBurst=0
```

Restart `systemd-journald` for the changes to take effect:

```sh
$ sudo systemctl restart systemd-journald
```

# Create and install rootfs and initrd image

## Build a custom Kata agent - OPTIONAL

> **Note:**
>
> - You should only do this step if you are testing with the latest version of the agent.

The rust-agent is built with a static linked `musl.` To configure this:

```
rustup target add x86_64-unknown-linux-musl
sudo ln -s /usr/bin/g++ /bin/musl-g++
```

To build the agent:

```
$ go get -d -u github.com/kata-containers/kata-containers
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/src/agent && make
```

## Get the osbuilder

```
$ go get -d -u github.com/kata-containers/kata-containers
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/osbuilder
```

## Create a rootfs image
### Create a local rootfs

As a prerequisite, you need to install Docker. Otherwise, you will not be
able to run the `rootfs.sh` script with `USE_DOCKER=true` as expected in
the following example.

```
$ export ROOTFS_DIR=${GOPATH}/src/github.com/kata-containers/kata-containers/tools/osbuilder/rootfs-builder/rootfs
$ sudo rm -rf ${ROOTFS_DIR}
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/osbuilder/rootfs-builder
$ script -fec 'sudo -E GOPATH=$GOPATH USE_DOCKER=true SECCOMP=no ./rootfs.sh ${distro}'
```
You MUST choose one of `alpine`, `centos`, `clearlinux`, `debian`, `euleros`, `fedora`, `suse`, and `ubuntu` for `${distro}`. By default `seccomp` packages are not included in the rootfs image. Set `SECCOMP` to `yes` to include them.

> **Note:**
>
> - Check the [compatibility matrix](../tools/osbuilder/README.md#platform-distro-compatibility-matrix) before creating rootfs.
> - You must ensure that the *default Docker runtime* is `runc` to make use of
>   the `USE_DOCKER` variable. If that is not the case, remove the variable
>   from the previous command. See [Checking Docker default runtime](#checking-docker-default-runtime).

### Add a custom agent to the image - OPTIONAL

> **Note:**
>
> - You should only do this step if you are testing with the latest version of the agent.

```
$ sudo install -o root -g root -m 0550 -t ${ROOTFS_DIR}/usr/bin ../../../src/agent/target/x86_64-unknown-linux-musl/release/kata-agent
$ sudo install -o root -g root -m 0440 ../../../src/agent/kata-agent.service ${ROOTFS_DIR}/usr/lib/systemd/system/
$ sudo install -o root -g root -m 0440 ../../../src/agent/kata-containers.target ${ROOTFS_DIR}/usr/lib/systemd/system/
```

### Build a rootfs image

```
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/osbuilder/image-builder
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
$ export ROOTFS_DIR="${GOPATH}/src/github.com/kata-containers/kata-containers/tools/osbuilder/rootfs-builder/rootfs"
$ sudo rm -rf ${ROOTFS_DIR}
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/osbuilder/rootfs-builder
$ script -fec 'sudo -E GOPATH=$GOPATH AGENT_INIT=yes USE_DOCKER=true SECCOMP=no ./rootfs.sh ${distro}'
```
`AGENT_INIT` controls if the guest image uses the Kata agent as the guest `init` process. When you create an initrd image,
always set `AGENT_INIT` to `yes`. By default `seccomp` packages are not included in the initrd image. Set `SECCOMP` to `yes` to include them.

You MUST choose one of `alpine`, `centos`, `clearlinux`, `euleros`, and `fedora` for `${distro}`.

> **Note:**
>
> - Check the [compatibility matrix](../tools/osbuilder/README.md#platform-distro-compatibility-matrix) before creating rootfs.

Optionally, add your custom agent binary to the rootfs with the following, `LIBC` default is `musl`, if `ARCH` is `ppc64le`, should set the `LIBC=gnu` and `ARCH=powerpc64le`:
```
$ export ARCH=$(shell uname -m)
$ [ ${ARCH} == "ppc64le" ] && export LIBC=gnu || export LIBC=musl
$ [ ${ARCH} == "ppc64le" ] && export ARCH=powerpc64le
$ sudo install -o root -g root -m 0550 -T ../../../src/agent/target/$(ARCH)-unknown-linux-$(LIBC)/release/kata-agent ${ROOTFS_DIR}/sbin/init
```

### Build an initrd image

```
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/osbuilder/initrd-builder
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

You can build and install the guest kernel image as shown [here](../tools/packaging/kernel/README.md#build-kata-containers-kernel).

# Install a hypervisor

When setting up Kata using a [packaged installation method](install/README.md#installing-on-a-linux-system), the
`QEMU` VMM is installed automatically. Cloud-Hypervisor and Firecracker VMMs are available from the [release tarballs](https://github.com/kata-containers/kata-containers/releases), as well as through [`kata-deploy`](../tools/packaging/kata-deploy/README.md).
You may choose to manually build your VMM/hypervisor.

## Build a custom QEMU

Kata Containers makes use of upstream QEMU branch. The exact version
and repository utilized can be found by looking at the [versions file](../versions.yaml).

Find the correct version of QEMU from the versions file:
```
$ source ${GOPATH}/src/github.com/kata-containers/kata-containers/tools/packaging/scripts/lib.sh
$ qemu_version=$(get_from_kata_deps "assets.hypervisor.qemu.version")
$ echo ${qemu_version}
```
Get source from the matching branch of QEMU:
```
$ go get -d github.com/qemu/qemu
$ cd ${GOPATH}/src/github.com/qemu/qemu
$ git checkout ${qemu_version}
$ your_qemu_directory=${GOPATH}/src/github.com/qemu/qemu
```

There are scripts to manage the build and packaging of QEMU. For the examples below, set your
environment as:
```
$ go get -d github.com/kata-containers/kata-containers
$ packaging_dir="${GOPATH}/src/github.com/kata-containers/kata-containers/tools/packaging"
```

Kata often utilizes patches for not-yet-upstream and/or backported fixes for components,
including QEMU. These can be found in the [packaging/QEMU directory](../tools/packaging/qemu/patches),
and it's *recommended* that you apply them. For example, suppose that you are going to build QEMU
version 5.2.0, do:
```
$ cd $your_qemu_directory
$ $packaging_dir/scripts/apply_patches.sh $packaging_dir/qemu/patches/5.2.x/
```

To build utilizing the same options as Kata, you should make use of the `configure-hypervisor.sh` script. For example:
```
$ cd $your_qemu_directory
$ $packaging_dir/scripts/configure-hypervisor.sh kata-qemu > kata.cfg
$ eval ./configure "$(cat kata.cfg)"
$ make -j $(nproc)
$ sudo -E make install
```

See the [static-build script for QEMU](../tools/packaging/static-build/qemu/build-static-qemu.sh) for a reference on how to get, setup, configure and build QEMU for Kata.

### Build a custom QEMU for aarch64/arm64 - REQUIRED
> **Note:**
>
> - You should only do this step if you are on aarch64/arm64.
> - You should include [Eric Auger's latest PCDIMM/NVDIMM patches](https://patchwork.kernel.org/cover/10647305/) which are
>   under upstream review for supporting NVDIMM on aarch64.
>
You could build the custom `qemu-system-aarch64` as required with the following command:
```
$ go get -d github.com/kata-containers/tests
$ script -fec 'sudo -E ${GOPATH}/src/github.com/kata-containers/tests/.ci/install_qemu.sh'
```

# Run Kata Containers with Containerd
Refer to the [How to use Kata Containers and Containerd](how-to/containerd-kata.md) how-to guide.

# Run Kata Containers with Kubernetes
Refer to the [Run Kata Containers with Kubernetes](how-to/run-kata-with-k8s.md) how-to guide.

# Troubleshoot Kata Containers

If you are unable to create a Kata Container first ensure you have
[enabled full debug](#enable-full-debug)
before attempting to create a container. Then run the
[`kata-collect-data.sh`](../src/runtime/data/kata-collect-data.sh.in)
script and paste its output directly into a
[GitHub issue](https://github.com/kata-containers/kata-containers/issues/new).

> **Note:**
>
> The `kata-collect-data.sh` script is built from the
> [runtime](../src/runtime) repository.

To perform analysis on Kata logs, use the
[`kata-log-parser`](https://github.com/kata-containers/tests/tree/master/cmd/log-parser)
tool, which can convert the logs into formats (e.g. JSON, TOML, XML, and YAML).

See [Set up a debug console](#set-up-a-debug-console).

# Appendices

## Checking Docker default runtime

```
$ sudo docker info 2>/dev/null | grep -i "default runtime" | cut -d: -f2- | grep -q runc  && echo "SUCCESS" || echo "ERROR: Incorrect default Docker runtime"
```
## Set up a debug console

Kata containers provides two ways to connect to the guest. One is using traditional login service, which needs additional works. In contrast the simple debug console is easy to setup.

### Simple debug console setup

Kata Containers 2.0 supports a shell simulated *console* for quick debug purpose. This approach uses VSOCK to
connect to the shell running inside the guest which the agent starts. This method only requires the guest image to
contain either `/bin/sh` or `/bin/bash`.

#### Enable agent debug console

Enable debug_console_enabled in the `configuration.toml` configuration file:

```
[agent.kata]
debug_console_enabled = true
```

This will pass `agent.debug_console agent.debug_console_vport=1026` to agent as kernel parameters, and sandboxes created using this parameters will start a shell in guest if new connection is accept from VSOCK.

#### Start `kata-monitor` - ONLY NEEDED FOR 2.0.x

For Kata Containers `2.0.x` releases, the `kata-runtime exec` command depends on the`kata-monitor` running, in order to get the sandbox's `vsock` address to connect to. Thus, first start the `kata-monitor` process.

```
$ sudo kata-monitor
```

`kata-monitor` will serve at `localhost:8090` by default.

#### Connect to debug console

Command `kata-runtime exec` is used to connect to the debug console.

```
$ kata-runtime exec 1a9ab65be63b8b03dfd0c75036d27f0ed09eab38abb45337fea83acd3cd7bacd
bash-4.2# id
uid=0(root) gid=0(root) groups=0(root)
bash-4.2# pwd
/
bash-4.2# exit
exit
```

`kata-runtime exec` has a command-line option `runtime-namespace`, which is used to specify under which [runtime namespace](https://github.com/containerd/containerd/blob/master/docs/namespaces.md) the particular pod was created. By default, it is set to `k8s.io` and works for containerd when configured
 with Kubernetes. For CRI-O, the namespace should set to `default` explicitly. This should not be confused with [Kubernetes namespaces](https://kubernetes.io/docs/concepts/overview/working-with-objects/namespaces/).
For other CRI-runtimes and configurations, you may need to set the namespace utilizing the `runtime-namespace` option.

If you want to access guest OS through a traditional way, see [Traditional debug console setup)](#traditional-debug-console-setup).

### Traditional debug console setup

By default you cannot login to a virtual machine, since this can be sensitive
from a security perspective. Also, allowing logins would require additional
packages in the rootfs, which would increase the size of the image used to
boot the virtual machine.

If you want to login to a virtual machine that hosts your containers, complete
the following steps (using rootfs or initrd image).

> **Note:** The following debug console instructions assume a systemd-based guest
> O/S image. This means you must create a rootfs for a distro that supports systemd.
> Currently, all distros supported by [osbuilder](../tools/osbuilder) support systemd
> except for Alpine Linux.
>
> Look for `INIT_PROCESS=systemd` in the `config.sh` osbuilder rootfs config file
> to verify an osbuilder distro supports systemd for the distro you want to build rootfs for.
> For an example, see the [Clear Linux config.sh file](../tools/osbuilder/rootfs-builder/clearlinux/config.sh).
>
> For a non-systemd-based distro, create an equivalent system
> service using that distro’s init system syntax. Alternatively, you can build a distro
> that contains a shell (e.g. `bash(1)`). In this circumstance it is likely you need to install
> additional packages in the rootfs and add “agent.debug_console” to kernel parameters in the runtime
> config file. This tells the Kata agent to launch the console directly.
>
> Once these steps are taken you can connect to the virtual machine using the [debug console](Developer-Guide.md#connect-to-the-virtual-machine-using-the-debug-console).

#### Create a custom image containing a shell

To login to a virtual machine, you must
[create a custom rootfs](#create-a-rootfs-image) or [custom initrd](#create-an-initrd-image---optional)
containing a shell such as `bash(1)`. For Clear Linux, you will need
an additional `coreutils` package.

For example using CentOS:

```
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/osbuilder/rootfs-builder
$ export ROOTFS_DIR=${GOPATH}/src/github.com/kata-containers/kata-containers/tools/osbuilder/rootfs-builder/rootfs
$ script -fec 'sudo -E GOPATH=$GOPATH USE_DOCKER=true EXTRA_PKGS="bash coreutils" ./rootfs.sh centos'
```

#### Build the debug image

Follow the instructions in the [Build a rootfs image](#build-a-rootfs-image)
section when using rootfs, or when using initrd, complete the steps in the [Build an initrd image](#build-an-initrd-image) section.

#### Configure runtime for custom debug image

Install the image:

>**Note**: When using an initrd image, replace the below rootfs image name `kata-containers.img` 
>with the initrd image name `kata-containers-initrd.img`.

```
$ name="kata-containers-centos-with-debug-console.img"
$ sudo install -o root -g root -m 0640 kata-containers.img "/usr/share/kata-containers/${name}"
```

Next, modify the `image=` values in the `[hypervisor.qemu]` section of the
[configuration file](../src/runtime/README.md#configuration)
to specify the full path to the image name specified in the previous code
section. Alternatively, recreate the symbolic link so it points to
the new debug image:

```
$ (cd /usr/share/kata-containers && sudo ln -sf "$name" kata-containers.img)
```

**Note**: You should take care to undo this change after you finish debugging
to avoid all subsequently created containers from using the debug image.

#### Create a container

Create a container as normal. For example using `crictl`:

```
$ sudo crictl run -r kata container.yaml pod.yaml
```

#### Connect to the virtual machine using the debug console

The steps required to enable debug console for QEMU slightly differ with
those for firecracker / cloud-hypervisor.
 
##### Enabling debug console for QEMU

Add `agent.debug_console` to the guest kernel command line to allow the agent process to start a debug console. 

```
$ sudo sed -i -e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 agent.debug_console"/g' "${kata_configuration_file}"
```

Here `kata_configuration_file` could point to `/etc/kata-containers/configuration.toml` 
or `/usr/share/defaults/kata-containers/configuration.toml`
or `/opt/kata/share/defaults/kata-containers/configuration-{hypervisor}.toml`, if
you installed Kata Containers using `kata-deploy`.

##### Enabling debug console for cloud-hypervisor / firecracker

Slightly different configuration is required in case of firecracker and cloud hypervisor. 
Firecracker and cloud-hypervisor don't have a UNIX socket connected to `/dev/console`. 
Hence, the kernel command line option `agent.debug_console` will not work for them. 
These hypervisors support `hybrid vsocks`,  which can be used for communication
between the host and the guest. The kernel command line option `agent.debug_console_vport`
 was added to allow developers specify on which `vsock` port the debugging console should be connected.


Add the parameter `agent.debug_console_vport=1026` to the kernel command line
as shown below:
```
sudo sed -i -e 's/^kernel_params = "\(.*\)"/kernel_params = "\1 agent.debug_console_vport=1026"/g' "${kata_configuration_file}"
```

> **Note** Ports 1024 and 1025 are reserved for communication with the agent
> and gathering of agent logs respectively. 

##### Connecting to the debug console

Next, connect to the debug console. The VSOCKS paths vary slightly between each
VMM solution.

In case of cloud-hypervisor, connect to the `vsock` as shown:
```
$ sudo su -c 'cd /var/run/vc/vm/{sandbox_id}/root/ && socat stdin unix-connect:clh.sock'
CONNECT 1026
```

**Note**: You need to type `CONNECT 1026` and press `RETURN` key after entering the `socat` command.

For firecracker, connect to the `hvsock` as shown:
```
$ sudo su -c 'cd /var/run/vc/firecracker/{sandbox_id}/root/ && socat stdin unix-connect:kata.hvsock'
CONNECT 1026
```

**Note**: You need to press the `RETURN` key to see the shell prompt.


For QEMU, connect to the `vsock` as shown:
```
$ sudo su -c 'cd /var/run/vc/vm/{sandbox_id} && socat "stdin,raw,echo=0,escape=0x11" "unix-connect:console.sock"
```

To disconnect from the virtual machine, type `CONTROL+q` (hold down the
`CONTROL` key and press `q`).

## Obtain details of the image

If the image is created using
[osbuilder](../tools/osbuilder), the following YAML
file exists and contains details of the image and how it was created:

```
$ cat /var/lib/osbuilder/osbuilder.yaml
```

## Capturing kernel boot logs

Sometimes it is useful to capture the kernel boot messages from a Kata Container
launch. If the container launches to the point whereby you can `exec` into it, and
if the container has the necessary components installed, often you can execute the `dmesg`
command inside the container to view the kernel boot logs.

If however you are unable to `exec` into the container, you can enable some debug
options to have the kernel boot messages logged into the system journal.

- Set `enable_debug = true` in the `[hypervisor.qemu]` and `[runtime]` sections

For generic information on enabling debug in the configuration file, see the
[Enable full debug](#enable-full-debug) section.

The kernel boot messages will appear in the `containerd` or `CRI-O` log appropriately,
such as:

```bash
$ sudo journalctl -t containerd
-- Logs begin at Thu 2020-02-13 16:20:40 UTC, end at Thu 2020-02-13 16:30:23 UTC. --
...
time="2020-09-15T14:56:23.095113803+08:00" level=debug msg="reading guest console" console-protocol=unix console-url=/run/vc/vm/ab9f633385d4987828d342e47554fc6442445b32039023eeddaa971c1bb56791/console.sock pid=107642 sandbox=ab9f633385d4987828d342e47554fc6442445b32039023eeddaa971c1bb56791 source=virtcontainers subsystem=sandbox vmconsole="[    0.395399] brd: module loaded"
time="2020-09-15T14:56:23.102633107+08:00" level=debug msg="reading guest console" console-protocol=unix console-url=/run/vc/vm/ab9f633385d4987828d342e47554fc6442445b32039023eeddaa971c1bb56791/console.sock pid=107642 sandbox=ab9f633385d4987828d342e47554fc6442445b32039023eeddaa971c1bb56791 source=virtcontainers subsystem=sandbox vmconsole="[    0.402845] random: fast init done"
time="2020-09-15T14:56:23.103125469+08:00" level=debug msg="reading guest console" console-protocol=unix console-url=/run/vc/vm/ab9f633385d4987828d342e47554fc6442445b32039023eeddaa971c1bb56791/console.sock pid=107642 sandbox=ab9f633385d4987828d342e47554fc6442445b32039023eeddaa971c1bb56791 source=virtcontainers subsystem=sandbox vmconsole="[    0.403544] random: crng init done"
time="2020-09-15T14:56:23.105268162+08:00" level=debug msg="reading guest console" console-protocol=unix console-url=/run/vc/vm/ab9f633385d4987828d342e47554fc6442445b32039023eeddaa971c1bb56791/console.sock pid=107642 sandbox=ab9f633385d4987828d342e47554fc6442445b32039023eeddaa971c1bb56791 source=virtcontainers subsystem=sandbox vmconsole="[    0.405599] loop: module loaded"
time="2020-09-15T14:56:23.121121598+08:00" level=debug msg="reading guest console" console-protocol=unix console-url=/run/vc/vm/ab9f633385d4987828d342e47554fc6442445b32039023eeddaa971c1bb56791/console.sock pid=107642 sandbox=ab9f633385d4987828d342e47554fc6442445b32039023eeddaa971c1bb56791 source=virtcontainers subsystem=sandbox vmconsole="[    0.421324] memmap_init_zone_device initialised 32768 pages in 12ms"
...
```
