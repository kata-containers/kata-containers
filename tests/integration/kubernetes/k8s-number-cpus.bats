#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR}" = "cloud-hypervisor" ] && skip "test not working https://github.com/kata-containers/kata-containers/issues/9039"
	[ "${KATA_HYPERVISOR}" = "qemu-runtime-rs" ] && skip "Requires CPU hotplug which isn't supported on ${KATA_HYPERVISOR} yet"
	pod_name="cpu-test"
	container_name="c1"
	get_pod_config_dir
	yaml_file="${pod_config_dir}/pod-number-cpu.yaml"

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	num_cpus_cmd='cat /proc/cpuinfo |grep processor|wc -l'
	exec_command=(sh -c "${num_cpus_cmd}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

# Skip on aarch64 due to missing cpu hotplug related functionality.
@test "Check number of cpus" {
	# Create pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	retries="10"
	max_number_cpus="3"

	for _ in $(seq 1 "$retries"); do
		# Get number of cpus
		number_cpus=$(kubectl exec pod/"$pod_name" -c "$container_name" \
			-- "${exec_command[@]}")
		if [[ "$number_cpus" =~ ^[0-9]+$ ]]; then
			# Verify number of cpus
			[ "$number_cpus" -le "$max_number_cpus" ]
			[ "$number_cpus" -eq "$max_number_cpus" ] && break
		fi
		sleep 1
	done
}

teardown() {
	[ "${KATA_HYPERVISOR}" = "cloud-hypervisor" ] && skip "test not working https://github.com/kata-containers/kata-containers/issues/9039"
	[ "${KATA_HYPERVISOR}" = "qemu-runtime-rs" ] && skip "Requires CPU hotplug which isn't supported on ${KATA_HYPERVISOR} yet"
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
