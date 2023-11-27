#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

install_dest="/usr/local/bin"

function get_installed_oras_version() {
	oras version | grep Version | sed -e s/Version:// | tr -d [:blank:]
}

oras_required_version="v1.1.0"
if command -v oras; then
	if [[ "${oras_required_version}" == "v$(get_installed_oras_version)" ]]; then
		echo "ORAS is already installed in the system"
		exit 0
	fi

	echo "Proceeding to cleanup the previous installed version of ORAS, and install the version specified in the versions.yaml file"
	oras_system_path=$(which oras)
	sudo rm -f ${oras_system_path}
fi

arch=$(uname -m)
if [ "${arch}" = "ppc64le" ]; then
 	echo "Building oras from source"
	go_version="go1.21.3"
 	# Install go
 	wget https://go.dev/dl/${go_version}.linux-ppc64le.tar.gz
 	rm -rf /usr/local/go && tar -C /usr/local -xzf ${go_version}.linux-ppc64le.tar.gz
 	export PATH=$PATH:/usr/local/go/bin
 	go version

 	git clone https://github.com/oras-project/oras.git
 	pushd oras 
	make build-linux-ppc64le
 	cp bin/linux/ppc64le/oras ${install_dest}
 	popd 
 	exit 0
 fi
if [ "${arch}" = "x86_64" ]; then
	arch="amd64"
fi
if [ "${arch}" = "aarch64" ]; then
	arch="arm64"
fi
oras_tarball="oras_${oras_required_version#v}_linux_${arch}.tar.gz"

echo "Downloading ORAS ${oras_required_version}"
sudo curl -OL https://github.com/oras-project/oras/releases/download/${oras_required_version}/${oras_tarball}

echo "Installing ORAS to ${install_dest}"
sudo mkdir -p "${install_dest}"
sudo tar -C "${install_dest}" -xzf "${oras_tarball}"
sudo rm -f "${oras_tarball}"
