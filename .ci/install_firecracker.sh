#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o nounset
set -o pipefail

cidir=$(dirname "$0")
arch=$("${cidir}"/kata-arch.sh -d)
source "${cidir}/lib.sh"

if [ "$arch" != "x86_64" ]; then
	die "Static binaries for Firecracker only available with x86_64."
fi

if [ "$KATA_DEV_MODE" = true ]; then
	die "KATA_DEV_MODE set so not running the test"
fi

docker_version=$(docker version --format '{{.Server.Version}}' | cut -d '.' -f1-2)

if [ "$docker_version" != "18.06" ]; then
	die "Firecracker hypervisor only works with docker 18.06"
fi

# This is the initial release of Kata
# Containers that introduces support for
# the Firecracker hypervisor
release_version="1.5.0-rc2"
file_name="kata-fc-static-${release_version}-${arch}.tar.gz"
url="https://github.com/kata-containers/runtime/releases/download/${release_version}/${file_name}"
echo "Get static binaries from release version ${release_version}"
curl -OL ${url}

echo "Decompress binaries from release version ${release_version}"
sudo tar -xvf ${file_name} -C /

echo "Install and configure docker"
docker_configuration_path="/etc/docker"
# Check if directory exists
[ -d "$docker_configuration_path" ] || sudo mkdir "$docker_configuration_path"

# Check if daemon.json exists
docker_configuration_file=$docker_configuration_path/daemon.json

# For Kata Containers and Firecracker
# a block based driver like devicemapper
# is required
driver="devicemapper"

# From decompressing the tarball, all the files are placed within
# /opt/kata. The runtime configuration is expected to land at
# /opt/kata/share/defaults/kata-containers/configuration.toml
path="/opt/kata/bin/kata-runtime"

if [ -f $docker_configuration_file ]; then
	# Check devicemapper flag
	check_devicemapper=$(grep -w '"storage-driver": "${driver}"' $docker_configuration_file | wc -l)
	[ $check_devicemapper -eq 0 ] && die "${driver} is not enabled at $docker_configuration_file"
	# Check kata runtime flag
	check_kata=$(grep -w '"path": "${path}"' $docker_configuration_file | wc -l)
	[ $check_kata -eq 0 ] && die "Kata Runtime path not found at $docker_configuration_file"
else
	cat <<-EOF | sudo tee "$docker_configuration_file"
	{
	 "runtimes": {
	  "kata-runtime": {
	   "path": "${path}"
	  }
	 },
	 "storage-driver": "${driver}"
	}
	EOF
fi

echo "Restart docker"
sudo systemctl daemon-reload
sudo systemctl restart docker

echo "Check vsock is supported"
check_vsock=$(sudo modprobe vhost_vsock)
if [ $? != 0 ]; then
	die "vsock is not supported on your host system"
fi

# FIXME - we need to create a symbolic link for kata-runtime
# in order that kata-runtime kata-env works
# https://github.com/kata-containers/runtime/issues/1144
sudo ln -s /opt/kata/bin/kata-runtime /usr/local/bin/
