#!/usr/bin/env bats
#
# Copyright (c) 2025 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	setup_common
	get_pod_config_dir

    pod_name="privileged"
	yaml_file="${pod_config_dir}/pod-privileged.yaml"

    cmd_nsenter=(nsenter --mount=/proc/1/ns/mnt true)

    policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_exec_to_policy_settings "${policy_settings_dir}" "${cmd_nsenter[@]}"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

# This should succeed because the CI uses kata-deploy which sets
# privileged_without_host_devices to true.
@test "Privileged pod runs and is able to execute privileged operations" {
	kubectl apply -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"
    kubectl exec "${pod_name}" -- "${cmd_nsenter[@]}"
}

teardown() {
	echo "Pod logs:"
	kubectl logs "${pod_name}" || true

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
