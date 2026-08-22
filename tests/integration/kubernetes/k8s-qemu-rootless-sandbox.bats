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

qemu_rootless_sandbox_supported() {
	[[ "${KATA_HYPERVISOR}" == qemu* ]] || return 1

	# Runtime-go does not pass EROFS layers to rootless QEMU by file
	# descriptor and therefore cannot traverse their snapshot directories.
	if [[ "${SNAPSHOTTER:-}" == "erofs" ]] && ! is_runtime_rs; then
		return 1
	fi

	# CoCo-dev does not enable a TEE and remains covered. Runtime-rs now
	# validates access required by real confidential handlers, but CI hosts do
	# not yet have deployment-provisioned /dev/sev and QGS permissions. Keep
	# those handlers excluded until deployment provides that access contract.
	if is_confidential_runtime_class "${KATA_HYPERVISOR}" &&
		[[ "${KATA_HYPERVISOR}" != qemu-coco-dev* ]]; then
		return 1
	fi

	# Rootless policy testing is supported only by shared_fs=none runtime-rs
	# handlers. Generated policy does not authorize the rootless host paths
	# used with filesystem sharing. With shared_fs=none, policy adds an
	# init-data disk that only runtime-rs passes to QEMU by file descriptor.
	if auto_generate_policy_enabled; then
		is_shared_fs_none_runtime_class "${KATA_HYPERVISOR}" || return 1
		is_runtime_rs || return 1
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

	# Adding emptyDir validates that rootless QEMU can access additional writable pod
	# storage:
	# - any runtime class with filesystem sharing can exercise emptyDir because QEMU
	#   does not open a host block image.
	# - Runtime-rs with shared_fs=none exercises this path by passing the image to
	#   QEMU by file descriptor.
	# - Runtime-go with shared_fs=none is skipped because the fd transport method is
	#   not implemented.
	# Other uses of block images for which file descriptors are passed by runtime-rs
	# are:
	# - raw /dev/loop* volumes (see k8s-block-volume.bats), not exercised in this
	# bats test to reduce complexity.
	# - init-data images, implicitly exercised by jobs where policy annotations are
	# attached.
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

	watchable_pod_config="${BATS_FILE_TMPDIR}/inotify-configmap-pod.yaml"
	cp "${pod_config_dir}/inotify-configmap-pod.yaml" "${watchable_pod_config}"
	yq -i ".spec.runtimeClassName = \"$(get_test_runtime_class)\"" "${watchable_pod_config}"
	set_node "${watchable_pod_config}" "${node}"

	auto_generate_policy "${pod_config_dir}" "${pod_config}"
	auto_generate_policy "${pod_config_dir}" "${watchable_pod_config}"

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

@test "QEMU propagates ConfigMap updates while rootless" {
	local arch
	local pod_termination_wait_time=180

	# k8s-inotify.bats is excluded on these architectures. Apply the same
	# scope only here because the per-architecture filters skip whole files,
	# and the rootless QEMU launch test above is supported.
	arch="$(uname -m)"
	case "${arch}" in
		aarch64|ppc64le|s390x)
			skip "ConfigMap inotify testing is not enabled on ${arch}"
			;;
	esac

	pod_name="inotify-configmap-testing"
	pod_config="${watchable_pod_config}"

	# Fail if another test already owns this ConfigMap. Use retried applies
	# below only for the test-owned Pod and the intentional ConfigMap update.
	kubectl create -f "${pod_config_dir}/inotify-configmap.yaml"
	retry_kubectl_apply "${pod_config}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" "pod/${pod_name}"

	retry_kubectl_apply "${pod_config_dir}/inotify-updated-configmap.yaml"

	command="kubectl describe pod ${pod_name} | grep \"State: \+Terminated\""
	info "Waiting ${pod_termination_wait_time} seconds for: ${command}"
	waitForProcess "${pod_termination_wait_time}" "${sleep_time}" "${command}"

	result=$(kubectl get pod "${pod_name}" \
		--output="jsonpath={.status.containerStatuses[]}")
	echo "${result}" | grep -vq Error
}

teardown() {
	qemu_rootless_sandbox_supported || return 0

	echo "=== QEMU rootless sandbox pod describe ==="
	kubectl describe pod "${pod_name:-test-e2e}" || true

	remove_kata_runtime_config_dropin_file \
		"${node}" \
		"${runtime_config_dropin:-}" || true

	[ -f "${pod_config:-}" ] && kubectl delete -f "${pod_config}" --ignore-not-found=true
	kubectl delete configmap cm --ignore-not-found=true

	print_node_journal_since_test_start \
		"${node}" \
		"${node_start_time:-}" \
		"${BATS_TEST_COMPLETED:-}"
}
