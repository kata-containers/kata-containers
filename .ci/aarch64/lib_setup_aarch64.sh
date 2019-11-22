#!/bin/bash
#
# Copyright (c) 2018 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

set -e

filter_test_script="${cidir}/${arch}/filter_test_aarch64.sh"

check_test_union()
{
	local test_union=$(bash -f ${filter_test_script})
	flag="$1"
	# regex match
	[[ ${test_union} =~ ${flag} ]] && echo "true"

	echo "false"
}

KUBERNETES=$(check_test_union kubernetes)
# if we do k8s integration test, CRI-O is the default CRI runtime
# we have specific env `CRI_CONTAINERD_K8S` for k8s running with containerd-cri.
if [ "$KUBERNETES" == "true" ]; then
	CRIO="true"
else
	CRIO="false"
fi
OPENSHIFT=$(check_test_union openshift)
