#!/usr/bin/env bats
#
# Copyright (c) NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

export KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"

readonly QEMU_SANDBOX_PARAM="on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny"
readonly QEMU_SANDBOX_POD_TIMEOUT="${QEMU_SANDBOX_POD_TIMEOUT:-30s}"

qemu_rootless_sandbox_supported() {
	[[ "${KATA_HYPERVISOR}" == qemu* ]] || return 1

	# Additional QEMU configurations are tracked in:
	# https://github.com/kata-containers/kata-containers/issues/13424
	is_shared_fs_none_runtime_class "${KATA_HYPERVISOR}" && return 1
	# Rootless QEMU cannot access EROFS layers below root-owned
	# snapshot directories.
	[[ "${SNAPSHOTTER:-}" == "erofs" ]] && return 1
	is_confidential_runtime_class "${KATA_HYPERVISOR}" && return 1
	return 0
}

setup() {
	local runtime_config_dropin_file
	local seccomp_key

	if ! qemu_rootless_sandbox_supported; then
		skip "QEMU rootless and seccomp sandbox smoke testing does not cover ${KATA_HYPERVISOR}"
	fi

	setup_common || die "setup_common failed"

	pod_name="test-e2e"
	pod_config="$(new_pod_config \
		"quay.io/prometheus/busybox:latest" \
		"kata-${KATA_HYPERVISOR}")"
	set_node "${pod_config}" "${node}"
	set_container_command "${pod_config}" 0 sleep 30

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	auto_generate_policy "${policy_settings_dir}" "${pod_config}"

	if is_runtime_rs; then
		seccomp_key="seccomp_sandbox"
	else
		seccomp_key="seccompsandbox"
	fi

	runtime_config_dropin_file="${BATS_FILE_TMPDIR}/99-k8s-qemu-sandbox.toml"
	cat > "${runtime_config_dropin_file}" <<EOF
[hypervisor.qemu]
rootless = true
${seccomp_key} = "${QEMU_SANDBOX_PARAM}"
EOF

	runtime_config_dropin="$(set_kata_runtime_config_dropin_file \
		"${node}" \
		"${runtime_config_dropin_file}")" || \
		die "Failed to install QEMU sandbox config drop-in on ${node}"
}

@test "QEMU runs rootless with its seccomp sandbox enabled" {
	local cmdline
	local qemu_pid
	local qemu_status
	local qemu_gid
	local qemu_uid

	kubectl apply -f "${pod_config}"
	kubectl wait --for=condition=Ready --timeout="${QEMU_SANDBOX_POD_TIMEOUT}" "pod/${pod_name}"

	qemu_pid="$(get_qemu_pid_for_pod "${pod_name}")"

	qemu_status="$(exec_host "${node}" "cat /proc/${qemu_pid}/status")"
	qemu_uid="$(awk '/^Uid:/ {print $2}' <<< "${qemu_status}")"
	qemu_gid="$(awk '/^Gid:/ {print $2}' <<< "${qemu_status}")"

	[[ "${qemu_uid}" =~ ^[0-9]+$ ]]
	[[ "${qemu_gid}" =~ ^[0-9]+$ ]]
	(( qemu_uid != 0 ))
	(( qemu_gid != 0 ))
	[[ "$(awk '/^Seccomp:/ {print $2}' <<< "${qemu_status}")" == "2" ]]
	[[ "$(awk '/^NoNewPrivs:/ {print $2}' <<< "${qemu_status}")" == "1" ]]

	cmdline="$(exec_host "${node}" "tr '\\0' ' ' < /proc/${qemu_pid}/cmdline")"
	[[ " ${cmdline} " == *" -sandbox ${QEMU_SANDBOX_PARAM} "* ]]
}

teardown() {
	qemu_rootless_sandbox_supported || return 0

	echo "=== QEMU rootless sandbox pod describe ==="
	kubectl describe pod "${pod_name:-test-e2e}" || true

	remove_kata_runtime_config_dropin_file \
		"${node}" \
		"${runtime_config_dropin:-}" || true

	delete_tmp_policy_settings_dir "${policy_settings_dir:-}"

	[ -f "${pod_config:-}" ] && kubectl delete -f "${pod_config}" --ignore-not-found=true

	print_node_journal_since_test_start \
		"${node}" \
		"${node_start_time:-}" \
		"${BATS_TEST_COMPLETED:-}"
}
