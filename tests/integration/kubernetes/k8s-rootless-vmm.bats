#!/usr/bin/env bats
#
# Copyright (c) NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

readonly QEMU_SANDBOX_PARAM="on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny"
readonly ROOTLESS_VMM_CLEANUP_TIMEOUT_SECONDS="${ROOTLESS_VMM_CLEANUP_TIMEOUT_SECONDS:-30}"

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

	for ((attempt = 0; attempt < ROOTLESS_VMM_CLEANUP_TIMEOUT_SECONDS; attempt++)); do
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
nvidia_gpu_request_supported() {
	[[ "${KATA_HYPERVISOR}" == qemu* ]] || return 1
	is_runtime_rs || return 1
	is_nvidia_gpu_platform || return 1
}

request_gpu_for_nvidia_gpu_runtime_rs() {
	local available_gpus
	local config="$1"

	available_gpus="$(kubectl get node "${node}" \
		-o jsonpath='{.status.allocatable.nvidia\.com/pgpu}')"
	[[ "${available_gpus}" =~ ^[1-9][0-9]*$ ]] || \
		die "${node} has no allocatable nvidia.com/pgpu resource"

	yq -i '.spec.containers[0].resources.limits."nvidia.com/pgpu" = "1"' \
		"${config}"
}

# Print why the current runtime cannot run this rootless VMM smoke test. A
# non-zero return means the runtime is supported.
rootless_vmm_skip_reason() {
	case "${KATA_HYPERVISOR}" in
		qemu*) qemu_rootless_skip_reason ;;
		clh*) clh_rootless_skip_reason ;;
		*)
			echo "rootless VMM coverage supports only QEMU and Cloud Hypervisor"
			return 0
			;;
	esac
}

qemu_rootless_skip_reason() {
	# Runtime-go does not pass EROFS layers to rootless QEMU by file
	# descriptor and therefore cannot traverse their snapshot directories.
	if [[ "${SNAPSHOTTER:-}" == "erofs" ]] && ! is_runtime_rs; then
		echo "rootless runtime-go QEMU does not support EROFS snapshot paths"
		return 0
	fi

	# CoCo-dev does not enable a TEE and remains covered. Runtime-rs now
	# validates access required by real confidential handlers, but CI hosts do
	# not yet have deployment-provisioned /dev/sev and QGS permissions. Keep
	# those handlers excluded until deployment provides that access contract.
	if is_confidential_runtime_class "${KATA_HYPERVISOR}" &&
		[[ "${KATA_HYPERVISOR}" != qemu-coco-dev* ]]; then
		echo "rootless QEMU confidential host-resource access is not enabled yet"
		return 0
	fi

	# Rootless policy testing is supported only by shared_fs=none runtime-rs
	# handlers. Generated policy does not authorize the rootless host paths
	# used with filesystem sharing. With shared_fs=none, policy adds an
	# init-data disk that only runtime-rs passes to QEMU by file descriptor.
	if auto_generate_policy_enabled; then
		if ! is_shared_fs_none_runtime_class "${KATA_HYPERVISOR}"; then
			echo "rootless QEMU with generated policy and filesystem sharing is not supported"
			return 0
		fi
		if ! is_runtime_rs; then
			echo "rootless QEMU with generated policy requires runtime-rs block-source FD transport"
			return 0
		fi
	fi

	return 1
}

clh_rootless_skip_reason() {
	local mshv_present

	# Cloud Hypervisor prefers Microsoft Hypervisor whenever /dev/mshv is
	# present. Managed AKS nodes currently expose that platform-owned device as
	# root:root 0600, which cannot be authorized by adding its owning group to
	# the temporary VMM user. Kata does not reconfigure the host-owned device;
	# enable this coverage once the platform provides scoped non-root access.
	mshv_present="$(exec_host "${node}" \
		'[[ ! -e /dev/mshv ]] || echo present')"
	if [[ "${mshv_present}" == "present" ]]; then
		echo "rootless Cloud Hypervisor with /dev/mshv requires platform-provisioned non-root access"
		return 0
	fi

	if is_shared_fs_none_runtime_class "${KATA_HYPERVISOR}"; then
		echo "rootless Cloud Hypervisor with shared_fs=none is not enabled yet"
		return 0
	fi

	# An unprivileged Cloud Hypervisor cannot traverse the restrictive EROFS
	# snapshot directories without a scoped-access mechanism.
	if [[ "${SNAPSHOTTER:-}" == "erofs" ]]; then
		echo "rootless Cloud Hypervisor does not support EROFS snapshot paths"
		return 0
	fi

	# Confidential Cloud Hypervisor handlers require additional host resources.
	if is_confidential_runtime_class "${KATA_HYPERVISOR}"; then
		echo "rootless Cloud Hypervisor confidential host-resource access is not enabled yet"
		return 0
	fi

	# Generated CoCo policies add an init-data disk below a root-owned host path,
	# which requires scoped VMM access. Eligible Cloud Hypervisor coverage also
	# uses filesystem sharing, whose rootless guest paths generated policy does
	# not authorize.
	if auto_generate_policy_enabled; then
		echo "rootless Cloud Hypervisor with generated policy is not enabled yet"
		return 0
	fi

	return 1
}

vmm_config_section() {
	case "${KATA_HYPERVISOR}" in
		qemu*) echo "qemu" ;;
		clh*) echo "clh" ;;
	esac
}

append_vmm_seccomp_config() {
	local config_file="$1"
	local seccomp_key

	case "${KATA_HYPERVISOR}" in
		qemu*)
			if is_runtime_rs; then
				seccomp_key="seccomp_sandbox"
			else
				seccomp_key="seccompsandbox"
			fi
			echo "${seccomp_key} = \"${QEMU_SANDBOX_PARAM}\"" >> "${config_file}"
			;;
		clh*)
			# Cloud Hypervisor enables its built-in seccomp filters by default,
			# so no opt-in configuration is required here.
			:
			;;
	esac
}

setup() {
	local runtime_config_dropin_file
	local skip_reason

	setup_common || die "setup_common failed"

	if skip_reason="$(rootless_vmm_skip_reason)"; then
		skip "${skip_reason} (KATA_HYPERVISOR=${KATA_HYPERVISOR})"
	fi

	pod_name="test-e2e"
	pod_config="$(new_pod_config \
		"quay.io/prometheus/busybox:latest" \
		"$(get_test_runtime_class)" \
		"" "" "10")"
	set_node "${pod_config}" "${node}"

	# Adding emptyDir validates that a rootless VMM can access additional
	# writable pod storage:
	# - with filesystem sharing, the VMM does not open a host block image;
	# - runtime-rs QEMU with shared_fs=none passes the image by file descriptor;
	# - runtime-go QEMU with shared_fs=none omits the disk because that transport
	#   is not implemented, while Cloud Hypervisor shared_fs=none is excluded.
	# Other block images passed by file descriptor to runtime-rs QEMU are:
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
	if nvidia_gpu_request_supported; then
		request_gpu_for_nvidia_gpu_runtime_rs "${pod_config}"
	fi

	watchable_pod_config="${BATS_FILE_TMPDIR}/inotify-configmap-pod.yaml"
	cp "${pod_config_dir}/inotify-configmap-pod.yaml" "${watchable_pod_config}"
	yq -i ".spec.runtimeClassName = \"$(get_test_runtime_class)\"" "${watchable_pod_config}"
	set_node "${watchable_pod_config}" "${node}"
	if nvidia_gpu_request_supported; then
		request_gpu_for_nvidia_gpu_runtime_rs "${watchable_pod_config}"
	fi

	auto_generate_policy "${pod_config_dir}" "${pod_config}"
	auto_generate_policy "${pod_config_dir}" "${watchable_pod_config}"

	runtime_config_dropin_file="${BATS_FILE_TMPDIR}/99-k8s-rootless-vmm.toml"
	cat > "${runtime_config_dropin_file}" <<EOF
[hypervisor.$(vmm_config_section)]
rootless = true
EOF
	append_vmm_seccomp_config "${runtime_config_dropin_file}"

	runtime_config_dropin="$(set_kata_runtime_config_dropin_file \
		"${node}" \
		"${runtime_config_dropin_file}")" || \
		die "Failed to install rootless VMM config drop-in on ${node}"
}

@test "VMM runs rootless with seccomp enabled" {
	local cmdline
	local host_resources
	local seccomp_status
	local vmm_gid
	local vmm_pid
	local vmm_status
	local vmm_uid
	host_resources="$(get_rootless_host_resources)"

	retry_kubectl_apply "${pod_config}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" "pod/${pod_name}"

	vmm_pid="$(get_vmm_pid_for_pod "${pod_name}")"

	vmm_status="$(exec_host "${node}" "cat /proc/${vmm_pid}/status")"
	vmm_uid="$(awk '/^Uid:/ {print $2}' <<< "${vmm_status}")"
	vmm_gid="$(awk '/^Gid:/ {print $2}' <<< "${vmm_status}")"
	seccomp_status="${vmm_status}"

	[[ "${vmm_uid}" =~ ^[0-9]+$ ]]
	[[ "${vmm_gid}" =~ ^[0-9]+$ ]]
	(( vmm_uid != 0 ))
	(( vmm_gid != 0 ))

	if [[ "${KATA_HYPERVISOR}" == qemu* ]]; then
		cmdline="$(exec_host "${node}" "tr '\\0' ' ' < /proc/${vmm_pid}/cmdline")"
		[[ " ${cmdline} " == *" -sandbox ${QEMU_SANDBOX_PARAM} "* ]]
	else
		# Cloud Hypervisor applies dedicated filters to its worker threads.
		# Inspect the VMM thread rather than the unfiltered process leader.
		seccomp_status="$(exec_host "${node}" \
			"for status in /proc/${vmm_pid}/task/*/status; do \
			grep -qE '^Name:[[:space:]]+vmm$' \"\${status}\" && \
			cat \"\${status}\" && break; done")"
	fi

	[[ "$(awk '/^Seccomp:/ {print $2}' <<< "${seccomp_status}")" == "2" ]]
	[[ "$(awk '/^NoNewPrivs:/ {print $2}' <<< "${seccomp_status}")" == "1" ]]

	kubectl delete -f "${pod_config}" --ignore-not-found=true
	wait_for_rootless_host_resources "${host_resources}"
}

@test "VMM propagates ConfigMap updates while rootless" {
	local arch
	local pod_termination_wait_time=180

	# k8s-inotify.bats is excluded on these architectures. Apply the same
	# scope only here because the per-architecture filters skip whole files,
	# and the rootless VMM launch test above is supported.
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
	rootless_vmm_skip_reason >/dev/null && return 0

	echo "=== Rootless VMM pod describe ==="
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
