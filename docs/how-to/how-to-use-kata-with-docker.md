# How to run kata containers from docker

This document describes the basics of running a kata container, using the docker command line tool.

> This might be helpful for those getting started with kata containers or just wanting to employ kata's confinement to existing workflows with docker.

## Requirements

- A working docker installation.

> **Note:** Newer versions of docker (v26+) require Kata Containers 3.29.0 (for the go runtime) and 3.30.0 (for the rust runtime), and the support is only tested with QEMU as the VMM.

## Install and configure Kata Containers

Download the appropriate architecture's package from the Releases on Github https://github.com/kata-containers/kata-containers/releases Extract the files to a temporary location and install into /opt:
```
$ tar -xvf kata-static-${VERSION}-${ARCH}.tar.zst
$ sudo mv opt/kata/ /opt/
```

Configure the docker daemon for the kata runtime (assuming no such file exists):
```
$ sudo cat <<EOF > /etc/docker/daemon.json
{
  "runtimes": {
    "kata": {
      "runtimeType": "/opt/kata/bin/containerd-shim-kata-v2"
    }
  }
}
$ sudo systemctl reload docker
```

Optionally at this point, to use a custom configuration for kata itself, create it in /etc/kata-containers/configuration.toml.

To launched a kata container and observe the guest kernel version:
```
$ docker run --runtime kata -it --rm ubuntu:24.04 uname -r
```

