#!/usr/bin/env bats
# Copyright (c) 2022 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"

setup() {
	sandbox_name="kata-cc-busybox-sandbox"
	crictl_delete_cc_pod_if_exists "$sandbox_name"

	echo "Prepare containerd for Confidential Container"
	SAVED_CONTAINERD_CONF_FILE="/etc/containerd/config.toml.$$"
	configure_cc_containerd "$SAVED_CONTAINERD_CONF_FILE"

	echo "Reconfigure Kata Containers"
	switch_image_service_offload on
}

@test "[cc][agent][cri][containerd] Test can pull an unencrypted image inside the guest" {
	local pod_config="${FIXTURES_DIR}/pod-config.yaml"
	local container_config="${FIXTURES_DIR}/container-config.yaml"

	# TODO: add a disable_full_debug to revert the changes on teardown.

	# On CI mode we only want to enable the agent debug for the case of
	# the test failure to obtain logs.
	if [ "${CI:-}" == "true" ]; then
		enable_runtime_debug
		enable_agent_debug
	elif [ "${DEBUG:-}" == "true" ]; then
		enable_full_debug
	fi

	echo "Create the sandbox"
	crictl_create_cc_pod "$pod_config"

	# Save the VM console logs which are useful in case the test fail.
	console_file="$(mktemp)"
	console_logger="$(crictl_record_cc_pod_console "$sandbox_name" \
		"$console_file")"

	echo "Create the cc container"
	crictl_create_cc_container "$sandbox_name" "$pod_config" \
		"$container_config"

	echo "Check the container is operational"
	local pod_id=$(crictl pods --name "$sandbox_name" -q)
	local container_id=$(crictl ps --pod ${pod_id} -q)
	crictl exec "$container_id" cat /proc/cmdline

	echo "Check the image was not pulled in the host"
	rootfs=($(find /run/kata-containers/shared/sandboxes/${pod_id}/shared \
		-name rootfs))
	[ ${#rootfs[@]} -eq 1 ]

	# TODO: confirm the image was pulled in the guest. `kata-runtime exec`
	#       can be used to get a shell prompt which is not exactly what we
	#       want but can be used to with `expect` to implement a
	#       run-command-and-disconnect mechanism.
}

teardown() {
	# Print the console logs and cleanup resources.
	if [[ -n "$console_logger" && -d "/proc/${console_logger}" ]]; then
		echo "-- VM console:"
		kill -9 "$console_logger" || true
		cat "$console_file"
	fi

	# Allow to not destroy the environment if you are developing/debugging
	# tests.
	if [[ "${CI:-false}" == "false" && "${DEBUG:-}" == true ]]; then
		echo "Leaving changes and created resources untoughted"
		return
	fi

	rm -f "$console_file"

	crictl_delete_cc_pod_if_exists "$sandbox_name" || true

	switch_image_service_offload off

	# Restore containerd to pre-test state.
	if [ -f "$SAVED_CONTAINERD_CONF_FILE" ]; then
		systemctl stop containerd || true
		sleep 5
		mv -f "$SAVED_CONTAINERD_CONF_FILE" "/etc/containerd/config.toml"
		systemctl start containerd || true
	fi
}
