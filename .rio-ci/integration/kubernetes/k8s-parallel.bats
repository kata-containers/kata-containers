#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"
	get_pod_config_dir
	job_name="jobtest"
	names=( "test1" "test2" "test3" )
}

@test "Parallel jobs" {
	# Create the jobs
	for i in "${names[@]}"; do
		pcl -e APPNAME="${job_name}-${i}" ${pod_config_dir}/job.pcl | kubectl create -f -
	done

	# Check the jobs
	kubectl get jobs -l jobgroup=${job_name}
	kubectl wait --for=condition=complete --timeout=$timeout jobs -l jobgroup=${job_name}

	# Check output of the jobs
	for i in $(kubectl get pods -l jobgroup=${job_name} -o name); do
		kubectl logs ${i}
	done
}

teardown() {
	# Delete jobs
	kubectl delete jobs -l jobgroup=${job_name}
}
