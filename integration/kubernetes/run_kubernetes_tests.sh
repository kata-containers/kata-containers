#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

source /etc/os-release
kubernetes_dir=$(dirname $0)

# Currently, Kubernetes tests only work on Ubuntu.
# We should delete this condition, when it works for other Distros.
if [ "$ID" != ubuntu  ]; then
	echo "Skip Kubernetes tests on $ID"
	echo "kubernetes tests on $ID aren't supported yet"
	exit
fi

# Currently, Kubernetes tests are not working with initrd.
if [ "$TEST_INITRD" = yes ] && [ "$AGENT_INIT" = yes ]; then
	echo "Skip Kubernetes tests on INITRD"
	echo "Issue github.com/kata-containers/tests/issues/335"
	exit
fi

pushd "$kubernetes_dir"
./init.sh
bats nginx.bats
bats k8s-uts+ipc-ns.bats
./cleanup_env.sh
popd
