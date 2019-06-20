* [Warning](#warning)
* [Assumptions](#assumptions)
* [Initial setup](#initial-setup)
* [Requirements to build individual components](#requirements-to-build-individual-components)
* [Build and install the Kata Containers runtime](#build-and-install-the-kata-containers-runtime)
    * [Check hardware requirements](#check-hardware-requirements)
    * [Configure to use initrd or rootfs image](#configure-to-use-initrd-or-rootfs-image)
    * [Enable full debug](#enable-full-debug)
        * [journald rate limiting](#journald-rate-limiting)
            * [systemd-journald suppressing messages](#systemd-journald-suppressing-messages)
            * [Disabling systemd-journald rate limiting](#disabling-systemd-journald-rate-limiting)
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
* [Install a hypervisor](#install-a-hypervisor)
    * [Build a custom QEMU](#build-a-custom-qemu)
        * [Build a custom QEMU for aarch64/arm64 - REQUIRED](#build-a-custom-qemu-for-aarch64arm64---required)
* [Run Kata Containers with Docker](#run-kata-containers-with-docker)
    * [Update the Docker systemd unit file](#update-the-docker-systemd-unit-file)
    * [Create a container using Kata](#create-a-container-using-kata)
* [Run Kata Containers with Kubernetes](#run-kata-containers-with-kubernetes)
    * [Install a CRI implementation](#install-a-cri-implementation)
        * [CRI-O](#cri-o)
        * [containerd with CRI plugin](#containerd-with-cri-plugin)
    * [Install Kubernetes](#install-kubernetes)
        * [Configure for CRI-O](#configure-for-cri-o)
        * [Configure for containerd](#configure-for-containerd)
    * [Run a Kubernetes pod with Kata Containers](#run-a-kubernetes-pod-with-kata-containers)
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

As a prerequisite, you need to install Docker. Otherwise, you will not be
able to run the `rootfs.sh` script with `USE_DOCKER=true` as expected in
the following example.

```
$ export ROOTFS_DIR=${GOPATH}/src/github.com/kata-containers/osbuilder/rootfs-builder/rootfs
$ sudo rm -rf ${ROOTFS_DIR}
$ cd $GOPATH/src/github.com/kata-containers/osbuilder/rootfs-builder
$ script -fec 'sudo -E GOPATH=$GOPATH USE_DOCKER=true SECCOMP=no ./rootfs.sh ${distro}'
```
You MUST choose one of `alpine`, `centos`, `clearlinux`, `euleros`, and `fedora` for `${distro}`. By default `seccomp` packages are not included in the rootfs image. Set `SECCOMP` to `yes` to include them.

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

You can build and install the guest kernel image as shown [here](https://github.com/kata-containers/packaging/tree/master/kernel#build-kata-containers-kernel).

# Install a hypervisor

When setting up Kata using a [packaged installation method](https://github.com/kata-containers/documentation/tree/master/install#installing-on-a-linux-system), the `qemu-lite` hypervisor is installed automatically. For other installation methods, you will need to manually install a suitable hypervisor.

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

Now that Kata Containers is installed on your system, you need some
extra components to make this work with Kubernetes.

## Install a CRI implementation

Kata Containers runtime is an OCI compatible runtime and cannot directly
interact with the CRI API level. For this reason we rely on a CRI
implementation to translate CRI into OCI. There are two supported ways
called [CRI-O](https://github.com/kubernetes-incubator/cri-o) and
[CRI-containerd](https://github.com/containerd/cri). It is up to you to
choose the one that you want, but you have to pick one. After choosing
either CRI-O or CRI-containerd, you must make the appropriate changes
to ensure it relies on the Kata Containers runtime.

As of Kata Containers 1.5, using `shimv2` with containerd 1.2.0 or above is the preferred
way to run Kata Containers with Kubernetes ([see the howto](https://github.com/kata-containers/documentation/blob/master/how-to/how-to-use-k8s-with-cri-containerd-and-kata.md#configure-containerd-to-use-kata-containers)).
The CRI-O will catch up soon.

### CRI-O

If you select CRI-O, follow the "CRI-O Tutorial" instructions
[here](https://github.com/kubernetes-incubator/cri-o/blob/master/tutorial.md)
to properly install it.

Once you have installed CRI-O, you need to modify the CRI-O configuration
with information about different container runtimes. By default, we choose
`runc`, but in this case we also specify Kata Containers runtime to run
__untrusted__ workloads. In other words, this defines an alternative runtime
to be used when the workload cannot be trusted and a higher level of security
is required. An additional flag can be used to let CRI-O know if a workload
should be considered _trusted_ or _untrusted_ by default.
For further details, see the documentation
[here](https://github.com/kata-containers/documentation/blob/master/design/architecture.md#mixing-vm-based-and-namespace-based-runtimes).

Additionally, we need CRI-O to perform the network namespace management.
Otherwise, when the VM starts the network will not be available.

The following is an example of how to modify the `/etc/crio/crio.conf` file
in order to apply the previous explanations, and therefore get Kata Containers
runtime to invoke by CRI-O.

```toml
# The "crio.runtime" table contains settings pertaining to the OCI
# runtime used and options for how to set up and manage the OCI runtime.
[crio.runtime]
manage_network_ns_lifecycle = true

# runtime is the OCI compatible runtime used for trusted container workloads.
# This is a mandatory setting as this runtime will be the default one
# and will also be used for untrusted container workloads if
# runtime_untrusted_workload is not set.
runtime = "/usr/bin/runc"

# runtime_untrusted_workload is the OCI compatible runtime used for untrusted
# container workloads. This is an optional setting, except if
# default_container_trust is set to "untrusted".
runtime_untrusted_workload = "/usr/bin/kata-runtime"

# default_workload_trust is the default level of trust crio puts in container
# workloads. It can either be "trusted" or "untrusted", and the default
# is "trusted".
# Containers can be run through different container runtimes, depending on
# the trust hints we receive from kubelet:
# - If kubelet tags a container workload as untrusted, crio will try first to
# run it through the untrusted container workload runtime. If it is not set,
# crio will use the trusted runtime.
# - If kubelet does not provide any information about the container workload trust
# level, the selected runtime will depend on the default_container_trust setting.
# If it is set to "untrusted", then all containers except for the host privileged
# ones, will be run by the runtime_untrusted_workload runtime. Host privileged
# containers are by definition trusted and will always use the trusted container
# runtime. If default_container_trust is set to "trusted", crio will use the trusted
# container runtime for all containers.
default_workload_trust = "untrusted"

```

Restart CRI-O to take changes into account
```
$ sudo systemctl restart crio
```

### containerd with CRI plugin

If you select containerd with `cri` plugin, follow the "Getting Started for Developers"
instructions [here](https://github.com/containerd/cri#getting-started-for-developers)
to properly install it.

To customize containerd to select Kata Containers runtime, follow our
"Configure containerd to use Kata Containers" internal documentation
[here](https://github.com/kata-containers/documentation/blob/master/how-to/how-to-use-k8s-with-cri-containerd-and-kata.md#configure-containerd-to-use-kata-containers).

## Install Kubernetes

Depending on what your needs are and what you expect to do with Kubernetes,
please refer to the following
[documentation](https://kubernetes.io/docs/setup/) to install it correctly.

Kubernetes talks with CRI implementations through a `container-runtime-endpoint`,
also called CRI socket. This socket path is different depending on which CRI
implementation you chose, and the Kubelet service has to be updated accordingly.

### Configure for CRI-O

`/etc/systemd/system/kubelet.service.d/0-crio.conf`
```
[Service]
Environment="KUBELET_EXTRA_ARGS=--container-runtime=remote --runtime-request-timeout=15m --container-runtime-endpoint=unix:///var/run/crio/crio.sock"
```

### Configure for containerd

`/etc/systemd/system/kubelet.service.d/0-cri-containerd.conf`
```
[Service]
Environment="KUBELET_EXTRA_ARGS=--container-runtime=remote --runtime-request-timeout=15m --container-runtime-endpoint=unix:///run/containerd/containerd.sock"
```
For more information about containerd see the "Configure Kubelet to use containerd"
documentation [here](https://github.com/kata-containers/documentation/blob/master/how-to/how-to-use-k8s-with-cri-containerd-and-kata.md#configure-kubelet-to-use-containerd).

## Run a Kubernetes pod with Kata Containers

After you update your Kubelet service based on the CRI implementation you
are using, reload and restart Kubelet. Then, start your cluster:
```bash
$ sudo systemctl daemon-reload
$ sudo systemctl restart kubelet

# If using CRI-O
$ sudo kubeadm init --skip-preflight-checks --cri-socket /var/run/crio/crio.sock --pod-network-cidr=10.244.0.0/16

# If using CRI-containerd
$ sudo kubeadm init --skip-preflight-checks --cri-socket /run/containerd/containerd.sock --pod-network-cidr=10.244.0.0/16

$ export KUBECONFIG=/etc/kubernetes/admin.conf
```

You can force Kubelet to use Kata Containers by adding some `untrusted`
annotation to your pod configuration. In our case, this ensures Kata
Containers is the selected runtime to run the described workload.

`nginx-untrusted.yaml`
```yaml
apiVersion: v1
kind: Pod
metadata:
  name: nginx-untrusted
  annotations:
    io.kubernetes.cri.untrusted-workload: "true"
spec:
  containers:
    - name: nginx
      image: nginx
```

Next, you run your pod:
```
$ sudo -E kubectl apply -f nginx-untrusted.yaml
```

# Troubleshoot Kata Containers

If you are unable to create a Kata Container first ensure you have
[enabled full debug](#enable-full-debug)
before attempting to create a container. Then run the
[`kata-collect-data.sh`](https://github.com/kata-containers/runtime/blob/master/data/kata-collect-data.sh.in)
script and paste its output directly into a
[GitHub issue](https://github.com/kata-containers/kata-containers/issues/new).

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
the following steps, which assume the use of a rootfs image.

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
> Once these steps are taken you can connect to the virtual machine using the [debug console](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md#connect-to-the-virtual-machine-using-the-debug-console).

### Create a custom image containing a shell

To login to a virtual machine, you must
[create a custom rootfs](#create-a-rootfs-image)
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
section.

### Configure runtime for custom debug image

Install the image:

```
$ name="kata-containers-centos-with-debug-console.img"
$ sudo install -o root -g root -m 0640 kata-containers.img "/usr/share/kata-containers/${name}"
```

Next, modify the `image=` values in the `[hypervisor.qemu]` section of the
[configuration file](https://github.com/kata-containers/runtime#configuration)
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
