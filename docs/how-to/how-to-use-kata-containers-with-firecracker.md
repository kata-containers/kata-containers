# Configure Kata Containers to use Firecracker

This document provides an overview on how to run Kata Containers with the AWS Firecracker hypervisor.

## Introduction

AWS Firecracker is an open source virtualization technology that is purpose-built for creating and managing secure, multi-tenant container and function-based services that provide serverless operational models. AWS Firecracker runs workloads in lightweight virtual machines, called `microVMs`, which combine the security and isolation properties provided by hardware virtualization technology with the speed and flexibility of Containers.

Please refer to AWS Firecracker [documentation](https://github.com/firecracker-microvm/firecracker/blob/main/docs/getting-started.md) for more details.

## Pre-requisites

This document requires the presence of Kata Containers on your system. Install using the instructions available through the following links:

- Kata Containers [automated installation](../install/README.md)

- Kata Containers manual installation: Automated installation does not seem to be supported for Clear Linux, so please use [manual installation](../Developer-Guide.md) steps.
> **Note:** Create rootfs image and not initrd image.

## Install AWS Firecracker

For information about the supported version of Firecracker, see the Kata Containers
[`versions.yaml`](../../versions.yaml).

To install Firecracker we need to get the `firecracker` and `jailer` binaries:

```bash
$ release_url="https://github.com/firecracker-microvm/firecracker/releases"
$ version=$(yq read <kata-repository>/versions.yaml assets.hypervisor.firecracker.version)
$ arch=`uname -m`
$ curl ${release_url}/download/${version}/firecracker-${version}-${arch} -o firecracker
$ curl ${release_url}/download/${version}/jailer-${version}-${arch} -o jailer
$ chmod +x jailer firecracker
```

To make the binaries available from the default system `PATH` it is recommended to move them to `/usr/local/bin` or add a symbolic link:

```bash
$ sudo ln -s $(pwd)/firecracker /usr/local/bin
$ sudo ln -s $(pwd)/jailer /usr/local/bin
```

More details can be found in [AWS Firecracker docs](https://github.com/firecracker-microvm/firecracker/blob/main/docs/getting-started.md)

In order to run Kata with AWS Firecracker a block device as the backing store for a VM is required. To interact with `containerd` and Kata we use the `devmapper` `snapshotter`.

## Configure `devmapper`

To check support for your `containerd` installation, you can run:

```
$ ctr plugins ls |grep devmapper
```

if the output of the above command is:

```
io.containerd.snapshotter.v1    devmapper                linux/amd64    ok
```
then you can skip this section and move on to `Configure Kata Containers with AWS Firecracker`

If the output of the above command is:

```
io.containerd.snapshotter.v1    devmapper                linux/amd64    error
```

then we need to setup `devmapper` `snapshotter`. Based on a [very useful
guide](https://docs.docker.com/storage/storagedriver/device-mapper-driver/)
from docker, we can set it up using the following scripts:

> **Note:** The following scripts assume a 100G sparse file for storing container images, a 10G sparse file for the thin-provisioning pool and 10G base image files for any sandboxed container created. This means that we will need at least 10GB free space.

```
#!/bin/bash
set -ex

DATA_DIR=/var/lib/containerd/devmapper
POOL_NAME=devpool

mkdir -p ${DATA_DIR}

# Create data file
sudo touch "${DATA_DIR}/data"
sudo truncate -s 100G "${DATA_DIR}/data"

# Create metadata file
sudo touch "${DATA_DIR}/meta"
sudo truncate -s 10G "${DATA_DIR}/meta"

# Allocate loop devices
DATA_DEV=$(sudo losetup --find --show "${DATA_DIR}/data")
META_DEV=$(sudo losetup --find --show "${DATA_DIR}/meta")

# Define thin-pool parameters.
# See https://www.kernel.org/doc/Documentation/device-mapper/thin-provisioning.txt for details.
SECTOR_SIZE=512
DATA_SIZE="$(sudo blockdev --getsize64 -q ${DATA_DEV})"
LENGTH_IN_SECTORS=$(bc <<< "${DATA_SIZE}/${SECTOR_SIZE}")
DATA_BLOCK_SIZE=128
LOW_WATER_MARK=32768

# Create a thin-pool device
sudo dmsetup create "${POOL_NAME}" \
    --table "0 ${LENGTH_IN_SECTORS} thin-pool ${META_DEV} ${DATA_DEV} ${DATA_BLOCK_SIZE} ${LOW_WATER_MARK}"

cat << EOF
#
# Add this to your config.toml configuration file and restart containerd daemon
#
[plugins]
  [plugins.devmapper]
    pool_name = "${POOL_NAME}"
    root_path = "${DATA_DIR}"
    base_image_size = "10GB"
    discard_blocks = true
EOF
```

Make it executable and run it:

```bash
$ sudo chmod +x ~/scripts/devmapper/create.sh
$ cd ~/scripts/devmapper/
$ sudo ./create.sh
```

Now, we can add the `devmapper` configuration provided from the script to `/etc/containerd/config.toml`.
> **Note:** If you are using the default `containerd` configuration (`containerd config default >> /etc/containerd/config.toml`), you may need to edit the existing `[plugins."io.containerd.snapshotter.v1.devmapper"]`configuration.
Save and restart `containerd`:


```bash
$ sudo systemctl restart containerd
```

We can use `dmsetup` to verify that the thin-pool was created successfully.

```bash
$ sudo dmsetup ls
```

 We should also check that `devmapper` is registered and running:

```bash
$ sudo ctr plugins ls | grep devmapper
```

This script needs to be run only once, while setting up the `devmapper` `snapshotter` for `containerd`. Afterwards, make sure that on each reboot, the thin-pool is initialized from the same data directory. Otherwise, all the fetched containers (or the ones that you have created) will be re-initialized. A simple script that re-creates the thin-pool from the same data directory is shown below:

```
#!/bin/bash
set -ex

DATA_DIR=/var/lib/containerd/devmapper
POOL_NAME=devpool

# Allocate loop devices
DATA_DEV=$(sudo losetup --find --show "${DATA_DIR}/data")
META_DEV=$(sudo losetup --find --show "${DATA_DIR}/meta")

# Define thin-pool parameters.
# See https://www.kernel.org/doc/Documentation/device-mapper/thin-provisioning.txt for details.
SECTOR_SIZE=512
DATA_SIZE="$(sudo blockdev --getsize64 -q ${DATA_DEV})"
LENGTH_IN_SECTORS=$(bc <<< "${DATA_SIZE}/${SECTOR_SIZE}")
DATA_BLOCK_SIZE=128
LOW_WATER_MARK=32768

# Create a thin-pool device
sudo dmsetup create "${POOL_NAME}" \
    --table "0 ${LENGTH_IN_SECTORS} thin-pool ${META_DEV} ${DATA_DEV} ${DATA_BLOCK_SIZE} ${LOW_WATER_MARK}"
```

We can create a systemd service to run the above script on each reboot:

```bash
$ sudo nano /lib/systemd/system/devmapper_reload.service
```

The service file:

```
[Unit]
Description=Devmapper reload script

[Service]
ExecStart=/path/to/script/reload.sh

[Install]
WantedBy=multi-user.target
```

Enable the newly created service:

```bash
$ sudo systemctl daemon-reload
$ sudo systemctl enable devmapper_reload.service
$ sudo systemctl start devmapper_reload.service
```

## Configure Kata Containers with AWS Firecracker

To configure Kata Containers with AWS Firecracker, copy the generated `configuration-fc.toml` file when building the `kata-runtime` to either `/etc/kata-containers/configuration-fc.toml` or `/usr/share/defaults/kata-containers/configuration-fc.toml`.

The following command shows full paths to the `configuration.toml` files that the runtime loads. It will use the first path that exists. (Please make sure the kernel and image paths are set correctly in the `configuration.toml` file)

```bash
$ sudo kata-runtime --show-default-config-paths
```

## Configure `containerd`
Next, we need to configure containerd. Add a file in your path (e.g. `/usr/local/bin/containerd-shim-kata-fc-v2`) with the following contents:

```
#!/bin/bash
KATA_CONF_FILE=/etc/kata-containers/configuration-fc.toml /usr/local/bin/containerd-shim-kata-v2 $@
```
> **Note:** You may need to edit the paths of the configuration file and the `containerd-shim-kata-v2` to correspond to your setup.

Make it executable:

```bash
$ sudo chmod +x /usr/local/bin/containerd-shim-kata-fc-v2
```

Add the relevant section in `containerd`’s `config.toml` file (`/etc/containerd/config.toml`):

```
[plugins.cri.containerd.runtimes]
  [plugins.cri.containerd.runtimes.kata-fc]
    runtime_type = "io.containerd.kata-fc.v2"
```

> **Note:** If you are using the default `containerd` configuration (`containerd config default >> /etc/containerd/config.toml`),
> the configuration should change to :
```
[plugins."io.containerd.grpc.v1.cri".containerd.runtimes]
  [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata-fc]
    runtime_type = "io.containerd.kata-fc.v2"
```

Restart `containerd`:

```bash
$ sudo systemctl restart containerd
```

## Verify the installation

We are now ready to launch a container using Kata with Firecracker to verify that everything worked:

```bash
$ sudo ctr images pull --snapshotter devmapper docker.io/library/ubuntu:latest
$ sudo ctr run --snapshotter devmapper --runtime io.containerd.run.kata-fc.v2 -t --rm docker.io/library/ubuntu
```
