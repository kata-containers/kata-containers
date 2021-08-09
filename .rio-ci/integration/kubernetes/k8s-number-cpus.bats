#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"
	pod_name="cpu-test"
	container_name="c1"
	get_pod_config_dir
}

# Skip on aarch64 due to missing cpu hotplug related functionality.
@test "Check number of cpus" {
	# Create pod
	pcl "${pod_config_dir}/pod-number-cpu.pcl" | kubectl apply -f -

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	retries="10"
	max_number_cpus="3"

	num_cpus_cmd='cat /proc/cpuinfo |grep processor|wc -l'
	for _ in $(seq 1 "$retries"); do
		# Get number of cpus
		number_cpus=$(kubectl exec pod/"$pod_name" -c "$container_name" \
			-- sh -c "$num_cpus_cmd")
		# Verify number of cpus
		[ "$number_cpus" -le "$max_number_cpus" ]
		[ "$number_cpus" -eq "$max_number_cpus" ] && break
		sleep 1
	done
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
}
