#!/usr/bin/env bats
#
# Copyright (c) 2023 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

# Currently only the Go runtime provides the config path used here.
# If a Rust hypervisor runs this test, mirror the enabling_hypervisor
# pattern in tests/common.bash to select the correct runtime-rs config.
shim_config_file="/opt/kata/share/defaults/kata-containers/configuration-${KATA_HYPERVISOR}.toml"

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

	setup_common || die "setup_common failed"
}

@test "Test cannot launch pod with measured boot enabled and incorrect hash" {
	pod_config="$(new_pod_config nginx "kata-${KATA_HYPERVISOR}")"
	auto_generate_policy "${pod_config_dir}" "${pod_config}"

	incorrect_hash="1111111111111111111111111111111111111111111111111111111111111111"

	# Read verity parameters from config, then override via annotations.
	kernel_verity_params=$(exec_host "$node" "sed -n 's/^kernel_verity_params = \"\\(.*\\)\"/\\1/p' ${shim_config_file}" || true)
	[ -n "${kernel_verity_params}" ] || die "Missing kernel_verity_params in ${shim_config_file}"

	kernel_verity_params=$(printf '%s\n' "$kernel_verity_params" | sed -E "s/root_hash=[^,]*/root_hash=${incorrect_hash}/")
	set_metadata_annotation "$pod_config" \
		"io.katacontainers.config.hypervisor.kernel_verity_params" \
		"${kernel_verity_params}"
	# Run on a specific node so we know from where to inspect the logs
	set_node "$pod_config" "$node"

	# For debug sake
	echo "Pod $pod_config file:"
	cat $pod_config

	assert_pod_fail "$pod_config"
	assert_logs_contain "$node" kata "${node_start_time}" "verity: .* metadata block .* is corrupted"
}

teardown() {
	check_and_skip

	teardown_common "${node}" "${node_start_time:-}"
}
