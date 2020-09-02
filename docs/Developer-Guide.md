* [Warning](#warning)
* [Assumptions](#assumptions)
* [Initial setup](#initial-setup)
* [Requirements to build individual components](#requirements-to-build-individual-components)
* [Build and install the Kata Containers runtime](#build-and-install-the-kata-containers-runtime)
* [Check hardware requirements](#check-hardware-requirements)
    * [Configure to use initrd or rootfs image](#configure-to-use-initrd-or-rootfs-image)
    * [Enable full debug](#enable-full-debug)
        * [debug logs and shimv2](#debug-logs-and-shimv2)
            * [Enabling full `containerd` debug](#enabling-full-containerd-debug)
            * [Enabling just `containerd shim` debug](#enabling-just-containerd-shim-debug)
            * [Enabling `CRI-O` and `shimv2` debug](#enabling-cri-o-and-shimv2-debug)
        * [journald rate limiting](#journald-rate-limiting)
            * [`systemd-journald` suppressing messages](#systemd-journald-suppressing-messages)
            * [Disabling `systemd-journald` rate limiting](#disabling-systemd-journald-rate-limiting)
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
* [Install a hypervisor](#install-a-hypervisor)
    * [Build a custom QEMU](#build-a-custom-qemu)
        * [Build a custom QEMU for aarch64/arm64 - REQUIRED](#build-a-custom-qemu-for-aarch64arm64---required)
* [Run Kata Containers with Docker](#run-kata-containers-with-docker)
    * [Update the Docker systemd unit file](#update-the-docker-systemd-unit-file)
    * [Create a container using Kata](#create-a-container-using-kata)
* [Run Kata Containers with Kubernetes](#run-kata-containers-with-kubernetes)
* [Troubleshoot Kata Containers](#troubleshoot-kata-containers)
* [Appendices](#appendices)
    * [Checking Docker default runtime](#checking-docker-default-runtime)
    * [Set up a debug console](#set-up-a-debug-console)
        * [Create a custom image containing a shell](#create-a-custom-image-containing-a-shell)
        * [Create a debug systemd service](#create-a-debug-systemd-service)
        * [Build the debug image](#build-the-debug-image)
        * [Configure runtime for custom debug image](#configure-runtime-for-custom-debug-image)
        * [Ensure debug options are valid](#ensure-debug-options-are-valid)
        * [Create a container](#create-a-container)
        * [Connect to the virtual machine using the debug console](#connect-to-the-virtual-machine-using-the-debug-console)
        * [Obtain details of the image](#obtain-details-of-the-image)
    * [Capturing kernel boot logs](#capturing-kernel-boot-logs)
    * [Running standalone](#running-standalone)
        * [Create an OCI bundle](#create-an-oci-bundle)
        * [Launch the runtime to create a container](#launch-the-runtime-to-create-a-container)

# Warning

This document is written **specifically for developers**: it is not intended for end users.

# Assumptions

- You are working on a non-critical test or development system.

# Initial setup

The recommended way to create a development environment is to first
[install the packaged versions of the Kata Containers components](install/README.md)
to create a working system.

The installation guide instructions will install all required Kata Containers
components, plus Docker*, the hypervisor, and the Kata Containers image and
guest kernel.

# Requirements to build individual components

You need to install the following to build Kata Containers components:

- [golang](https://golang.org/dl)

  To view the versions of go known to work, see the `golang` entry in the
  [versions database](https://github.com/kata-containers/runtime/blob/master/versions.yaml).

- `make`.
- `gcc` (required for building the shim and runtime).

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

As a prerequisite, you need to install Docker. Otherwise, you will not be
able to run the `rootfs.sh` script with `USE_DOCKER=true` as expected in
the following example.

```
$ export ROOTFS_DIR=${GOPATH}/src/github.com/kata-containers/osbuilder/rootfs-builder/rootfs
$ sudo rm -rf ${ROOTFS_DIR}
$ cd $GOPATH/src/github.com/kata-containers/osbuilder/rootfs-builder
$ script -fec 'sudo -E GOPATH=$GOPATH USE_DOCKER=true SECCOMP=no ./rootfs.sh ${distro}'
```
You MUST choose one of `alpine`, `centos`, `clearlinux`, `debian`, `euleros`, `fedora`, `suse`, and `ubuntu` for `${distro}`. By default `seccomp` packages are not included in the rootfs image. Set `SECCOMP` to `yes` to include them.

> **Note:**
>
> - Check the [compatibility matrix](https://github.com/kata-containers/osbuilder#platform-distro-compatibility-matrix) before creating rootfs.
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
$ sudo rm -rf ${ROOTFS_DIR}
$ cd $GOPATH/src/github.com/kata-containers/osbuilder/rootfs-builder
$ script -fec 'sudo -E GOPATH=$GOPATH AGENT_INIT=yes USE_DOCKER=true SECCOMP=no ./rootfs.sh ${distro}'
```
`AGENT_INIT` controls if the guest image uses the Kata agent as the guest `init` process. When you create an initrd image,
always set `AGENT_INIT` to `yes`. By default `seccomp` packages are not included in the initrd image. Set `SECCOMP` to `yes` to include them.

You MUST choose one of `alpine`, `centos`, `clearlinux`, `euleros`, and `fedora` for `${distro}`.

> **Note:**
>
> - Check the [compatibility matrix](https://github.com/kata-containers/osbuilder#platform-distro-compatibility-matrix) before creating rootfs.

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

You can build and install the guest kernel image as shown [here](../tools/packaging/kernel/README.md#build-kata-containers-kernel).

# Install a hypervisor

When setting up Kata using a [packaged installation method](install/README.md#installing-on-a-linux-system), the `qemu-lite` hypervisor is installed automatically. For other installation methods, you will need to manually install a suitable hypervisor.

## Build a custom QEMU

Your QEMU directory need to be prepared with source code. Alternatively, you can use the [Kata containers QEMU](https://github.com/kata-containers/qemu/tree/master) and checkout the recommended branch:

```
$ go get -d github.com/kata-containers/qemu
$ qemu_branch=$(grep qemu-lite- ${GOPATH}/src/github.com/kata-containers/runtime/versions.yaml | cut -d '"' -f2)
$ cd ${GOPATH}/src/github.com/kata-containers/qemu
$ git checkout -b $qemu_branch remotes/origin/$qemu_branch
$ your_qemu_directory=${GOPATH}/src/github.com/kata-containers/qemu
```

To build a version of QEMU using the same options as the default `qemu-lite` version , you could use the `configure-hypervisor.sh` script:

```
$ go get -d github.com/kata-containers/packaging
$ cd $your_qemu_directory
$ ${GOPATH}/src/github.com/kata-containers/packaging/scripts/configure-hypervisor.sh qemu > kata.cfg
$ eval ./configure "$(cat kata.cfg)"
$ make -j $(nproc)
$ sudo -E make install
```

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

# Run Kata Containers with Docker

## Update the Docker systemd unit file

```
$ dockerUnit=$(systemctl show -p FragmentPath docker.service | cut -d "=" -f 2)
$ unitFile=${dockerUnit:-/etc/systemd/system/docker.service.d/kata-containers.conf}
$ test -e "$unitFile" || { sudo mkdir -p "$(dirname $unitFile)"; echo -e "[Service]\nType=simple\nExecStart=\nExecStart=/usr/bin/dockerd -D --default-runtime runc" | sudo tee "$unitFile"; }
$ grep -q "kata-runtime=" $unitFile || sudo sed -i 's!^\(ExecStart=[^$].*$\)!\1 --add-runtime kata-runtime=/usr/local/bin/kata-runtime!g' "$unitFile"
$ sudo systemctl daemon-reload
$ sudo systemctl restart docker
```

## Create a container using Kata

```
$ sudo docker run -ti --runtime kata-runtime busybox sh
```

# Run Kata Containers with Kubernetes
Refer to to the [Run Kata Containers with Kubernetes](how-to/run-kata-with-k8s.md) how-to guide.

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

To obtain a full backtrace for the agent, proxy, runtime, or shim send the
`SIGUSR1` signal to the process ID of the component. The component will send a
backtrace to the system log on the host system and continue to run without
interruption.

For example, to obtain a backtrace for `kata-proxy`:

```
$ sudo kill -USR1 $kata_proxy_pid
$ sudo journalctl -t kata-proxy
```

See [Set up a debug console](#set-up-a-debug-console).

# Appendices

## Checking Docker default runtime

```
$ sudo docker info 2>/dev/null | grep -i "default runtime" | cut -d: -f2- | grep -q runc  && echo "SUCCESS" || echo "ERROR: Incorrect default Docker runtime"
```

## Set up a debug console

By default you cannot login to a virtual machine, since this can be sensitive
from a security perspective. Also, allowing logins would require additional
packages in the rootfs, which would increase the size of the image used to
boot the virtual machine.

If you want to login to a virtual machine that hosts your containers, complete
the following steps (using rootfs or initrd image).

> **Note:** The following debug console instructions assume a systemd-based guest
> O/S image. This means you must create a rootfs for a distro that supports systemd.
> Currently, all distros supported by [osbuilder](https://github.com/kata-containers/osbuilder) support systemd
> except for Alpine Linux.
>
> Look for `INIT_PROCESS=systemd` in the `config.sh` osbuilder rootfs config file
> to verify an osbuilder distro supports systemd for the distro you want to build rootfs for.
> For an example, see the [Clear Linux config.sh file](https://github.com/kata-containers/osbuilder/blob/master/rootfs-builder/clearlinux/config.sh).
>
> For a non-systemd-based distro, create an equivalent system
> service using that distro’s init system syntax. Alternatively, you can build a distro
> that contains a shell (e.g. `bash(1)`). In this circumstance it is likely you need to install
> additional packages in the rootfs and add “agent.debug_console” to kernel parameters in the runtime
> config file. This tells the Kata agent to launch the console directly.
>
> Once these steps are taken you can connect to the virtual machine using the [debug console](Developer-Guide.md#connect-to-the-virtual-machine-using-the-debug-console).

### Create a custom image containing a shell

To login to a virtual machine, you must
[create a custom rootfs](#create-a-rootfs-image) or [custom initrd](#create-an-initrd-image---optional)
containing a shell such as `bash(1)`. For Clear Linux, you will need
an additional `coreutils` package.

For example using CentOS:

```
$ cd $GOPATH/src/github.com/kata-containers/osbuilder/rootfs-builder
$ export ROOTFS_DIR=${GOPATH}/src/github.com/kata-containers/osbuilder/rootfs-builder/rootfs
$ script -fec 'sudo -E GOPATH=$GOPATH USE_DOCKER=true EXTRA_PKGS="bash coreutils" ./rootfs.sh centos'
```

### Create a debug systemd service

Create the service file that starts the shell in the rootfs directory:

```
$ cat <<EOT | sudo tee ${ROOTFS_DIR}/lib/systemd/system/kata-debug.service
[Unit]
Description=Kata Containers debug console

[Service]
Environment=PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
StandardInput=tty
StandardOutput=tty
# Must be disabled to allow the job to access the real console
PrivateDevices=no
Type=simple
ExecStart=/bin/bash
Restart=always
EOT
```

**Note**: You might need to adjust the `ExecStart=` path.

Add a dependency to start the debug console:

```
$ sudo sed -i '$a Requires=kata-debug.service' ${ROOTFS_DIR}/lib/systemd/system/kata-containers.target
```

### Build the debug image

Follow the instructions in the [Build a rootfs image](#build-a-rootfs-image)
section when using rootfs, or when using initrd, complete the steps in the [Build an initrd image](#build-an-initrd-image) section.

### Configure runtime for custom debug image

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

### Ensure debug options are valid

For the debug console to work, you **must** ensure that proxy debug is
**disabled** in the configuration file. If proxy debug is enabled, you will
not see any output when you connect to the virtual machine:

```
$ sudo mkdir -p /etc/kata-containers/
$ sudo install -o root -g root -m 0640 /usr/share/defaults/kata-containers/configuration.toml /etc/kata-containers
$ sudo awk '{if (/^\[proxy\.kata\]/) {got=1}; if (got == 1 && /^.*enable_debug/) {print "#enable_debug = true"; got=0; next; } else {print}}' /etc/kata-containers/configuration.toml > /tmp/configuration.toml
$ sudo install -o root -g root -m 0640 /tmp/configuration.toml /etc/kata-containers/
```

### Create a container

Create a container as normal. For example using Docker:

```
$ sudo docker run -ti busybox sh
```

### Connect to the virtual machine using the debug console

```
$ id=$(sudo docker ps -q --no-trunc)
$ console="/var/run/vc/vm/${id}/console.sock"
$ sudo socat "stdin,raw,echo=0,escape=0x11" "unix-connect:${console}"
```

**Note**: You need to press the `RETURN` key to see the shell prompt.

To disconnect from the virtual machine, type `CONTROL+q` (hold down the
`CONTROL` key and press `q`).

### Obtain details of the image

If the image is created using
[osbuilder](https://github.com/kata-containers/osbuilder), the following YAML
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

Which debug options you enable depends on if you are using the hypervisor `vsock` mode
or not, as defined by the `use_vsock` setting in the `[hypervisor.qemu]` section of
the configuration file. The following details the settings:

- For `use_vsock = false`:
    - Set `enable_debug = true` in both the `[hypervisor.qemu]` and `[proxy.kata]` sections
- For `use_vsock = true`:
    - Set `enable_debug = true` in both the `[hypervisor.qemu]` and `[shim.kata]` sections

For generic information on enabling debug in the configuration file, see the
[Enable full debug](#enable-full-debug) section.

The kernel boot messages will appear in the `kata-proxy` or `kata-shim` log appropriately,
such as:

```bash
$ sudo journalctl -t kata-proxy
-- Logs begin at Thu 2020-02-13 16:20:40 UTC, end at Thu 2020-02-13 16:30:23 UTC. --
...
Feb 13 16:20:56 minikube kata-proxy[17371]: time="2020-02-13T16:20:56.608714324Z" level=info msg="[    1.418768] brd: module loaded\n" name=kata-proxy pid=17371 sandbox=a13ffb2b9b5a66f7787bdae9a427fa954a4d21ec4031d0179eee2573986a8a6e source=agent
Feb 13 16:20:56 minikube kata-proxy[17371]: time="2020-02-13T16:20:56.628493231Z" level=info msg="[    1.438612] loop: module loaded\n" name=kata-proxy pid=17371 sandbox=a13ffb2b9b5a66f7787bdae9a427fa954a4d21ec4031d0179eee2573986a8a6e source=agent
Feb 13 16:20:56 minikube kata-proxy[17371]: time="2020-02-13T16:20:56.67707956Z" level=info msg="[    1.487165]  pmem0: p1\n" name=kata-proxy pid=17371 sandbox=a13ffb2b9b5a66f7787bdae9a427fa954a4d21ec4031d0179eee2573986a8a6e source=agent
...
```

## Running standalone

It is possible to start the runtime without a container manager. This is
mostly useful for testing and debugging purposes.

### Create an OCI bundle

To build an
[OCI bundle](https://github.com/opencontainers/runtime-spec/blob/master/bundle.md),
required by the runtime:

```
$ bundle="/tmp/bundle"
$ rootfs="$bundle/rootfs"
$ mkdir -p "$rootfs" && (cd "$bundle" && kata-runtime spec)
$ sudo docker export $(sudo docker create busybox) | tar -C "$rootfs" -xvf -
```

### Launch the runtime to create a container

Run the runtime standalone by providing it with the path to the
previously-created [OCI bundle](#create-an-oci-bundle):

```
$ sudo kata-runtime --log=/dev/stdout run --bundle "$bundle" foo
```
