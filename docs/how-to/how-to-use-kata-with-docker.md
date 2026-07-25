# How to run kata containers from docker

This document describes the basics of running a kata container, using the docker command line tool.

!!! note
    This might be helpful for those getting started with Kata Containers or
    wanting to employ Kata's confinement in existing workflows with Docker.

## Requirements

- A working docker installation.

!!! warning "Deprecated Go runtime"
    This guide uses the deprecated Go runtime. Docker v26+ requires Kata
    Containers 3.29.0 or newer for the Go runtime. Docker support is tested
    only with QEMU as the VMM.

## Install and configure Kata Containers

Download the appropriate architecture's `kata-go-static` package from the
[GitHub releases](https://github.com/kata-containers/kata-containers/releases).
Extract the files to a temporary location and install them into `/opt`:

```sh
tar -xvf kata-go-static-${VERSION}-${ARCH}.tar.zst
sudo mv opt/kata/ /opt/
```

Configure the docker daemon for the kata runtime (assuming no such file exists):

```sh
sudo tee /etc/docker/daemon.json >/dev/null <<EOF
{
  "runtimes": {
    "kata": {
      "runtimeType": "/opt/kata/bin/containerd-shim-kata-v2"
    }
  }
}
EOF
sudo systemctl reload docker
```

Optionally, to use a custom Kata configuration, create
`/etc/kata-containers/configuration.toml`.

To launch a Kata container and observe the guest kernel version:

```sh
docker run --runtime kata -it --rm ubuntu:24.04 uname -r
```

