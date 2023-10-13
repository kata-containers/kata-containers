#!/usr/bin/env bats
# Copyright (c) 2022 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

test_tag="[cc][agent][cri][containerd]"

setup() {
	setup_common
}

@test "$test_tag Test can launch pod with measured boot enabled" {
	local container_config="${FIXTURES_DIR}/container-config_unsigned-unprotected.yaml"

	switch_measured_rootfs_verity_scheme dm-verity

	create_test_pod

	assert_container "$container_config"
}

@test "$test_tag Test cannot launch pod with measured boot enabled and rootfs modified" {
	local container_config="${FIXTURES_DIR}/container-config_unsigned-unprotected.yaml"

	switch_measured_rootfs_verity_scheme dm-verity
	setup_signature_files

	assert_pod_fail
}

@test "$test_tag Test can pull an unencrypted image inside the guest without signature config" {
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
	local container_config="${FIXTURES_DIR}/container-config.yaml"

	setup_signature_files
	if [ "${SKOPEO:-}" = "yes" ]; then
		add_kernel_params \
			"agent.container_policy_file=/etc/containers/quay_verification/quay_policy.json"
	fi

	create_test_pod

	assert_container "$container_config"
}

@test "$test_tag Test cannot pull an unencrypted unsigned image from a protected registry" {
	local container_config="${FIXTURES_DIR}/container-config_unsigned-protected.yaml"

	setup_signature_files
	if [ "${SKOPEO:-}" = "yes" ]; then
		add_kernel_params \
			"agent.container_policy_file=/etc/containers/quay_verification/quay_policy.json"
	fi

	create_test_pod

	assert_container_fail "$container_config"
	if [ "${SKOPEO:-}" = "yes" ]; then
		assert_logs_contain 'Signature for identity .* is not accepted'
	else
		assert_logs_contain 'Validate image failed: The signatures do not satisfied! Reject reason: \[Match reference failed.\]'
	fi
}

@test "$test_tag Test can pull an unencrypted unsigned image from an unprotected registry" {
	local container_config="${FIXTURES_DIR}/container-config_unsigned-unprotected.yaml"

	setup_signature_files
	if [ "${SKOPEO:-}" = "yes" ]; then
		add_kernel_params \
			"agent.container_policy_file=/etc/containers/quay_verification/quay_policy.json"
	fi

	create_test_pod

	assert_container "$container_config"
}

@test "$test_tag Test unencrypted signed image with unknown signature is rejected" {
	local container_config="${FIXTURES_DIR}/container-config_signed-protected-other.yaml"

	setup_signature_files
	if [ "${SKOPEO:-}" = "yes" ]; then
		add_kernel_params \
			"agent.container_policy_file=/etc/containers/quay_verification/quay_policy.json"
	fi

	create_test_pod

	assert_container_fail "$container_config"
	if [ "${SKOPEO:-}" = "yes" ]; then
		assert_logs_contain "Invalid GPG signature"
	else
		assert_logs_contain 'Validate image failed: The signatures do not satisfied! Reject reason: \[signature verify failed! There is no pubkey can verify the signature!\]'
	fi
}

@test "$test_tag Test unencrypted image signed with cosign" {
	local container_config="${FIXTURES_DIR}/container-config_cosigned.yaml"

	setup_cosign_signatures_files

	create_test_pod

	assert_container "$container_config"
}

@test "$test_tag Test unencrypted image with unknown cosign signature" {
	local container_config="${FIXTURES_DIR}/container-config_cosigned-other.yaml"

	setup_cosign_signatures_files

	create_test_pod

	assert_container_fail "$container_config"
	if [ "${SKOPEO:-}" = "yes" ]; then
		assert_logs_contain 'Signature for identity .* is not accepted'
	else
		assert_logs_contain 'Validate image failed: \[PublicKeyVerifier { key: CosignVerificationKey'
	fi
}

teardown() {
	teardown_common

	# Print the logs and cleanup resources.
	echo "-- Kata logs:"
	# Note - with image-rs we hit more that the default 1000 lines of logs
	sudo journalctl -xe -t kata --since "$test_start_time" -n 100000 
}
