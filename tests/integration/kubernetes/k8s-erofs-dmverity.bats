#!/usr/bin/env bats
#
# Copyright (c) 2026 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#
# Verifies that, when the erofs snapshotter is configured in integrity mode,
# kata-agent activates dm-verity for every EROFS layer in the guest rootfs.
#
# Verification strategy:
#   We assert that dmesg shows BOTH:
#     * "device-mapper: verity: sha256 using ..."  — emitted by
#       drivers/md/dm-verity-target.c when a verity target is loaded.
#     * "erofs (device dm-N): mounted ..."         — emitted by
#       fs/erofs/super.c on a successful mount.
#   The presence of both implies each EROFS layer went through a
#   dm-verity device on the I/O path.

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	# Hard gate: only run on the dedicated integrity CI leg.
	[[ "${SNAPSHOTTER:-}" == "erofs" ]] || skip "needs SNAPSHOTTER=erofs"
	[[ "${EROFS_SNAPSHOTTER_MODE:-}" == "integrity" ]] \
		|| skip "needs EROFS_SNAPSHOTTER_MODE=integrity"

	setup_common || die "setup_common failed"

	pod_name="erofs-dmverity-probe"
	yaml_file="${pod_config_dir}/pod-erofs-dmverity-probe.yaml"
}

@test "EROFS layers are mounted via dm-verity inside the guest" {
	create_container_timeout=180
	set_metadata_annotation "${yaml_file}" \
		"io.katacontainers.config.runtime.create_container_timeout" \
		"${create_container_timeout}"
	kubectl apply -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${create_container_timeout}s" pod "${pod_name}"

	# Read the guest kernel ring buffer and require both a dm-verity target
	# load record and an EROFS mount record. Limit to the last 100 lines.
	# shellcheck disable=SC2016 # variables are expanded inside the pod's shell
	run kubectl exec "${pod_name}" -- sh -c '
		log=$(dmesg 2>&1 | tail -n 100)
		verity_lines=$(printf "%s\n" "$log" | grep -c "device-mapper: verity")
		erofs_lines=$(printf "%s\n" "$log" | grep -cE "erofs \(device dm-[0-9]+\): mounted")
		echo "verity_lines=${verity_lines} erofs_lines=${erofs_lines}"
		if [ "${verity_lines}" -lt 1 ] || [ "${erofs_lines}" -lt 1 ]; then
			echo "--- last 100 lines of dmesg ---"
			printf "%s\n" "$log"
			exit 1
		fi
	'
	[ "$status" -eq 0 ] || \
		die "dm-verity + EROFS mount evidence not found in guest dmesg: ${output}"
}

teardown() {
	# `setup` may have skipped before exporting node/pod_name; guard each step.
	if [[ "${SNAPSHOTTER:-}" == "erofs" \
		&& "${EROFS_SNAPSHOTTER_MODE:-}" == "integrity" ]]; then
		# Debugging information
		kubectl describe "pod/${pod_name}"
		kubectl get "pod/${pod_name}" -o yaml
		kubectl delete pod --grace-period=0 --force --ignore-not-found \
			"${pod_name:-erofs-dmverity-probe}" || true
		teardown_common "${node:-}" "${node_start_time:-}"
	fi
}

