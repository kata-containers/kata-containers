#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
TEST_INITRD="${TEST_INITRD:-no}"

# Not working on ARM CI see https://github.com/kata-containers/tests/issues/4727
setup() {
	setup_common || die "setup_common failed"
}

@test "Guaranteed QoS" {
	# Skip for SNP/TDX runtime-rs until podOverhead issue is resolved
	# See: https://github.com/kata-containers/kata-containers/pull/13228
	[ "${KATA_HYPERVISOR}" == "qemu-snp-runtime-rs" ] && skip "Skipping Guaranteed QoS test for SNP runtime-rs - podOverhead needs adjustment"
	[ "${KATA_HYPERVISOR}" == "qemu-tdx-runtime-rs" ] && skip "Skipping Guaranteed QoS test for TDX runtime-rs - podOverhead needs adjustment"

	pod_name="qos-test"
	yaml_file="${pod_config_dir}/pod-guaranteed.yaml"
	# Add policy to the yaml file
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"

	# Create pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check pod class
	kubectl get pod "$pod_name" --output=yaml | grep "qosClass: Guaranteed"
}

@test "Burstable QoS" {
	pod_name="burstable-test"
	yaml_file="${pod_config_dir}/pod-burstable.yaml"

	# Add policy to the yaml file
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"

	# Create pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check pod class
	kubectl get pod "$pod_name" --output=yaml | grep "qosClass: Burstable"
}

@test "BestEffort QoS" {
	pod_name="besteffort-test"
	yaml_file="${pod_config_dir}/pod-besteffort.yaml"

	# Add policy to the yaml file
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"

	# Create pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check pod class
	kubectl get pod "$pod_name" --output=yaml | grep "qosClass: BestEffort"
}

teardown() {
	[[ -n "${pod_name:-}" ]] && kubectl delete pod "${pod_name}"
	[[ -n "${policy_settings_dir:-}" ]] && delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
