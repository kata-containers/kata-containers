#!/usr/bin/env bats
#
# Copyright (c) 2023 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

check_and_skip() {
	# Currently the only kernel built with measured rootfs support is
	# the kernel-tdx-experimental.
	[ "${KATA_HYPERVISOR}" = "qemu-tdx" ] || \
		skip "measured rootfs tests not implemented for hypervisor: $KATA_HYPERVISOR"
}

setup() {
	check_and_skip
	setup_common
}

teardown() {
	check_and_skip

	kubectl describe -f "${pod_config}" || true
	kubectl delete -f "${pod_config}" || true
}

@test "Test cannnot launch pod with measured boot enabled and incorrect hash" {
	pod_config="$(new_pod_config nginx "kata-${KATA_HYPERVISOR}")"

	incorrect_hash="5180b1568c2ba972e4e06ee0a55976acae8329f2a5d1d2004395635e1ec4a76e"

	# Despite the kernel being built with support, it is not currently enabled
	# on configuration.toml. To avoid editing that file on the worker node,
	# here it will be enabled via pod annotations.
	set_metadata_annotation "$pod_config" \
		"io.katacontainers.config.hypervisor.kernel_params" \
		"rootfs_verity.scheme=dm-verity rootfs_verity.hash=$incorrect_hash"
	# Run on a specific node so we know from where to inspect the logs
	set_node "$pod_config" "$node"

	# For debug sake
	echo "Pod $pod_config file:"
	cat $pod_config

	assert_pod_fail "$pod_config"

	assert_logs_contain "$node" kata "$node_start_time" \
		'verity: .* metadata block .* is corrupted'
}