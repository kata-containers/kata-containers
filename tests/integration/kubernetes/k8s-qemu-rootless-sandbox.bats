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
readonly QEMU_SANDBOX_CLEANUP_TIMEOUT_SECONDS="${QEMU_SANDBOX_CLEANUP_TIMEOUT_SECONDS:-30}"

get_rootless_host_resources() {
	local query="{ getent passwd | "
	query+="awk -F: '\$1 ~ /^kata-[0-9]+$/ "
	query+="&& \$5 ~ /Kata Containers temporary hypervisor user/ "
	query+="{ print \"user:\" \$1 \":\" \$3 }'; "
	query+="find /run/user -mindepth 1 -maxdepth 1 -type d "
	query+="-printf 'dir:%f\\n' 2>/dev/null; } | sort"

	exec_host "${node}" "${query}"
}

wait_for_rootless_host_resources() {
	local actual_resources
	local attempt
	local expected_resources="$1"

	for ((attempt = 0; attempt < QEMU_SANDBOX_CLEANUP_TIMEOUT_SECONDS; attempt++)); do
		actual_resources="$(get_rootless_host_resources)"
		[[ "${actual_resources}" == "${expected_resources}" ]] && return 0
		sleep 1
	done

	echo "Rootless host resources before the sandbox:"
	echo "${expected_resources}"
	echo "Rootless host resources after sandbox teardown:"
	echo "${actual_resources}"
	return 1
}

# Remove this helper once NVIDIA GPU runtime-rs configurations enable rootless
# by default. Until then, explicitly request a GPU so this test exercises the
# runtime-rs VFIO file-descriptor path.
request_gpu_for_nvidia_gpu_runtime_rs() {
	local available_gpus
	local config="$1"

	is_runtime_rs || return 0
	is_nvidia_gpu_platform || return 0

	available_gpus="$(kubectl get node "${node}" \
		-o jsonpath='{.status.allocatable.nvidia\.com/pgpu}')"
	[[ "${available_gpus}" =~ ^[1-9][0-9]*$ ]] || \
		die "${node} has no allocatable nvidia.com/pgpu resource"

	yq -i '.spec.containers[0].resources.limits."nvidia.com/pgpu" = "1"' \
		"${config}"
}

qemu_rootless_sandbox_supported() {
	[[ "${KATA_HYPERVISOR}" == qemu* ]] || return 1

	# CoCo-dev does not enable a TEE and remains covered for both runtimes.
	# Actual confidential handlers require runtime-rs rootless device access.
	if is_confidential_runtime_class "${KATA_HYPERVISOR}" &&
		[[ "${KATA_HYPERVISOR}" != qemu-coco-dev* ]] &&
		! is_runtime_rs; then
		return 1
	fi
	# Runtime-go does not pass EROFS layers to rootless QEMU by file
	# descriptor and therefore cannot traverse their snapshot directories.
	if [[ "${SNAPSHOTTER:-}" == "erofs" ]] && ! is_runtime_rs; then
		return 1
	fi
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
		"$(get_test_runtime_class)" \
		"" "" "10")"
	set_node "${pod_config}" "${node}"
	# /dev/loop* remains covered by k8s-block-volume.bats. When policy
	# generation is enabled, confidential runtime-rs handlers cover init-data.
	# Do not request a disk-backed emptyDir from runtime-go with shared_fs=none;
	# only runtime-rs implements the block-source file-descriptor path.
	if is_runtime_rs || ! is_shared_fs_none_runtime_class "${KATA_HYPERVISOR}"; then
		yq -i '
			.spec.volumes += [{"name": "rootless-emptydir", "emptyDir": {}}] |
			.spec.containers[0].volumeMounts += [{
				"name": "rootless-emptydir",
				"mountPath": "/mnt/rootless-emptydir"
			}]
		' "${pod_config}"
	fi
	set_container_command "${pod_config}" 0 sleep 30
	request_gpu_for_nvidia_gpu_runtime_rs "${pod_config}"

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
	local host_resources
	local qemu_pid
	local qemu_status
	local qemu_gid
	local qemu_uid
	host_resources="$(get_rootless_host_resources)"

	retry_kubectl_apply "${pod_config}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" "pod/${pod_name}"

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

	kubectl delete -f "${pod_config}" --ignore-not-found=true
	wait_for_rootless_host_resources "${host_resources}"
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
