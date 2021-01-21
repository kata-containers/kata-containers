# Kata Containers snap package

* [Install Kata Containers](#install-kata-containers)
* [Configure Kata Containers](#configure-kata-containers)
* [Integration with non-compatible shim v2 Container Engines](#integration-with-non-compatible-shim-v2-container-engines)
    * [Integration with Docker](#integration-with-docker)
    * [Integration with Podman](#integration-with-podman)
* [Integration with shim v2 Container Engines](#integration-with-shim-v2-container-engines)
* [Remove Kata Containers snap package](#remove-kata-containers-snap-package)


## Install Kata Containers

Kata Containers can be installed in any Linux distribution that supports
[snapd](https://docs.snapcraft.io/installing-snapd).

> NOTE: From Kata Containers 2.x, only the [Containerd Runtime V2 (Shim API)](https://github.com/containerd/containerd/tree/master/runtime/v2)
> is supported, note that some container engines (`docker`, `podman`, etc) may not
> be able to run Kata Containers 2.x.

Kata Containers 1.x is released through the *stable* channel while Kata Containers
2.x is available in the *candidate* channel.

Run the following command to install **Kata Containers 1.x**:

```sh
$ sudo snap install kata-containers --classic
```

Run the following command to install **Kata Containers 2.x**:

```sh
$ sudo snap install kata-containers --candidate --classic
```

## Configure Kata Containers

By default Kata Containers snap image is mounted at `/snap/kata-containers` as a
read-only file system, therefore default configuration file can not be edited.
Fortunately Kata Containers supports loading a configuration file from another
path than the default.

```sh
$ sudo mkdir -p /etc/kata-containers
$ sudo cp /snap/kata-containers/current/usr/share/defaults/kata-containers/configuration.toml /etc/kata-containers/
$ $EDITOR /etc/kata-containers/configuration.toml
```

## Integration with non-compatible shim v2 Container Engines

At the time of writing this document, `docker` and `podman` **do not support Kata
Containers 2.x, therefore Kata Containers 1.x must be used instead.**

The path to the runtime provided by the Kata Containers 1.x snap package is
`/snap/bin/kata-containers.runtime`, it should be used to run Kata Containers 1.x.

### Integration with Docker

`/etc/docker/daemon.json` is the configuration file for `docker`, use the
following configuration to add a new runtime (`kata`) to `docker`.

```json
{
  "runtimes": {
    "kata": {
      "path": "/snap/bin/kata-containers.runtime"
    }
  }
}
```

Once the above configuration has been applied, use the
following commands to restart `docker` and run Kata Containers 1.x.

```sh
$ sudo systemctl restart docker
$ docker run -ti --runtime kata busybox sh
```

### Integration with Podman

`/usr/share/containers/containers.conf` is the configuration file for `podman`,
add the following configuration in the `[engine.runtimes]` section.

```toml
kata = [
   "/snap/bin/kata-containers.runtime"
]
```

Once the above configuration has been applied, use the following command to run
Kata Containers 1.x with `podman`

```sh
$ sudo podman run -ti --runtime kata docker.io/library/busybox sh
```

## Integration with shim v2 Container Engines

The Container engine daemon (`cri-o`, `containerd`, etc) needs to be able to find the
`containerd-shim-kata-v2` binary to allow Kata Containers to be created.
Run the following command to create a symbolic link to the shim v2 binary.

```sh
$ sudo ln -sf /snap/kata-containers/current/usr/bin/containerd-shim-kata-v2 /usr/local/bin/containerd-shim-kata-v2
```

Once the symbolic link has been created and the engine daemon configured, `io.containerd.kata.v2`
can be used as runtime.

Read the following documents to know how to run Kata Containers 2.x with `containerd`.

* [How to use Kata Containers and Containerd](https://github.com/kata-containers/kata-containers/blob/main/docs/how-to/containerd-kata.md)
* [Install Kata Containers with containerd](https://github.com/kata-containers/kata-containers/blob/main/docs/install/container-manager/containerd/containerd-install.md)


## Remove Kata Containers snap package

Run the following command to remove the Kata Containers snap:

```sh
$ sudo snap remove kata-containers
```
