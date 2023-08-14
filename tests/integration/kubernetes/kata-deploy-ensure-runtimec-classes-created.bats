#!/usr/bin/env bats
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	# We expect 2 runtime classes because:
	# * `kata` is the default runtimeclass created, basically an alias for `kata-${KATA_HYPERVISOR}`.
	# * `kata-${KATA_HYPERVISOR}` is the other one
	#    * As part of the tests we're only deploying the specific runtimeclass that will be used, instead of all of them.
	expected_runtime_classes=2
}

@test "Test runtimeclasses are being properly created" {
	# We filter `kata-mshv-vm-isolation` out as that's present on AKS clusters, but that's not coming from kata-deploy
	current_runtime_classes=$(kubectl get runtimeclasses | grep -v "kata-mshv-vm-isolation" | grep "kata" | wc -l)
	[[ ${current_runtime_classes} -eq ${expected_runtime_classes} ]]
	[[ $(kubectl get runtimeclasses | grep -q "${KATA_HYPERVISOR}") -eq 0 ]]
}

teardown() {
	kubectl get runtimeclasses
}
