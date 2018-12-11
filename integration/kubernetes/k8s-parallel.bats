#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	export KUBECONFIG=/etc/kubernetes/admin.conf
	if sudo -E kubectl get runtimeclass | grep -q kata; then
		pod_config_dir="${BATS_TEST_DIRNAME}/runtimeclass_workloads"
	else
		pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
	fi
}

@test "Parallel jobs" {
	job_name="jobtest"
	declare -a names=( test1 test2 test3 )
	# Create yaml files
	for i in "${names[@]}"; do
		sed "s/\$ITEM/$i/" ${pod_config_dir}/job-template.yaml > ${pod_config_dir}/job-$i.yaml
	done

	# Create the jobs
	for i in "${names[@]}"; do
		sudo -E kubectl create -f "${pod_config_dir}/job-$i.yaml"
	done

	# Check the jobs
	sudo -E kubectl get jobs -l jobgroup=${job_name}

	# Check the pods
	sudo -E kubectl wait --for=condition=Ready pod -l jobgroup=${job_name}

	# Check output of the jobs
	for i in $(sudo -E kubectl get pods -l jobgroup=${job_name} -o name); do
		sudo -E kubectl logs ${i}
	done
}

teardown() {
	# Delete jobs
	sudo -E kubectl delete jobs -l jobgroup=${job_name}

	# Remove generated yaml files
	for i in "${names[@]}"; do
		rm -f ${pod_config_dir}/job-$i.yaml
	done
}
