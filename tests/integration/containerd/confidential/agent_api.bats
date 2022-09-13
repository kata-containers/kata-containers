#!/usr/bin/env bats
# Copyright (c) 2022 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
load "${BATS_TEST_DIRNAME}/../../confidential/lib.sh"

setup() {
	setup_common

	# Test will change the guest image so let's save it to restore
	# on teardown.
	saved_img="$(save_guest_img)"

	agent_config_file="${FIXTURES_DIR}/agent-configuration-no-exec.toml"

	# In case the tests run behind a firewall where images needed to be fetched
	# through a proxy.
	local https_proxy="${HTTPS_PROXY:-${https_proxy:-}}"
	if [ -n "$https_proxy" ]; then
		local new_file="${BATS_FILE_TMPDIR}/$(basename ${agent_config_file})"
		echo "https_proxy = '${https_proxy}'" > "$new_file"
		cat $agent_config_file >> "$new_file"
		agent_config_file="$new_file"
		# Print the file content just for the sake of debugging.
		echo "New agent configure file with HTTPS proxy: $agent_config_file"
		cat $agent_config_file
	fi
}

@test "[cc][agent][cri][containerd] Test agent API endpoint can be restricted" {
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
	cp_to_guest_img "/tests/fixtures" "${agent_config_file}"
	add_kernel_params \
		"agent.config_file=/tests/fixtures/$(basename ${agent_config_file})"

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

	# Print the logs
	echo "-- Kata, containerd, crio logs:"
	# Note - with image-rs we can hit much more that the default 1000 lines of logs
	local cmd="journalctl -x"
	for syslog_id in kata containerd crio;do
		cmd+=" -t \"$syslog_id\""
	done
	cmd+=" --since \"$test_start_time\" -n 100000"
	eval ${cmd}

	# Restore the original guest image file.
	new_guest_img "$saved_img" || true
	rm -f "$saved_img"
}
