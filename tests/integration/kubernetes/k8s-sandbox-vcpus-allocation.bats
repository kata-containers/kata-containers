#!/usr/bin/env bats
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR}" == "dragonball" ] || [ "${KATA_HYPERVISOR}" == "cloud-hypervisor" ] && \
		skip "runtime-rs is still using the old vcpus allocation algorithm, skipping the test see https://github.com/kata-containers/kata-containers/issues/8660"
	[ "${KATA_HYPERVISOR}" = "qemu-runtime-rs" ] && skip "Requires CPU hotplug which isn't supported on ${KATA_HYPERVISOR} yet"

	get_pod_config_dir
	pods=( "vcpus-less-than-one-with-no-limits" "vcpus-less-than-one-with-limits" "vcpus-more-than-one-with-limits" )
	expected_vcpus=( 1 1 2 )

	yaml_file="${pod_config_dir}/pod-sandbox-vcpus-allocation.yaml"
	add_allow_all_policy_to_yaml "${yaml_file}"
}

@test "Check the number vcpus are correctly allocated to the sandbox" {
	# Create the pods
	kubectl create -f "${yaml_file}"

	# Wait for completion
	kubectl wait --for=jsonpath='{.status.phase}'=Succeeded --timeout=$timeout pod --all

	# Check the pods
	for i in {0..2}; do
		[ `kubectl logs ${pods[$i]}` -eq ${expected_vcpus[$i]} ]
	done
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "dragonball" ] || [ "${KATA_HYPERVISOR}" == "cloud-hypervisor" ] && \
		skip "runtime-rs is still using the old vcpus allocation algorithm, skipping the test see https://github.com/kata-containers/kata-containers/issues/8660"
	[ "${KATA_HYPERVISOR}" = "qemu-runtime-rs" ] && skip "Requires CPU hotplug which isn't supported on ${KATA_HYPERVISOR} yet"

	for pod in "${pods[@]}"; do
		kubectl logs ${pod}
	done

	kubectl delete -f "${yaml_file}"
}
