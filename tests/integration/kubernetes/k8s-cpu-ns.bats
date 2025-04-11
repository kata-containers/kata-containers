#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "dragonball" ] && skip "test not working see: ${dragonball_limitations}"
	[ "${KATA_HYPERVISOR}" == "cloud-hypervisor" ] && skip "https://github.com/kata-containers/kata-containers/issues/9039"
	[ "${KATA_HYPERVISOR}" == "qemu-runtime-rs" ] && skip "Requires CPU hotplug which isn't supported on ${KATA_HYPERVISOR} yet"
	( [ "${KATA_HYPERVISOR}" == "qemu-tdx" ] || [ "${KATA_HYPERVISOR}" == "qemu-snp" ] || \
		[ "${KATA_HYPERVISOR}" == "qemu-sev" ] || [ "${KATA_HYPERVISOR}" == "qemu-se" ] ) \
		&& skip "TEEs do not support memory / CPU hotplug"


	pod_name="constraints-cpu-test"
	container_name="first-cpu-container"

	weightsyspath="/sys/fs/cgroup/cpu.weight"
	maxsyspath="/sys/fs/cgroup/cpu.max"

	total_cpus=2
	# https://github.com/containers/crun/blob/main/crun.1.md#cgroup-v2
	# The weight is calculated by the:
	# weight = (1 + ((request - 2) * 9999) / 262142)
	total_requests=20
	total_cpu_container=1

	get_pod_config_dir
	yaml_file="${pod_config_dir}/pod-cpu.yaml"

	# Add policy to the yaml file
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	num_cpus_cmd="grep -e '^processor' /proc/cpuinfo |wc -l"
	exec_num_cpus_cmd=(sh -c "${num_cpus_cmd}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_num_cpus_cmd[@]}"

	maxsyspath_cmd="cat ${maxsyspath}"
	exec_maxsyspath_cmd=(sh -c "${maxsyspath_cmd}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_maxsyspath_cmd[@]}"

	weightsyspath_cmd="cat ${weightsyspath}"
	exec_weightsyspath_cmd=(sh -c "${weightsyspath_cmd}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_weightsyspath_cmd[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "Check CPU constraints" {
	# Create the pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	retries="10"

	# Check the total of cpus
	for _ in $(seq 1 "$retries"); do
		# Get number of cpus
		total_cpus_container=$(kubectl exec pod/"$pod_name" -c "$container_name" \
			-- "${exec_num_cpus_cmd[@]}")
		# Verify number of cpus
		[ "$total_cpus_container" -le "$total_cpus" ]
		[ "$total_cpus_container" -eq "$total_cpus" ] && break
		sleep 1
	done
	[ "$total_cpus_container" -eq "$total_cpus" ]

	# Check the total of requests
	total_requests_container=$(kubectl exec $pod_name -c $container_name \
		-- "${exec_weightsyspath_cmd[@]}")
	info "total_requests_container = $total_requests_container"

	[ "$total_requests_container" -eq "$total_requests" ]

	# Check the cpus inside the container

	read total_cpu_quota total_cpu_period <<< $(kubectl exec $pod_name -c $container_name \
		-- "${exec_maxsyspath_cmd[@]}")

	division_quota_period=$(echo $((total_cpu_quota/total_cpu_period)))

	[ "$division_quota_period" -eq "$total_cpu_container" ]
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "dragonball" ] && skip "test not working see: ${dragonball_limitations}"
	[ "${KATA_HYPERVISOR}" == "qemu-runtime-rs" ] && skip "Requires CPU hotplug which isn't supported on ${KATA_HYPERVISOR} yet"
	[ "${KATA_HYPERVISOR}" == "cloud-hypervisor" ] && skip "https://github.com/kata-containers/kata-containers/issues/9039"
	( [ "${KATA_HYPERVISOR}" == "qemu-tdx" ] || [ "${KATA_HYPERVISOR}" == "qemu-snp" ] || \
		[ "${KATA_HYPERVISOR}" == "qemu-sev" ] || [ "${KATA_HYPERVISOR}" == "qemu-se" ] ) \
		&& skip "TEEs do not support memory / CPU hotplug"

	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
