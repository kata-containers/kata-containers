#!/usr/bin/env bats
# Copyright (c) 2022 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../../common.bash"

# Check the containerd logged messages on host have a given message.
#
# Parameters:
#      $1 - the message
#
# Note: get the logs since the global $start_date.
#
assert_containerd_logs_contain() {
	local message="$1"
	journalctl -x -t containerd --since "$start_date" | grep "$message"
}

setup() {
	start_date=$(date +"%Y-%m-%d %H:%M:%S")
	sandbox_name="kata-cc-busybox-sandbox"
	pod_config="${FIXTURES_DIR}/pod-config.yaml"
	pod_id=""

	echo "Delete any existing ${sandbox_name} pod"
	crictl_delete_cc_pod_if_exists "$sandbox_name"

	echo "Prepare containerd for Confidential Container"
	SAVED_CONTAINERD_CONF_FILE="/etc/containerd/config.toml.$$"
	configure_cc_containerd "$SAVED_CONTAINERD_CONF_FILE"

	echo "Reconfigure Kata Containers"
	clear_kernel_params
	switch_image_service_offload on
	enable_full_debug

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
	crictl_create_cc_pod "$pod_config"
	crictl_create_cc_container "$sandbox_name" "$pod_config" \
		"$container_config"

	pod_id=$(crictl pods --name "$sandbox_name" -q)
	container_id=$(crictl ps --pod ${pod_id} -q)
	crictl exec "$container_id" cat /proc/cmdline

	# Check that the agent endpoints can be restricted. In this case it will
	# have ExecProcessRequest blocked.
	#
	crictl_delete_cc_pod_if_exists "$sandbox_name"
	# Copy an configuration file to the guest image and pass to the agent.
	cp_to_guest_img "/tests/fixtures" \
		"${FIXTURES_DIR}/${agent_config_filename}"
	add_kernel_params \
		"agent.config_file=/tests/fixtures/${agent_config_filename}"

	crictl_create_cc_pod "$pod_config"
	crictl_create_cc_container "$sandbox_name" "$pod_config" \
		"$container_config"

	pod_id=$(crictl pods --name "$sandbox_name" -q)
	container_id=$(crictl ps --pod ${pod_id} -q)
	# The endpoint ExecProcessRequest is not allowed so this exec
	# operation should fail.
	! crictl exec "$container_id" cat /proc/cmdline

	assert_containerd_logs_contain "ExecProcessRequest is blocked"
}

teardown() {
	# Restore containerd to pre-test state.
	if [ -f "$SAVED_CONTAINERD_CONF_FILE" ]; then
		systemctl stop containerd || true
		sleep 5
		mv -f "$SAVED_CONTAINERD_CONF_FILE" "/etc/containerd/config.toml"
		systemctl start containerd || true
	fi

	# Restore the original guest image file.
	new_guest_img "$saved_img" || true
	rm -f "$saved_img"

	switch_image_service_offload off
	disable_full_debug
}
