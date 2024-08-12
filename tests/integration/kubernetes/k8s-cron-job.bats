#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	get_pod_config_dir
	job_name="cron-job-pi-test"
	yaml_file="${pod_config_dir}/cron-job.yaml"

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "Run a cron job to completion" {
	# Create cron job
	kubectl apply -f "${yaml_file}"

	# Verify job
	waitForProcess "$wait_time" "$sleep_time" "kubectl describe cronjobs.batch $job_name | grep SuccessfulCreate"

	# List pods that belong to the cron-job
	pod_name=$(kubectl get pods --no-headers -o custom-columns=":metadata.name" | grep '^cron-job-pi-test' | head -n 1)

	# Verify that the job is completed
	cmd="kubectl get pods -o jsonpath='{.items[*].status.phase}' | grep Succeeded"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Verify the output of the pod
	pi_number="3.14"
	kubectl logs "$pod_name" | grep "$pi_number"
}

teardown() {
	# Debugging information
	kubectl describe pod "$pod_name"
	kubectl describe cronjobs.batch/"$job_name"

	# Clean-up

	kubectl delete cronjobs.batch/"$job_name"
	# Verify that the job is not running
	run kubectl get cronjobs.batch
	echo "$output"
	[[ "$output" =~ "No resources found" ]]

    # Verify that pod is not running
	run kubectl get pods
	echo "$output"
	[[ "$output" =~ "No resources found" ]]

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
