#!/usr/bin/env bats
# Copyright (c) 2022 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

test_tag="[cc][agent][cri][containerd]"

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

setup() {
	setup_common
	if [ "${SKOPEO:-}" == "yes" ]; then
		setup_skopeo_signature_files_in_guest
	else
		setup_offline_fs_kbc_signature_files_in_guest
	fi
}

@test "$test_tag Test can pull an unencrypted image inside the guest" {
	local container_config="${FIXTURES_DIR}/container-config.yaml"

	create_test_pod

	assert_container "$container_config"

	echo "Check the image was not pulled in the host"
	local pod_id=$(crictl pods --name "$sandbox_name" -q)
	rootfs=($(find /run/kata-containers/shared/sandboxes/${pod_id}/shared \
		-name rootfs))
	[ ${#rootfs[@]} -eq 1 ]
}

@test "$test_tag Test can pull a unencrypted signed image from a protected registry" {
	skip_if_skopeo_not_present
	local container_config="${FIXTURES_DIR}/container-config.yaml"

	add_kernel_params \
		"agent.container_policy_file=/etc/containers/quay_verification/quay_policy.json"

	create_test_pod

	assert_container "$container_config"
}

@test "$test_tag Test cannot pull an unencrypted unsigned image from a protected registry" {
	skip_if_skopeo_not_present
	local container_config="${FIXTURES_DIR}/container-config_unsigned-protected.yaml"

	add_kernel_params \
		"agent.container_policy_file=/etc/containers/quay_verification/quay_policy.json"

	create_test_pod

	assert_container_fail "$container_config"

	assert_logs_contain 'Signature for identity .* is not accepted'
}

@test "$test_tag Test can pull an unencrypted unsigned image from an unprotected registry" {
	skip_if_skopeo_not_present
	local container_config="${FIXTURES_DIR}/container-config_unsigned-unprotected.yaml"

	add_kernel_params \
		"agent.container_policy_file=/etc/containers/quay_verification/quay_policy.json"

	create_test_pod

	assert_container "$container_config"
}

@test "$test_tag Test unencrypted signed image with unknown signature is rejected" {
	skip_if_skopeo_not_present
	local container_config="${FIXTURES_DIR}/container-config_signed-protected-other.yaml"

	add_kernel_params \
		"agent.container_policy_file=/etc/containers/quay_verification/quay_policy.json"

	create_test_pod

	assert_container_fail "$container_config"
	assert_logs_contain "Invalid GPG signature"
}

teardown() {
	teardown_common

	# Print the logs and cleanup resources.
	echo "-- Kata logs:"
	sudo journalctl -xe -t kata --since "$test_start_time"
}
