#!/usr/bin/env bats
#
# Copyright (c) 2026 Chiranjeevi Uddanti
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	pod_name="sandbox-cgroup-pod"

	setup_common || die "setup_common failed"

	yaml_file="${pod_config_dir}/pod-sandbox-cgroup.yaml"
	set_node "$yaml_file" "$node"

	# Add policy to yaml
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

# Regression test for https://github.com/kata-containers/kata-containers/issues/12479
@test "Pod with sandbox_cgroup_only=false starts successfully" {
	# Create pod
	kubectl create -f "${yaml_file}"

	# Wait for pod to be ready
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"
}

teardown() {
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}
