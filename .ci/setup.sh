#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

#Note: If add clearlinux as supported CI use a stateless os-release file
source /etc/os-release

if [ "$ID" == fedora ];then
	sudo -E dnf -y install automake yamllint coreutils moreutils
elif [ "$ID" == centos ];then
	sudo -E yum -y install epel-release
	sudo -E yum -y install automake yamllint coreutils moreutils
elif [ "$ID" == ubuntu ];then
	sudo apt-get -qq update
	sudo apt-get install -y -qq make automake qemu-utils python-pip coreutils moreutils
	sudo pip install yamllint
else 
	echo "Linux distribution not supported"
fi

bash "${cidir}/static-checks.sh"
