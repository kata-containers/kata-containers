#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	export KUBECONFIG=/etc/kubernetes/admin.conf
	pod_name="constraints-cpu-test"
	container_name="first-cpu-container"
	sharessyspath="/sys/fs/cgroup/cpu/cpu.shares"
	quotasyspath="/sys/fs/cgroup/cpu/cpu.cfs_quota_us"
	periodsyspath="/sys/fs/cgroup/cpu/cpu.cfs_period_us"
	total_cpus=2
	total_requests=512
	total_cpu_container=1
	pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
}

@test "Check CPU constraints" {
	issue="https://github.com/kata-containers/tests/issues/794"
	[ "${CRI_RUNTIME}" == "containerd" ] && skip "test not working with ${CRI_RUNTIME} see: ${issue}"

	# Create the pod
	sudo -E kubectl create -f "${pod_config_dir}/pod-cpu.yaml"

	# Check pod creation
	sudo -E kubectl wait --for=condition=Ready pod "$pod_name"

	# Check the total of cpus
	total_cpus_container=$(sudo -E kubectl exec $pod_name -c $container_name nproc)

	[ $total_cpus_container -eq $total_cpus ]

	# Check the total of requests
	total_requests_container=$(sudo -E kubectl exec $pod_name -c $container_name cat $sharessyspath)

	[ $total_requests_container -eq $total_requests ]

	# Check the cpus inside the container

	total_cpu_quota=$(sudo -E kubectl exec $pod_name -c $container_name cat $quotasyspath)

	total_cpu_period=$(sudo -E kubectl exec $pod_name -c $container_name cat $periodsyspath)

	division_quota_period=$(echo $((total_cpu_quota/total_cpu_period)))

	[ $division_quota_period -eq $total_cpu_container ]
}

teardown() {
       sudo -E kubectl delete pod "$pod_name"
}
