#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

source /etc/os-release
openshift_dir=$(dirname $0)

# Currently, the CI runs Openshift tests on Fedora.
if [ "$ID" != "fedora" ] && [ "$CI" == true ]; then
	echo "Skip Openshift tests on $ID"
	echo "CI only runs openshift tests on fedora"
	exit
fi

pushd "$openshift_dir"
./init.sh
bats hello_world.bats
./teardown.sh
popd
