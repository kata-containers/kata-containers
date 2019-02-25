#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

setup() {
	export KUBECONFIG="$HOME/.kube/config"
	get_pod_config_dir
	job_name="jobtest"
	names=( "test1" "test2" "test3" )
}

@test "Parallel jobs" {
	# Create yaml files
	for i in "${names[@]}"; do
		sed "s/\$ITEM/$i/" ${pod_config_dir}/job-template.yaml > ${pod_config_dir}/job-$i.yaml
	done

	# Create the jobs
	for i in "${names[@]}"; do
		kubectl create -f "${pod_config_dir}/job-$i.yaml"
	done

	# Check the jobs
	kubectl get jobs -l jobgroup=${job_name}

	# Check the pods
	kubectl wait --for=condition=Ready pod -l jobgroup=${job_name}

	# Check output of the jobs
	for i in $(kubectl get pods -l jobgroup=${job_name} -o name); do
		kubectl logs ${i}
	done
}

teardown() {
	# Delete jobs
	kubectl delete jobs -l jobgroup=${job_name}

	# Remove generated yaml files
	for i in "${names[@]}"; do
		rm -f ${pod_config_dir}/job-$i.yaml
	done
}
