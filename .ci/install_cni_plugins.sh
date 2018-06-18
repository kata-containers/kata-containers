#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

plugins_version=$(get_version "externals.cni-plugins.commit")
echo "Retrieve CNI plugins repository"
go get -d github.com/containernetworking/plugins || true
pushd $GOPATH/src/github.com/containernetworking/plugins
git checkout "$plugins_version"

echo "Build CNI plugins"
./build.sh

echo "Install CNI binaries"
cni_bin_path="/opt/cni"
sudo mkdir -p ${cni_bin_path}
sudo cp -a bin ${cni_bin_path}

echo "Configure CNI"
cni_net_config_path="/etc/cni/net.d"
sudo mkdir -p ${cni_net_config_path}

sudo sh -c 'cat >/etc/cni/net.d/10-mynet.conf <<-EOF
{
	"cniVersion": "0.3.0",
	"name": "mynet",
	"type": "bridge",
	"bridge": "cni0",
	"isGateway": true,
	"ipMasq": true,
	"ipam": {
		"type": "host-local",
		"subnet": "10.88.0.0/16",
		"routes": [
			{ "dst": "0.0.0.0/0"  }
		]
	}
}
EOF'

sudo sh -c 'cat >/etc/cni/net.d/99-loopback.conf <<-EOF
{
	"cniVersion": "0.3.0",
	"type": "loopback"
}
EOF'

popd
