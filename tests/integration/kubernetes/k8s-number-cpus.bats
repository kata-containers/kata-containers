#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[[ "${KATA_HYPERVISOR}" = "cloud-hypervisor" ]]&& skip "test not working https://github.com/kata-containers/kata-containers/issues/9039"
	pod_name="cpu-test"
	container_name="c1"
	get_pod_config_dir
}

# Skip on aarch64 due to missing cpu hotplug related functionality.
@test "Check number of cpus" {
	# Create pod
	kubectl create -f "${pod_config_dir}/pod-number-cpu.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	retries="10"
	max_number_cpus="3"

	num_cpus_cmd='cat /proc/cpuinfo |grep processor|wc -l'
	for _ in $(seq 1 "$retries"); do
		# Get number of cpus
		number_cpus=$(kubectl exec pod/"$pod_name" -c "$container_name" \
			-- sh -c "$num_cpus_cmd")
		if [[ "$number_cpus" =~ ^[0-9]+$ ]]; then
			# Verify number of cpus
			[ "$number_cpus" -le "$max_number_cpus" ]
			[ "$number_cpus" -eq "$max_number_cpus" ] && break
		fi
		sleep 1
	done
}

teardown() {
	[[ "${KATA_HYPERVISOR}" = "cloud-hypervisor" ]]&& skip "test not working https://github.com/kata-containers/kata-containers/issues/9039"
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
}
