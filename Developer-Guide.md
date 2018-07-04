* [Warning](#warning)
* [Assumptions](#assumptions)
* [Initial setup](#initial-setup)
* [Requirements to build individual components](#requirements-to-build-individual-components)
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
* [Run Kata Containers with Docker](#run-kata-containers-with-docker)
    * [Update Docker configuration](#update-docker-configuration)
    * [Create a container using Kata](#create-a-container-using-kata)
* [Run Kata Containers with Kubernetes](#run-kata-containers-with-kubernetes)
    * [Install a CRI implementation](#install-a-cri-implementation)
        * [CRI-O](#cri-o)
        * [CRI-containerd](#cri-containerd)
    * [Install Kubernetes](#install-kubernetes)
        * [Configure for CRI-O](#configure-for-cri-o)
        * [Configure for CRI-containerd](#configure-for-cri-containerd)
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

- [golang](https://golang.org/dl) version 1.8.3 or newer.

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

As a prerequisite, you need to install Docker. Otherwise, you will not be
able to run the `rootfs.sh` script with `USE_DOCKER=true` as expected in
the following example.

```
$ export ROOTFS_DIR=${GOPATH}/src/github.com/kata-containers/osbuilder/rootfs-builder/rootfs
$ sudo rm -rf ${ROOTFS_DIR}
$ cd $GOPATH/src/github.com/kata-containers/osbuilder/rootfs-builder
$ script -fec 'sudo -E GOPATH=$GOPATH USE_DOCKER=true ./rootfs.sh ${distro}'
```
You MUST choose one of `alpine`, `centos`, `clearlinux`, `euleros`, and `fedora` for `${distro}`.

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
$ script -fec 'sudo -E GOPATH=$GOPATH AGENT_INIT=yes USE_DOCKER=true ./rootfs.sh ${distro}'
```
`AGENT_INIT` controls if the guest image uses kata agent as the guest `init` process. When you create an initrd image,
always set `AGENT_INIT` to `yes`.

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

As a prerequisite, you need to install `libelf-dev` and `bc`. Otherwise, you
will not be able to build the kernel from sources.

```
$ go get github.com/kata-containers/tests
$ cd $GOPATH/src/github.com/kata-containers/tests/.ci
$ kernel_arch="$(./kata-arch.sh)"
$ kernel_dir="$(./kata-arch.sh --kernel)"
$ tmpdir="$(mktemp -d)"
$ pushd "$tmpdir"
$ curl -L https://raw.githubusercontent.com/kata-containers/packaging/master/kernel/configs/${kernel_arch}_kata_kvm_4.14.x -o .config
$ kernel_version=$(grep "Linux/[${kernel_arch}]*" .config | cut -d' ' -f3 | tail -1)
$ kernel_tar_file="linux-${kernel_version}.tar.xz"
$ kernel_url="https://cdn.kernel.org/pub/linux/kernel/v$(echo $kernel_version | cut -f1 -d.).x/${kernel_tar_file}"
$ curl -LOk ${kernel_url}
$ tar -xf ${kernel_tar_file}
$ mv .config "linux-${kernel_version}"
$ pushd "linux-${kernel_version}"
$ curl -L https://raw.githubusercontent.com/kata-containers/packaging/master/kernel/patches/0001-NO-UPSTREAM-9P-always-use-cached-inode-to-fill-in-v9.patch | patch -p1
$ make ARCH=${kernel_dir} -j$(nproc)
$ kata_kernel_dir="/usr/share/kata-containers"
$ kata_vmlinuz="${kata_kernel_dir}/kata-vmlinuz-${kernel_version}.container"
$ [ $kernel_arch = ppc64le ] && kernel_file="$(realpath ./vmlinux)" || kernel_file="$(realpath arch/${kernel_arch}/boot/bzImage)"
$ sudo install -o root -g root -m 0755 -D "${kernel_file}" "${kata_vmlinuz}"
$ sudo ln -sf "${kata_vmlinuz}" "${kata_kernel_dir}/vmlinuz.container"
$ kata_vmlinux="${kata_kernel_dir}/kata-vmlinux-${kernel_version}"
$ sudo install -o root -g root -m 0755 -D "$(realpath vmlinux)" "${kata_vmlinux}"
$ sudo ln -sf "${kata_vmlinux}" "${kata_kernel_dir}/vmlinux.container"
$ popd
$ popd
$ rm -rf "${tmpdir}"
```

# Run Kata Containers with Docker

## Update Docker configuration

```
$ dir=/etc/systemd/system/docker.service.d
$ file="$dir/kata-containers.conf"
$ sudo mkdir -p "$dir"
$ sudo test -e "$file" || echo -e "[Service]\nType=simple\nExecStart=\nExecStart=/usr/bin/dockerd -D --default-runtime runc" | sudo tee "$file"
$ sudo grep -q "kata-runtime=" $file || sudo sed -i 's!^\(ExecStart=[^$].*$\)!\1 --add-runtime kata-runtime=/usr/local/bin/kata-runtime!g' "$file"
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
[here](https://github.com/kata-containers/documentation/blob/master/architecture.md#mixing-vm-based-and-namespace-based-runtimes).

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

### CRI-containerd

If you select CRI-containerd, follow the "Getting Started for Developers"
instructions [here](https://github.com/containerd/cri#getting-started-for-developers)
to properly install it.

To customize CRI-containerd to select Kata Containers runtime, follow our
"Configure containerd to use Kata Containers" internal documentation
[here](https://github.com/kata-containers/documentation/blob/master/how-to/how-to-use-k8s-with-cri-containerd-and-kata.md#configure-containerd-to-use-kata-containers).

## Install Kubernetes

Depending on what your needs are and what you expect to do with Kubernetes,
please refer to the following
[documentation](https://kubernetes.io/docs/setup/pick-right-solution/) to
install it correctly.

Kubernetes talks with CRI implementations through a `container-runtime-endpoint`,
also called CRI socket. This socket path is different depending on which CRI
implementation you chose, and the kubelet service has to be updated accordingly.

### Configure for CRI-O

`/etc/systemd/system/kubelet.service.d/0-crio.conf`
```
[Service]                                                 
Environment="KUBELET_EXTRA_ARGS=--container-runtime=remote --runtime-request-timeout=15m --container-runtime-endpoint=unix:///var/run/crio/crio.sock"
```

### Configure for CRI-containerd

`/etc/systemd/system/kubelet.service.d/0-cri-containerd.conf`
```
[Service]                                                 
Environment="KUBELET_EXTRA_ARGS=--container-runtime=remote --runtime-request-timeout=15m --container-runtime-endpoint=unix:///run/containerd/containerd.sock"
```
For more information about CRI-containerd see the "Configure Kubelet to use containerd"
documentation [here](https://github.com/kata-containers/documentation/blob/master/how-to/how-to-use-k8s-with-cri-containerd-and-kata.md#configure-kubelet-to-use-containerd).

## Run a Kubernetes pod with Kata Containers

After you update your kubelet service based on the CRI implementation you
are using, reload and restart kubelet. Then, start your cluster:
```bash
$ sudo systemctl daemon-reload
$ sudo systemctl restart kubelet

# If using CRI-O
$ sudo kubeadm init --skip-preflight-checks --cri-socket /var/run/crio/crio.sock --pod-network-cidr=10.244.0.0/16

# If using CRI-containerd
$ sudo kubeadm init --skip-preflight-checks --cri-socket /run/containerd/containerd.sock --pod-network-cidr=10.244.0.0/16

$ export KUBECONFIG=/etc/kubernetes/admin.conf
```

You can force kubelet to use Kata Containers by adding some _untrusted_
annotation to your pod configuration. In our case, this ensures Kata
Containers is the selected runtime to run the described workload.

_nginx-untrusted.yaml_
```yaml
apiVersion: v1
kind: Pod
metadata:
  name: nginx-untrusted
  annotations:
    io.kubernetes.cri.untrusted-workload: "true"
spec:
  containers:
    name: nginx
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

See [Set up a debug console](#set-up-a-debug-console).

# Appendices

## Checking Docker default runtime

```
$ sudo docker info 2>/dev/null | grep -i "default runtime" | cut -d: -f2- | grep -q runc  && echo "SUCCESS" || echo "ERROR: Incorrect default Docker runtime"
```

## Set up a debug console

By default you cannot login to a virtual machine since this can be sensitive
from a security perspective. Also allowing logins would require additional
packages in the rootfs, which would increase the size of the image used to
boot the virtual machine.

If you want to login to a virtual machine that hosts your containers, complete
the following steps, which assume a rootfs image.

### Create a custom image containing a shell

To login to a virtual machine, you must
[create a custom rootfs](#create-a-rootfs-image)
containing a shell such as `bash(1)`.

For example using CentOS:

```
$ cd $GOPATH/src/github.com/kata-containers/osbuilder/rootfs-builder
$ export ROOTFS_DIR=${GOPATH}/src/github.com/kata-containers/osbuilder/rootfs-builder/rootfs
$ script -fec 'sudo -E GOPATH=$GOPATH USE_DOCKER=true EXTRA_PKGS="bash" ./rootfs.sh centos'
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
PrivateDevices=yes
Type=simple
ExecStart=/usr/bin/bash
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
$ sudo awk '{if (/^\[proxy\.kata\]/) {got=1}; if (got == 1 && /^.*enable_debug/) {print "#enable_debug = true"; got=0; next; } else {print}}' /usr/share/defaults/kata-containers/configuration.toml > /tmp/configuration.toml
$ sudo install -o root -g root -m 0640 /tmp/configuration.toml /usr/share/defaults/kata-containers/configuration.toml
```

### Create a container

Create a container as normal. For example using Docker:

```
$ sudo docker run -ti busybox sh
```

### Connect to the virtual machine using the debug console

```
$ id=$(sudo docker ps -q --no-trunc)
$ console="/var/run/vc/sbs/${id}/console.sock"
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
