#!/bin/bash
#
# Copyright (c) 2018 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

set -e

filter_test_script="${cidir}/filter/filter_test_union.sh"
config_file="${cidir}/aarch64/configuration_aarch64.yaml"

check_test_union()
{
	local test_union=$(${filter_test_script} ${config_file})
	flag="$1"
	# regex match
	[[ ${test_union} =~ ${flag} ]] && echo "yes" && exit 0

	echo "no"
}

KUBERNETES=$(check_test_union kubernetes)
# if we do k8s integration test, CRI-O is the default CRI runtime
# we have specific env `CRI_CONTAINERD_K8S` for k8s running with containerd-cri.
if [ "$KUBERNETES" == "yes" ]; then
	CRIO="yes"
else
	CRIO="no"
fi
OPENSHIFT=$(check_test_union openshift)
