## Introduction

The files in this directory can be used to build a modified Kata Containers rootfs
and kernel with modifications to support Intel® QuickAssist Technology (QAT) 
hardware. It is designed to work with Kata Container versions 2.0 and higher.

To properly load the driver modules, systemd init must be used. It is not adequate 
to use the agent as the init. Because of this, alpine is not a valid base OS image
to use. The following rootfs OS's have been tested with this Dockerfile.

* Clear Linux
* Debian
* Ubuntu 

The generated files will need to be copied and configured into your Kata Containers
setup.

Please see the 
[Using Intel® QuickAssist Technology and Kata](../../../../docs/use-cases/using-Intel-QAT-and-kata.md)
documentation for more specific details on how to configure a host system and 
enable acceleration of workloads.

## Building

The image build and run are executed using Docker, from within this `QAT` folder. 
It is required to use **all** the files in this directory to build the Docker 
image:

```sh
$ docker build --label kataqat --tag kataqat:latest . 
$ mkdir ./output
$ docker run -ti --rm --privileged -v /dev:/dev -v $(pwd)/output:/output kataqat
```

> **Note:** The use of the `--privileged` and `-v /dev:/dev` arguments to the `docker run` are
> necessary, to enable the scripts within the container to generate a roofs file system.

When complete, the generated files will be placed into the output directory.
Sample config files that have been modified with a `[SHIM`] section are also 
placed into the `config` subdirectory as a reference that can be used with
Kata Containers.

```sh
# ls -lR output
output:
total 136656
drwxr-xr-x 2 root root      4096 Feb 11 23:59 configs
-rw-r--r-- 1 root root 134217728 Feb 11 23:59 kata-containers.img
-rw-r--r-- 1 root root   5710336 Feb 11 23:59 vmlinuz-kata-linux-5.4.71-84_qat

output/configs:
total 20
-rw-r--r-- 1 root root 4082 Feb 11 23:59 200xxvf_dev0.conf
-rw-r--r-- 1 root root 4082 Feb 11 23:59 c3xxxvf_dev0.conf
-rw-r--r-- 1 root root 4082 Feb 11 23:59 c6xxvf_dev0.conf
-rw-r--r-- 1 root root 4082 Feb 11 23:59 d15xxvf_dev0.conf
-rw-r--r-- 1 root root 4082 Feb 11 23:59 dh895xccvf_dev0.conf
```

## Options

A number of parameters to the scripts are configured in the `Dockerfile`, and thus can be modified
on the commandline. The `AGENT_VERSION` is not set and by default will use the
latest stable version of Kata Containers. 


| Variable | Definition | Default value |
| -------- | ---------- | ------------- |
| `AGENT_VERSION` | Kata agent that is installed into the rootfs |  |
| `KATA_REPO_VERSION` | Kata Branch or Tag to build from | `main` |
| `OUTPUT_DIR` | Directory inside container where results are stored | `/output` |
| `QAT_CONFIGURE_OPTIONS` | `configure` options for QAT driver | `--enable-icp-sriov=guest` |
| `QAT_DRIVER_URL` | URL to curl QAT driver from | `https://01.org/sites/default/files/downloads/${QAT_DRIVER_VER}` |
| `QAT_DRIVER_VER` | QAT driver version to use | `qat1.7.l.4.9.0-00008.tar.gz` |
| `ROOTFS_OS` | Operating system to use for the rootfs | `ubuntu` |

Variables can be set on the `docker run` commandline, for example:

```sh
$ docker run -ti --rm --privileged -e "AGENT_VERSION=2.0.0" -v /dev:/dev -v ${PWD}/output:/output kataqat
```
