#!/usr/bin/env bats
# Copyright (c) 2022 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"

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
# Note: the global $sandbox_name, $pod_config and $console_file should be set
# 	already. It also relies on $CI and $DEBUG exported by CI scripts or
# 	the developer, to decide how to set debug flags.
#
create_test_pod() {
	# TODO: add a disable_full_debug to revert the changes on teardown.
	# On CI mode we only want to enable the agent debug for the case of
	# the test failure to obtain logs.
	if [ "${CI:-}" == "true" ]; then
		enable_runtime_debug
		enable_agent_debug
	elif [ "${DEBUG:-}" == "true" ]; then
		enable_full_debug
	fi

	echo "Create the test sandbox"
	crictl_create_cc_pod "$pod_config"

	echo "Connect to the sandbox console"
	console_logger="$(crictl_record_cc_pod_console "$sandbox_name" \
		"$console_file")"
}

# Create container and check it is operational.
#
# Parameters:
#	$1 - the container configuration file.
#
# Note: the global $sandbox_name should be set already.
#
assert_container() {
	local container_config="$1"

	echo "Create the cc container"
	crictl_create_cc_container "$sandbox_name" "$pod_config" \
		"$container_config"

	echo "Check the container is operational"
	local pod_id=$(crictl pods --name "$sandbox_name" -q)
	local container_id=$(crictl ps --pod ${pod_id} -q)
	crictl exec "$container_id" cat /proc/cmdline
}

# Try to create a container when it is expected to fail.
#
# Parameters:
# 	$1 - the container configuration file.
#
# Note: the global $sandbox_name and $pod_config should be set already.
#
assert_container_fail() {
	local container_config="$1"

	echo "Attempt to create the container but it should fail"
	! crictl_create_cc_container "$sandbox_name" "$pod_config" \
		"$container_config" || /bin/false
}

setup() {
	sandbox_name="kata-cc-busybox-sandbox"
	pod_config="${FIXTURES_DIR}/pod-config.yaml"
	pod_id=""

	# Save the VM console logs which are useful in case the test fail. Even
	# if the test doesn't need a pod with console, the file will be created
	# because it is cheap.
	console_file="$(mktemp)"
	# Hold the PID of the process used to record the pod's console.
	console_logger=""

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
	grep 'Signature for identity .* is not accepted' "$console_file"
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
	grep "Invalid GPG signature" "$console_file"
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

	clear_kernel_params
	switch_image_service_offload off

	# Restore containerd to pre-test state.
	if [ -f "$SAVED_CONTAINERD_CONF_FILE" ]; then
		systemctl stop containerd || true
		sleep 5
		mv -f "$SAVED_CONTAINERD_CONF_FILE" "/etc/containerd/config.toml"
		systemctl start containerd || true
	fi
}
