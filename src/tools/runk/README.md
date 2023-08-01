# runk

## Overview

> **Warnings:**
> `runk` is currently an experimental tool.
> Only continue if you are using a non-critical system.

`runk` is a standard OCI container runtime written in Rust based on a modified version of
the [Kata Container agent](https://github.com/kata-containers/kata-containers/tree/main/src/agent), `kata-agent`.

`runk` conforms to the [OCI Container Runtime specifications](https://github.com/opencontainers/runtime-spec).

Unlike the [Kata Container runtime](https://github.com/kata-containers/kata-containers/tree/main/src/agent#features),
`kata-runtime`, `runk` spawns and runs containers on the host machine directly.
The user can run `runk` in the same way as the existing container runtimes such as `runc`,
the most used implementation of the OCI runtime specs.

## Why does `runk` exist?

The `kata-agent` is a process running inside a virtual machine (VM) as a supervisor for managing containers
and processes running within those containers.
In other words, the `kata-agent` is a kind of "low-level" container runtime inside VM because the agent
spawns and runs containers according to the OCI runtime specs.
However, the `kata-agent` does not have the OCI Command-Line Interface (CLI) that is defined in the
[runtime spec](https://github.com/opencontainers/runtime-spec/blob/main/runtime.md).
The `kata-runtime` provides the CLI part of the Kata Containers runtime component,
but the `kata-runtime` is a container runtime for creating hardware-virtualized containers running on the host.

`runk` is a Rust-based standard OCI container runtime that manages normal containers,
not hardware-virtualized containers.
`runk` aims to become one of the alternatives to existing OCI compliant container runtimes.
The `kata-agent` has most of the [features](https://github.com/kata-containers/kata-containers/tree/main/src/agent#features)
needed for the container runtime and delivers high performance with a low memory footprint owing to the
implementation by Rust language.
Therefore, `runk` leverages the mechanism of the `kata-agent` to avoid reinventing the wheel.

## Performance

`runk` is faster than `runc` and has a lower memory footprint.

This table shows the average of the elapsed time and the memory footprint (maximum resident set size)
for running sequentially 100 containers, the containers run `/bin/true` using `run` command with
[detached mode](https://github.com/opencontainers/runc/blob/main/docs/terminals.md#detached)
on 12 CPU cores (`3.8 GHz AMD Ryzen 9 3900X`) and 32 GiB of RAM.
`runk` always runs containers with detached mode currently.

Evaluation Results:

|                       | `runk` (v0.0.1) | `runc` (v1.0.3) | `crun` (v1.4.2) |
|-----------------------|---------------|---------------|---------------|
| time [ms]           | 39.83         | 50.39         | 38.41         |
| memory footprint [MB] | 4.013         | 10.78         | 1.738         |

## Status of `runk`

We drafted the initial code here, and any contributions to `runk` and [`kata-agent`](https://github.com/kata-containers/kata-containers/tree/main/src/agent)
are welcome.

Regarding features compared to `runc`, see the `Status of runk` section in the [issue](https://github.com/kata-containers/kata-containers/issues/2784).

## Building

In order to enable seccomp support, you need to install the `libseccomp` library on
your platform.

> e.g. `libseccomp-dev` for Ubuntu, or `libseccomp-devel` for CentOS

You can build `runk`:

```bash
$ cd runk
$ make
```

If you want to build a statically linked binary of `runk`, set the environment
variables for the [`libseccomp` crate](https://github.com/libseccomp-rs/libseccomp-rs) and
set the `LIBC` to `musl`:

```bash
$ export LIBSECCOMP_LINK_TYPE=static
$ export LIBSECCOMP_LIB_PATH="the path of the directory containing libseccomp.a"
$ export LIBC=musl
$ make
```

> **Note**:
>
> - If the compilation fails when `runk` tries to link the `libseccomp` library statically
>   against `musl`, you will need to build the `libseccomp` manually with `-U_FORTIFY_SOURCE`.
>   For the details, see [our script](https://github.com/kata-containers/kata-containers/blob/main/ci/install_libseccomp.sh)
>   to install the `libseccomp` for the agent.
> - On `ppc64le` and `s390x`, `glibc` should be used even if `LIBC=musl` is specified.
> - If you do not want to enable seccomp support, run `make SECCOMP=no`.

To install `runk` into default directory for executable program (`/usr/local/bin`):

```bash
$ sudo -E make install
```

## Using `runk` directly

Please note that `runk` is a low level tool not developed with an end user in mind.
It is mostly employed by other higher-level container software like `containerd`.

If you still want to use `runk` directly, here's how.

### Prerequisites

It is necessary to create an OCI bundle to use the tool. The simplest method is:

``` bash
$ bundle_dir="bundle"
$ rootfs_dir="$bundle_dir/rootfs"
$ image="busybox"
$ mkdir -p "$rootfs_dir" && (cd "$bundle_dir" && runk spec)
$ sudo docker export $(sudo docker create "$image") | tar -C "$rootfs_dir" -xf -
```

> **Note:**
> If you use the unmodified `runk spec` template, this should give a `sh` session inside the container.
> However, if you use `runk` directly and run a container with the unmodified template,
> `runk` cannot launch the `sh` session because `runk` does not support terminal handling yet.
> You need to edit the process field in the `config.json` should look like this below
> with `"terminal": false` and `"args": ["sleep", "10"]`.

```json
"process": {
    "terminal": false,
    "user": {
        "uid": 0,
        "gid": 0
    },
    "args": [
        "sleep",
        "10"
    ],
    "env": [
        "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        "TERM=xterm"
    ],
    "cwd": "/",
    [...]
}
```

If you want to launch the `sh` session inside the container, you need to run `runk` from `containerd`.

Please refer to the [Using `runk` from containerd](#using-runk-from-containerd) section

### Running a container

Now you can go through the [lifecycle operations](https://github.com/opencontainers/runtime-spec/blob/main/runtime.md)
in your shell.
You need to run `runk` as `root` because `runk` does not have the rootless feature which is the ability
to run containers without root privileges.

```bash
$ cd $bundle_dir

# Create a container
$ sudo runk create test

# View the container is created and in the "created" state
$ sudo runk state test

# Start the process inside the container
$ sudo runk start test

# After 10 seconds view that the container has exited and is now in the "stopped" state
$ sudo runk state test

# Now delete the container
$ sudo runk delete test
```

## Using `runk` from `Docker`

`runk` can run containers using [`Docker`](https://github.com/docker).

First, install `Docker` from package by following the
[`Docker` installation instructions](https://docs.docker.com/engine/install/).

### Running a container with `Docker` command line

Start the docker daemon:

```bash
$ sudo dockerd --experimental --add-runtime="runk=/usr/local/bin/runk"
```

> **Note:**
> Before starting the `dockerd`, you need to stop the normal docker daemon
> running on your environment (i.e., `systemctl stop docker`).

Launch a container in a different terminal:

```bash
$ sudo docker run -it --rm --runtime runk busybox sh
/ #
```

## Using `runk` from `Podman`

`runk` can run containers using [`Podman`](https://github.com/containers/podman).

First, install `Podman` from source code or package by following the
[`Podman` installation instructions](https://podman.io/getting-started/installation).

### Running a container with `Podman` command line

```bash
$ sudo podman --runtime /usr/local/bin/runk run -it --rm busybox sh
/ #
```

> **Note:**
> `runk` does not support some commands except
> [OCI standard operations](https://github.com/opencontainers/runtime-spec/blob/main/runtime.md#operations)
> yet, so those commands do not work in `Docker/Podman`. Regarding commands currently
> implemented in `runk`, see the [Status of `runk`](#status-of-runk) section.

## Using `runk` from `containerd`

`runk` can run containers with the containerd runtime handler support on `containerd`.

### Prerequisites for `runk` with containerd

* `containerd` v1.2.4 or above
* `cri-tools`

> **Note:**
> [`cri-tools`](https://github.com/kubernetes-sigs/cri-tools) is a set of tools for CRI
> used for development and testing.

Install `cri-tools` from source code:

```bash
$ go get github.com/kubernetes-sigs/cri-tools
$ pushd $GOPATH/src/github.com/kubernetes-sigs/cri-tools
$ make
$ sudo -E make install
$ popd
```

Write the `crictl` configuration file:

``` bash
$ cat <<EOF | sudo tee /etc/crictl.yaml
runtime-endpoint: unix:///run/containerd/containerd.sock
EOF
```

### Configure `containerd` to use `runk`

Update `/etc/containerd/config.toml`:

```bash
$ cat <<EOF | sudo tee /etc/containerd/config.toml
version = 2
[plugins."io.containerd.runtime.v1.linux"]
  shim_debug = true
[plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runc]
  runtime_type = "io.containerd.runc.v2"
[plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runk]
  runtime_type = "io.containerd.runc.v2"
  [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runk.options]
    BinaryName = "/usr/local/bin/runk"
EOF
```

Restart `containerd`:

```bash
$ sudo systemctl restart containerd
```

### Running a container with `crictl` command line

You can run containers in `runk` via containerd's CRI.

Pull the `busybox` image:

``` bash
$ sudo crictl pull busybox
```

Create the sandbox configuration:

``` bash
$ cat <<EOF | tee sandbox.json
{
    "metadata": {
        "name": "busybox-sandbox",
        "namespace": "default",
        "attempt": 1,
        "uid": "hdishd83djaidwnduwk28bcsb"
    },
    "log_directory": "/tmp",
    "linux": {
    }
}
EOF
```

Create the container configuration:

``` bash
$ cat <<EOF | tee container.json
{
    "metadata": {
        "name": "busybox"
    },
    "image": {
        "image": "docker.io/busybox"
    },
    "command": [
        "sh"
    ],
    "envs": [
        {
            "key": "PATH",
            "value": "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
        },
        {
            "key": "TERM",
            "value": "xterm"
        }
    ],
    "log_path": "busybox.0.log",
    "stdin": true,
    "stdin_once": true,
    "tty": true
}
EOF
```

With the `crictl` command line of `cri-tools`, you can specify runtime class with `-r` or `--runtime` flag.

Launch a sandbox and container using the `crictl`:

```bash
# Run a container inside a sandbox
$ sudo crictl run -r runk container.json sandbox.json
f492eee753887ba3dfbba9022028975380739aba1269df431d097b73b23c3871

# Attach to the running container
$ sudo crictl attach --stdin --tty f492eee753887ba3dfbba9022028975380739aba1269df431d097b73b23c3871
/ #
```

