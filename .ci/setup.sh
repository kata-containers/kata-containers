#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

source /etc/os-release

echo  "Setup script for packaging"

if [ "$ID" == ubuntu ];then
	echo "Install snap dependencies"
	sudo apt-get install -y snapd snapcraft

	echo "Install kernel dependencies"
	sudo -E apt install -y libelf-dev bc gcc
fi
