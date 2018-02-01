#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source /etc/os-release
source "${cidir}/lib.sh"

echo "Set up environment"
if [ "$ID" == ubuntu ];then
	bash -f "${cidir}/setup_env_ubuntu.sh"
else
	die "ERROR: Unrecognised distribution."
	exit 1
fi

echo "Install shim"
bash -f ${cidir}/install_shim.sh

echo "Install proxy"
bash -f ${cidir}/install_proxy.sh

echo "Install runtime"
bash -f ${cidir}/install_runtime.sh

echo "Drop caches"
sync
sudo -E PATH=$PATH bash -c "echo 3 > /proc/sys/vm/drop_caches"
