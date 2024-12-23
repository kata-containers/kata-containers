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
			return
			;;
		*)
			if [ "${KATA_HOST_OS}" != "cbl-mariner" ]; then
				skip "measured rootfs tests not implemented for: (hypervisor: $KATA_HYPERVISOR, host os: $KATA_HOST_OS)"
			fi
			;;
	esac
}

setup() {
	check_and_skip
	setup_common || die "setup_common failed"
}

get_kernel_params() {
	case "${KATA_HYPERVISOR}" in
		qemu-tdx|qemu-coco-dev)
			local incorrect_hash="1111111111111111111111111111111111111111111111111111111111111111"
			echo "rootfs_verity.scheme=dm-verity rootfs_verity.hash=$incorrect_hash"
			;;
		*)
			if [ "${KATA_HOST_OS}" == "cbl-mariner" ]; then
				echo "$(get_mariner_kernel_params true)"
			fi
			;;
	esac
}

set_mariner_annotations() {
	config_file="$1"

	local mariner_kernel_path="/usr/share/cloud-hypervisor/vmlinux.bin"
	set_metadata_annotation "$config_file" \
		"io.katacontainers.config.hypervisor.kernel" \
		"$mariner_kernel_path"

	local mariner_image_path="/opt/kata/share/kata-containers/kata-containers-mariner.img"
	set_metadata_annotation "$config_file" \
		"io.katacontainers.config.hypervisor.image" \
		"$mariner_image_path"

	local mariner_rootfs_type="custom"
	set_metadata_annotation "$config_file" \
		"io.katacontainers.config.hypervisor.rootfs_type" \
		"$mariner_rootfs_type"
}

@test "Test cannnot launch pod with measured boot enabled and incorrect hash" {
	pod_config="$(new_pod_config nginx "kata-${KATA_HYPERVISOR}")"

	# To avoid editing that file on the worker node, here it will be
	# enabled via pod annotations.
	local kernel_params="$(get_kernel_params)"
	info "kernel_params = $kernel_params"

	set_metadata_annotation "$pod_config" \
		"io.katacontainers.config.hypervisor.kernel_params" \
		"$kernel_params"

	if [ "${KATA_HOST_OS}" == "cbl-mariner" ]; then
		set_mariner_annotations "$pod_config"
	fi

	# Run on a specific node so we know from where to inspect the logs
	set_node "$pod_config" "$node"

#	Skip adding the policy, as it's causing the test to fail.
#	See more details on: https://github.com/kata-containers/kata-containers/issues/9612
#	# Add an "allow all" policy if policy testing is enabled.
#	add_allow_all_policy_to_yaml "$pod_config"

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
