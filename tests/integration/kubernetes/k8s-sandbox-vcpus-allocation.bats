#!/usr/bin/env bats
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR}" == "dragonball" ] || [ "${KATA_HYPERVISOR}" == "cloud-hypervisor" ] && \
		skip "runtime-rs is still using the old vcpus allocation algorithm, skipping the test see https://github.com/kata-containers/kata-containers/issues/8660"
	[ "$(uname -m)" == "aarch64" ] && skip "See: https://github.com/kata-containers/kata-containers/issues/10928"

	setup_common
	get_pod_config_dir
	pods=( "vcpus-less-than-one-with-no-limits" "vcpus-less-than-one-with-limits" "vcpus-more-than-one-with-limits" "vcpus-default-more-than-one" )
	expected_vcpus=( 1 1 2 4 )

	yaml_file="${pod_config_dir}/pod-sandbox-vcpus-allocation.yaml"
	set_node "$yaml_file" "$node"
	add_allow_all_policy_to_yaml "${yaml_file}"
}

@test "Check the number vcpus are correctly allocated to the sandbox" {
	local pod
	local log

	# Create the pods
	kubectl create -f "${yaml_file}"

	# Wait for completion
	kubectl wait --for=jsonpath='{.status.phase}'=Succeeded --timeout=$timeout pod --all

	# Check the pods
	for i in {0..3}; do
		pod="${pods[$i]}"
		bats_unbuffered_info "Getting log for pod: ${pod}"

		log=$(kubectl logs "${pod}")
		bats_unbuffered_info "Log: ${log}"

		[ "${log}" -eq "${expected_vcpus[$i]}" ]
	done
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "dragonball" ] || [ "${KATA_HYPERVISOR}" == "cloud-hypervisor" ] && \
		skip "runtime-rs is still using the old vcpus allocation algorithm, skipping the test see https://github.com/kata-containers/kata-containers/issues/8660"
	[ "$(uname -m)" == "aarch64" ] && skip "See: https://github.com/kata-containers/kata-containers/issues/10928"

	for pod in "${pods[@]}"; do
		kubectl logs ${pod}
	done

	teardown_common "${node}" "${node_start_time:-}"
}
