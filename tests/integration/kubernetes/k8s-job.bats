#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	setup_common
	get_pod_config_dir
	job_name="job-pi-test"
	yaml_file="${pod_config_dir}/job.yaml"
	set_node "${yaml_file}" "${node}"

	# Initialize pod_name here to ensure it's always declared,
	# even if it's an empty string initially.
	# It will be populated later in the @test.
	pod_name=""

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "Run a job to completion" {
	local cmd
	local logs
	local pi_number

	# Create job
	kubectl apply -f "${yaml_file}"

	# Verify job
	waitForProcess "$wait_time" "$sleep_time" "kubectl describe job $job_name | grep SuccessfulCreate"

	# List pods that belong to the job
	pod_name=$(kubectl get pods --selector=job-name=$job_name --output=jsonpath='{.items[*].metadata.name}')

	# Verify that the job is completed
	cmd="kubectl get pods -o jsonpath='{.items[*].status.phase}' | grep Succeeded"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Verify the output of the pod
	bats_unbuffered_info "Getting logs for $pod_name"
	logs=$(kubectl logs "$pod_name")
	bats_unbuffered_info "Logs: $logs"

	pi_number="3.14"
	echo "$logs" | grep "$pi_number"
}

teardown() {
	# Debugging information
	kubectl describe pod "$pod_name"
	kubectl describe jobs/"$job_name"

	# Clean-up
	kubectl delete pod "$pod_name"
	# Verify that pod is not running
	run kubectl get pods
	echo "$output"
	[[ "$output" =~ "No resources found" ]]

	kubectl delete jobs/"$job_name"
	# Verify that the job is not running
	run kubectl get jobs
	echo "${output}"
	[[ "${output}" =~ "No resources found" ]]

	delete_tmp_policy_settings_dir "${policy_settings_dir}"

	teardown_common "${node}" "${node_start_time:-}"
}
