#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	cronjob_name="cron-job-pi-test"
	job_name=""
	pod_name=""

	setup_common || die "setup_common failed"
	yaml_file="${pod_config_dir}/cron-job.yaml"

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "Run a cron job to completion" {
	local cronjob_uid
	local cmd
	local pi_number="3.14"

	# Create cron job
	kubectl apply -f "${yaml_file}"
	cronjob_uid=$(kubectl get cronjob "${cronjob_name}" -o jsonpath='{.metadata.uid}')

	# Capture one owned Job while polling. The subshell isolates waitForProcess's timeout changes.
	cmd="kubectl get jobs -o jsonpath='{range .items[?(@.metadata.ownerReferences[0].uid==\"${cronjob_uid}\")]}{.metadata.name}{\"\\n\"}{end}' | head -n 1 | grep ."
	job_name=$(waitForProcess "${wait_time}" "${sleep_time}" "${cmd}")

	kubectl wait --for=condition=complete "job/${job_name}" --timeout="${timeout}"
	pod_name=$(kubectl get pods --selector="job-name=${job_name}" \
		--field-selector=status.phase=Succeeded -o jsonpath='{.items[0].metadata.name}')
	[[ -n "${pod_name}" ]]

	# Verify the output of the pod
	kubectl logs "${pod_name}" | grep -F "${pi_number}"
}

teardown() {
	# Debugging information
	if [[ -n "${job_name}" ]]; then
		kubectl describe "job/${job_name}"
		kubectl describe pods --selector="job-name=${job_name}"
	fi
	kubectl describe "cronjob/${cronjob_name}"

	# Foreground deletion waits for the dependent Jobs and Pods as well.
	kubectl delete "cronjob/${cronjob_name}" --ignore-not-found \
		--cascade=foreground --wait=true --timeout="${timeout}"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
