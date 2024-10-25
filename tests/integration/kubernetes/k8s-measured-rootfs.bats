#!/usr/bin/env bats
#
# Copyright (c) 2023 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

check_and_skip() {
	if [ "${KATA_HYPERVISOR}" != "qemu-tdx" ]; then
		skip "measured rootfs tests not implemented for hypervisor: $KATA_HYPERVISOR"
	fi
}

setup() {
	check_and_skip
	setup_common
}

@test "Test cannnot launch pod with measured boot enabled and incorrect hash" {
	pod_config="$(new_pod_config nginx "kata-${KATA_HYPERVISOR}")"

	incorrect_hash="5180b1568c2ba972e4e06ee0a55976acae8329f2a5d1d2004395635e1ec4a76e"

	# To avoid editing that file on the worker node, here it will be
	# enabled via pod annotations.
	set_metadata_annotation "$pod_config" \
		"io.katacontainers.config.hypervisor.kernel_params" \
		"rootfs_verity.scheme=dm-verity rootfs_verity.hash=$incorrect_hash"
	# Run on a specific node so we know from where to inspect the logs
	set_node "$pod_config" "$node"

#	Skip adding the policy, as it's causing the test to fail.
#	See more details on: https://github.com/kata-containers/kata-containers/issues/9612
#	# Add an "allow all" policy if policy testing is enabled.
#	add_allow_all_policy_to_yaml "$pod_config"

	# For debug sake
	echo "Pod $pod_config file:"
	cat $pod_config

	assert_pod_fail "$pod_config"

	assert_logs_contain "$node" kata "$node_start_time" \
		'verity: .* metadata block .* is corrupted'
}

teardown() {
	check_and_skip

	teardown_common "${node}" "${node_start_time:-}"
}
