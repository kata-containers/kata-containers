#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	get_pod_config_dir
	job_name="jobtest"
	names=( "test1" "test2" "test3" )

	# Create genpolicy settings - common for all of the test jobs
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"

	# Create yaml files
	for i in "${names[@]}"; do
		yaml_file="${pod_config_dir}/job-$i.yaml"
		sed "s/\$ITEM/$i/" ${pod_config_dir}/job-template.yaml > ${yaml_file}
		auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
	done
}

@test "Parallel jobs" {
	# Create the jobs
	for i in "${names[@]}"; do
		kubectl create -f "${pod_config_dir}/job-$i.yaml"
	done

	# Check the jobs
	kubectl get jobs -l jobgroup=${job_name}

	# Check the pods
	kubectl wait --for=condition=Ready --timeout=$timeout pod -l jobgroup=${job_name}

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

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
