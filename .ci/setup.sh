#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

cidir=$(dirname "$0")
bash "${cidir}/static-checks.sh"

#Note: If add clearlinux as supported CI use a stateless os-release file
source /etc/os-release

if [ "$ID" == fedora ];then
	sudo -E dnf -y install automake  bats
elif [ "$ID" == ubuntu ];then
	#bats isn't available for Ubuntu trusty, need for travis
	sudo add-apt-repository -y ppa:duggan/bats
	sudo apt-get -qq update
	sudo apt-get install -y -qq automake bats qemu-utils
else 
	echo "Linux distribution not supported"
fi
