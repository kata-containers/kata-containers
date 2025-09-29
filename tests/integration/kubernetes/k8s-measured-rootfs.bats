#!/usr/bin/env bats
#
# Copyright (c) 2023 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

check_and_skip() {
	case "${KATA_HYPERVISOR}" in
		qemu-tdx|qemu-coco-dev)
			if [ "$(uname -m)" == "s390x" ]; then
				skip "measured rootfs tests not implemented for s390x"
			fi
			return
			;;
		*)
			skip "measured rootfs tests not implemented for hypervisor: $KATA_HYPERVISOR"
			;;
	esac
}

setup() {
	check_and_skip
	get_pod_config_dir
	setup_common || die "setup_common failed"
}

@test "Test cannnot launch pod with measured boot enabled and incorrect hash" {
	pod_config="$(new_pod_config nginx "kata-${KATA_HYPERVISOR}")"
	auto_generate_policy "${pod_config_dir}" "${pod_config}"

	incorrect_hash="1111111111111111111111111111111111111111111111111111111111111111"

	# To avoid editing that file on the worker node, here it will be
	# enabled via pod annotations.
	set_metadata_annotation "$pod_config" \
		"io.katacontainers.config.hypervisor.kernel_params" \
		"rootfs_verity.scheme=dm-verity rootfs_verity.hash=$incorrect_hash"
	# Run on a specific node so we know from where to inspect the logs
	set_node "$pod_config" "$node"

	# For debug sake
	echo "Pod $pod_config file:"
	cat $pod_config
	kubectl apply -f $pod_config

	waitForProcess "60" "3" "exec_host $node journalctl -t kata | grep \"verity: .* metadata block .* is corrupted\""
}

teardown() {
	check_and_skip

	teardown_common "${node}" "${node_start_time:-}"
}
