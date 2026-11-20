#!/usr/bin/env bats
#
# Copyright (c) NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# When a shim dies, the container runtime runs the shim's delete command in a
# new process, which only knows the sandbox through its state.json. Kill the
# shim of a running pod and check that this cleanup leaves nothing of the
# sandbox behind, including the temporary user and runtime directory for a
# rootless VMM.
#
# When the VMM dies instead, the shim is still there and the teardown is its
# own: kill the VMM of a running pod and check the same once the pod is gone.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	is_runtime_rs || skip "cleanup after a killed shim or VMM is tested for runtime-rs only (KATA_HYPERVISOR=${KATA_HYPERVISOR})"

	setup_common || die "setup_common failed"

	pod_name="shim-kill-cleanup"
	pod_config="$(new_pod_config \
		"quay.io/prometheus/busybox:latest" \
		"$(get_test_runtime_class)" \
		"" "" "10")"
	yq -i ".metadata.name = \"${pod_name}\"" "${pod_config}"
	# With shared_fs=none an emptyDir is a disk runtime-rs creates on the host,
	# which recovery must remove too.
	yq -i '
		.spec.volumes += [{"name": "scratch", "emptyDir": {}}] |
		.spec.containers[0].volumeMounts += [{
			"name": "scratch",
			"mountPath": "/mnt/scratch"
		}]
	' "${pod_config}"
	set_container_command "${pod_config}" 0 sleep infinity
	# Without this, kubelet replaces the killed sandbox for the same pod,
	# and the replacement re-registers resources at the same per-pod paths
	# the test is waiting to see disappear.
	yq -i '.spec.restartPolicy = "Never"' "${pod_config}"
	set_node "${pod_config}" "${node}"
	# With a GPU the sandbox holds a passed-through device, which a VMM left
	# behind keeps from every sandbox after it.
	if nvidia_gpu_request_supported; then
		request_gpu_for_nvidia_gpu_runtime_rs "${pod_config}"
	fi

	auto_generate_policy "${pod_config_dir}" "${pod_config}"
}

# A sandbox started after the cleanup has to come up. The leftovers the tests
# wait for are processes and paths; what else a dead sandbox can keep, a
# passed-through device among it, shows when the next one needs it.
start_pod_again() {
	kubectl delete -f "${pod_config}" --ignore-not-found=true --wait=true --timeout="${timeout}"
	retry_kubectl_apply "${pod_config}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" "pod/${pod_name}"
}

# Record rootless resources while the VMM is still running. Rootful VMMs have
# no temporary account to check. PID lookup supports QEMU and Cloud Hypervisor.
record_rootless_vmm() {
	rootless_vmm_uid=""
	rootless_vmm_user=""
	case "${KATA_HYPERVISOR}" in
		qemu*|clh*) ;;
		*) return 0 ;;
	esac
	local vmm_pid
	vmm_pid="$(get_vmm_pid_for_pod "${pod_name}")"
	[[ "${vmm_pid}" =~ ^[0-9]+$ ]] || die "No VMM found for pod ${pod_name}"
	rootless_vmm_uid="$(exec_host "${node}" "awk '/^Uid:/ {print \$2}' /proc/${vmm_pid}/status")"
	[[ "${rootless_vmm_uid}" =~ ^[0-9]+$ ]] || die "Could not read VMM UID"
	(( rootless_vmm_uid != 0 )) || return 0
	rootless_vmm_user="$(exec_host "${node}" "getent passwd ${rootless_vmm_uid} | cut -d: -f1")"
	[[ "${rootless_vmm_user}" =~ ^kata-[a-f0-9]{8}-[0-9]+$ ]] || \
		die "Unexpected rootless VMM user: ${rootless_vmm_user}"
}

wait_for_recorded_rootless_vmm() {
	[[ -n "${rootless_vmm_user}" ]] || return 0
	wait_for_rootless_host_resources "${rootless_vmm_uid}" "${rootless_vmm_user}"
}

@test "Nothing of a sandbox is left once its shim was killed" {
	local sandbox_id

	[ "${CONTAINER_RUNTIME}" == "crio" ] && skip "CRI-O does not run the shim's delete command after a shim died"

	retry_kubectl_apply "${pod_config}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" "pod/${pod_name}"

	sandbox_id="$(get_pod_sandbox_id "${pod_name}")"
	[[ -n "${sandbox_id}" ]] || die "No sandbox ID found for pod ${pod_name}"

	record_rootless_vmm
	kill_shim_and_wait_for_cleanup "${node}" "${sandbox_id}"
	wait_for_recorded_rootless_vmm
	start_pod_again
}

@test "Nothing of a sandbox is left once its VMM was killed" {
	local sandbox_id

	case "${KATA_HYPERVISOR}" in
		qemu*|clh*) ;;
		*) skip "the VMM is only looked up for QEMU and Cloud Hypervisor (KATA_HYPERVISOR=${KATA_HYPERVISOR})" ;;
	esac

	retry_kubectl_apply "${pod_config}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" "pod/${pod_name}"

	sandbox_id="$(get_pod_sandbox_id "${pod_name}")"
	[[ -n "${sandbox_id}" ]] || die "No sandbox ID found for pod ${pod_name}"

	record_rootless_vmm
	kill_vmm_and_wait_for_cleanup "${node}" "${sandbox_id}" "${pod_name}"
	wait_for_recorded_rootless_vmm
	start_pod_again
}

teardown() {
	is_runtime_rs || skip "cleanup after a killed shim or VMM is tested for runtime-rs only (KATA_HYPERVISOR=${KATA_HYPERVISOR})"

	[ -f "${pod_config:-}" ] && kubectl delete -f "${pod_config}" --ignore-not-found=true

	teardown_common "${node}" "${node_start_time:-}"
}
