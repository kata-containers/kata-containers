#!/usr/bin/env bats
# Copyright (c) 2022 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	setup_common

	# Test will change the guest image so let's save it to restore
	# on teardown.
	saved_img="$(save_guest_img)"
}

@test "[cc][agent][cri][containerd] Test agent API endpoint can be restricted" {
	local agent_config_filename="agent-configuration-no-exec.toml"
	local container_config="${FIXTURES_DIR}/container-config.yaml"
	local pod_id=""
	local container_id=""

	# Check that the agent allow ExecProcessRequest requests by default.
	#
	echo "Check can create a container and exec a command"
	create_test_pod
	assert_container "$container_config"

	# Check that the agent endpoints can be restricted. In this case it will
	# have ExecProcessRequest blocked.
	#
	crictl_delete_cc_pod_if_exists "$sandbox_name"
	# Copy an configuration file to the guest image and pass to the agent.
	cp_to_guest_img "/tests/fixtures" \
		"${FIXTURES_DIR}/${agent_config_filename}"
	add_kernel_params \
		"agent.config_file=/tests/fixtures/${agent_config_filename}"

	create_test_pod
	crictl_create_cc_container "$sandbox_name" "$pod_config" \
		"$container_config"

	# The endpoint ExecProcessRequest is not allowed so any exec
	# operation should fail.
	echo "Check cannot exec on container"
	! assert_can_exec_on_container
	echo "Check failed to exec because the endpoint is blocked"
	assert_logs_contain "ExecProcessRequest is blocked"
}

teardown() {
	teardown_common

	# Restore the original guest image file.
	new_guest_img "$saved_img" || true
	rm -f "$saved_img"
}
