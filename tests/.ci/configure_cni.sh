#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

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
