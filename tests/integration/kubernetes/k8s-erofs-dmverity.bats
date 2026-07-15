#!/usr/bin/env bats
#
# Copyright (c) 2026 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#
# Verifies that, when the erofs snapshotter is configured with dm-verity,
# kata-agent activates dm-verity for every workload EROFS layer mounted
# inside the guest.
#
# Verification strategy:
#   The pod derives the expected workload layer count from the root overlay's
#   lowerdirs, then requires the same number of kata-verity devices and
#   dm-backed EROFS mounts. The guest rootfs and pause container are outside
#   this test's scope. The command exits 0 on success and 1 on failure; with
#   restartPolicy: Never the pod reaches Succeeded or Failed respectively,
#   which the test polls for.

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	# Hard gate: only run on CI legs with erofs + dm-verity enabled.
	[[ "${SNAPSHOTTER:-}" == "erofs" ]] || skip "needs SNAPSHOTTER=erofs"
	[[ "${EROFS_DMVERITY:-}" == "dmverity" ]] \
		|| skip "needs EROFS_DMVERITY=dmverity"

	# Auto-generated policy support for the erofs dm-verity scenario is not
	# yet validated — genpolicy may not correctly handle the dm-verity
	# storage configuration this test relies on. Skip rather than fail
	# unpredictably when policy enforcement is active.
	auto_generate_policy_enabled && skip "auto-generated policy not yet supported for erofs-dmverity"

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

	# The container command runs the dmesg probe then exits. With
	# restartPolicy: Never the pod reaches Succeeded (probe passed) or
	# Failed (probe exited non-zero). Poll until a terminal phase is
	# reached so that a failure is reported promptly rather than waiting
	# for the full create_container_timeout.
	local deadline=$((SECONDS + create_container_timeout))
	local phase=""
	while (( SECONDS < deadline )); do
		phase=$(kubectl get pod "${pod_name}" \
			-o jsonpath='{.status.phase}' 2>/dev/null || true)
		case "${phase}" in
			Succeeded|Failed) break ;;
		esac
		sleep 2
	done

	case "${phase}" in
		Succeeded)
			# Probe passed — surface the diagnostic line in the CI log.
			kubectl logs "${pod_name}" 2>/dev/null || true
			;;
		Failed)
			# Probe exited non-zero — retrieve its logs before failing.
			kubectl logs "${pod_name}" 2>/dev/null || true
			die "dm-verity + EROFS mount evidence not found in guest dmesg"
			;;
		*)
			# Timed out without reaching Succeeded or Failed.
			kubectl logs "${pod_name}" 2>/dev/null || true
			die "timed out waiting for pod ${pod_name} (phase=${phase:-unknown})"
			;;
	esac
}

teardown() {
	# `setup` may have skipped before exporting node/pod_name; guard each step.
	if [[ "${SNAPSHOTTER:-}" == "erofs" \
		&& "${EROFS_DMVERITY:-}" == "dmverity" ]]; then
		# Debugging information
		kubectl describe "pod/${pod_name}"
		kubectl get "pod/${pod_name}" -o yaml
		kubectl delete pod --grace-period=0 --force --ignore-not-found \
			"${pod_name:-erofs-dmverity-probe}" || true
		teardown_common "${node:-}" "${node_start_time:-}"
	fi
}
