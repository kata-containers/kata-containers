#!/usr/bin/env bats
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR}" == "dragonball" ] && \
		skip "runtime-rs is still using the old vcpus allocation algorithm, skipping the test"

	get_pod_config_dir
	pods=( "vcpus-less-than-one-with-no-limits" "vcpus-less-than-one-with-limits" "vcpus-more-than-one-with-limits" )
	expected_vcpus=( 1 1 2 )
}

@test "Check the number vcpus are correctly allocated to the sandbox" {
	# Create the pods
	kubectl create -f "${pod_config_dir}/pod-sandbox-vcpus-allocation.yaml"

	# Check the pods
	for i in {0..2}; do
		kubectl wait --for=jsonpath='{.status.conditions[0].reason}'=PodCompleted --timeout=$timeout pod ${pods[$i]}
		[ `kubectl logs ${pods[$i]}` -eq ${expected_vcpus[$i]} ]
	done
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "dragonball" ] && \
		skip "runtime-rs is still using the old vcpus allocation algorithm, skipping the test"

	for pod in "${pods[@]}"; do
		kubectl logs ${pod}
	done

	kubectl delete -f "${pod_config_dir}/pod-sandbox-vcpus-allocation.yaml"
}
