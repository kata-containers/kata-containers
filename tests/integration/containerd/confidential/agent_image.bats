#!/usr/bin/env bats
# Copyright (c) 2022 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/asserts.sh"

# Currently the agent can only check images signature if using skopeo.
# There isn't a way to probe the agent to determine if skopeo is present
# or not, so we need to rely on build variables. If we are running under
# CI then we assume the variables are properly exported, otherwise we
# should skip testing.
#
skip_if_skopeo_not_present () {
	if [ "${CI:-}" == "true" ]; then
		if [ "${SKOPEO:-no}" == "no" ]; then
			skip "Skopeo seems not installed in guest"
		fi
	else
		skip "Cannot determine skopeo is installed in guest"
	fi
}

# Create the test pod.
#
# Note: the global $sandbox_name, $pod_config should be set
# 	already. It also relies on $CI and $DEBUG exported by CI scripts or
# 	the developer, to decide how to set debug flags.
#
create_test_pod() {
	# On CI mode we only want to enable the agent debug for the case of
	# the test failure to obtain logs.
	if [ "${CI:-}" == "true" ]; then
		enable_full_debug
	elif [ "${DEBUG:-}" == "true" ]; then
		enable_full_debug
		enable_agent_console
	fi

	echo "Create the test sandbox"
	crictl_create_cc_pod "$pod_config"
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
	switch_image_service_offload on
	clear_kernel_params
}

@test "[cc][agent][cri][containerd] Test can pull an unencrypted image inside the guest" {
	local container_config="${FIXTURES_DIR}/container-config.yaml"

	create_test_pod

	assert_container "$container_config"

	echo "Check the image was not pulled in the host"
	local pod_id=$(crictl pods --name "$sandbox_name" -q)
	rootfs=($(find /run/kata-containers/shared/sandboxes/${pod_id}/shared \
		-name rootfs))
	[ ${#rootfs[@]} -eq 1 ]

	# TODO: confirm the image was pulled in the guest. `kata-runtime exec`
	#       can be used to get a shell prompt which is not exactly what we
	#       want but can be used to with `expect` to implement a
	#       run-command-and-disconnect mechanism.
}

@test "[cc][agent][cri][containerd] Test can pull a unencrypted signed image from a protected registry" {
	skip_if_skopeo_not_present
	local container_config="${FIXTURES_DIR}/container-config.yaml"

	add_kernel_params \
		"agent.container_policy_file=/etc/containers/quay_verification/quay_policy.json"

	create_test_pod

	assert_container "$container_config"
}

@test "[cc][agent][cri][containerd] Test cannot pull an unencrypted unsigned image from a protected registry" {
	skip_if_skopeo_not_present
	local container_config="${FIXTURES_DIR}/container-config_unsigned-protected.yaml"

	add_kernel_params \
		"agent.container_policy_file=/etc/containers/quay_verification/quay_policy.json"

	create_test_pod

	assert_container_fail "$container_config"

	assert_logs_contain 'Signature for identity .* is not accepted'
}

@test "[cc][agent][cri][containerd] Test can pull an unencrypted unsigned image from an unprotected registry" {
	skip_if_skopeo_not_present
	local container_config="${FIXTURES_DIR}/container-config_unsigned-unprotected.yaml"

	add_kernel_params \
		"agent.container_policy_file=/etc/containers/quay_verification/quay_policy.json"

	create_test_pod

	assert_container "$container_config"
}

@test "[cc][agent][cri][containerd] Test unencrypted signed image with unknown signature is rejected" {
	skip_if_skopeo_not_present
	local container_config="${FIXTURES_DIR}/container-config_signed-protected-other.yaml"

	add_kernel_params \
		"agent.container_policy_file=/etc/containers/quay_verification/quay_policy.json"

	create_test_pod

	assert_container_fail "$container_config"
	assert_logs_contain "Invalid GPG signature"
}

teardown() {
	# Print the logs and cleanup resources.
	echo "-- Kata logs:"
	sudo journalctl -xe -t kata --since "$start_date"

	# Allow to not destroy the environment if you are developing/debugging
	# tests.
	if [[ "${CI:-false}" == "false" && "${DEBUG:-}" == true ]]; then
		echo "Leaving changes and created resources untoughted"
		return
	fi

	crictl_delete_cc_pod_if_exists "$sandbox_name" || true

	clear_kernel_params
	switch_image_service_offload off
	disable_full_debug

	# Restore containerd to pre-test state.
	if [ -f "$SAVED_CONTAINERD_CONF_FILE" ]; then
		systemctl stop containerd || true
		sleep 5
		mv -f "$SAVED_CONTAINERD_CONF_FILE" "/etc/containerd/config.toml"
		systemctl start containerd || true
	fi
}
