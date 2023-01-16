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

	create_test_pod

	assert_container "$container_config"
}

@test "$test_tag Test cannot pull an unencrypted unsigned image from a protected registry" {
	local container_config="${FIXTURES_DIR}/container-config_unsigned-protected.yaml"

	setup_signature_files
	
	create_test_pod

	assert_container_fail "$container_config"
	assert_logs_contain 'Validate image failed: The signatures do not satisfied! Reject reason: \[Match reference failed.\]'
}

@test "$test_tag Test can pull an unencrypted unsigned image from an unprotected registry" {
	local container_config="${FIXTURES_DIR}/container-config_unsigned-unprotected.yaml"

	setup_signature_files

	create_test_pod

	assert_container "$container_config"
}

@test "$test_tag Test unencrypted signed image with unknown signature is rejected" {
	local container_config="${FIXTURES_DIR}/container-config_signed-protected-other.yaml"

	setup_signature_files

	create_test_pod

	assert_container_fail "$container_config"
	assert_logs_contain 'Validate image failed: The signatures do not satisfied! Reject reason: \[signature verify failed! There is no pubkey can verify the signature!\]'
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
	assert_logs_contain 'Validate image failed: \[PublicKeyVerifier { key: CosignVerificationKey'
}

@test "$test_tag Test pull an unencrypted unsigned image from an authenticated registry with correct credentials" {
	local container_config="${FIXTURES_DIR}/container-config_authenticated.yaml"

	setup_credentials_files "quay.io/kata-containers/confidential-containers-auth" 

	create_test_pod
	
	assert_container "${container_config}"
}

@test "$test_tag Test cannot pull an image from an authenticated registry with incorrect credentials" {
	local container_config="${FIXTURES_DIR}/container-config_authenticated.yaml"

	REGISTRY_CREDENTIAL_ENCODED="QXJhbmRvbXF1YXl0ZXN0YWNjb3VudHRoYXRkb2VzbnRleGlzdDpwYXNzd29yZAo=" setup_credentials_files "quay.io/kata-containers/confidential-containers-auth"

	create_test_pod

	assert_container_fail "$container_config"
	assert_logs_contain 'failed to pull manifest Authentication failure'
}

@test "$test_tag Test cannot pull an image from an authenticated registry without credentials" {
	local container_config="${FIXTURES_DIR}/container-config_authenticated.yaml"

	create_test_pod

	assert_container_fail "$container_config"
	assert_logs_contain 'failed to pull manifest Not authorized'
}

teardown() {
	teardown_common

	# Print the logs and cleanup resources.
	echo "-- Kata logs:"
	# Note - with image-rs we hit more that the default 1000 lines of logs
	sudo journalctl -xe -t kata --since "$test_start_time" -n 100000 
}
