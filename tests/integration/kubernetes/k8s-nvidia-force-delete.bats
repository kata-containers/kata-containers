#!/usr/bin/env bats
#
# Copyright (c) NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# A pod that is deleted with --force is gone from Kubernetes at once, and its
# GPUs are handed to the next pod. When its VM was still booting, nobody stops
# that VM until the boot is over, so for a while two VMs want the same GPUs.
# The second one has to wait for them, not fail.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

# A VMM binds its GPUs within its first second, and a guest with a GPU takes
# much longer than this to boot.
vmm_settle_seconds=3

skip_unless_supported() {
	nvidia_gpu_request_supported || \
		skip "a GPU held by a force-deleted pod is tested for runtime-rs with QEMU only (KATA_HYPERVISOR=${KATA_HYPERVISOR})"
}

gpu_pod_config() {
	local name="$1"
	local config

	config="$(new_pod_config "quay.io/prometheus/busybox:latest" "$(get_test_runtime_class)" "" "" "10")" || return 1
	yq -i ".metadata.name = \"${name}\"" "${config}" || return 1
	# The GPUs are counted and the VMM is looked for on this node only.
	set_node "${config}" "${node}" || return 1
	yq -i '.spec.restartPolicy = "Never"' "${config}" || return 1
	yq -i ".spec.containers[0].resources.limits.\"nvidia.com/gpu\" = \"${gpus}\"" "${config}" || return 1
	set_container_command "${config}" 0 sleep infinity >&2 || return 1
	auto_generate_policy "${pod_config_dir}" "${config}" >&2 || return 1

	echo "${config}"
}

# A pod whose VM is still booting has no sandbox ID to look its VMM up by.
wait_for_booting_vmm() {
	local cmd="pid=\$(pgrep -n -f '[q]emu.*sandbox-') && [ \"\$(ps -o etimes= -p \"\${pid}\")\" -ge ${vmm_settle_seconds} ] && echo \"\${pid}\""
	local pid

	for _ in $(seq 1 60); do
		pid="$(exec_host "${node}" "${cmd}" || true)"
		if [[ "${pid}" =~ ^[0-9]+$ ]]; then
			echo "${pid}"
			return 0
		fi
		sleep 1
	done

	return 1
}

# The old VM being alive when the new pod is created does not say that it
# still held the GPUs when the new VMM asked for them. Only the shim knows.
# Another sandbox on the node may have waited too, so only the entries of this
# pod's sandbox count.
waited_for_gpus() {
	local sandbox_id

	sandbox_id="$(get_pod_sandbox_id "$1")"
	[[ -n "${sandbox_id}" ]] || return 1
	exec_host "${node}" "journalctl -t kata --since \"${node_start_time}\" -o json | grep 'waiting for .* to be released' | grep -q '${sandbox_id}'"
}

# Events outlive the pod they are about, and a pod of the same name may have
# run before.
failed_sandbox_events() {
	local uid

	uid="$(kubectl get pod "$1" -o jsonpath='{.metadata.uid}')"
	kubectl get events \
		--field-selector "involvedObject.uid=${uid},reason=FailedCreatePodSandBox" \
		-o jsonpath='{range .items[*]}{.message}{"\n"}{end}'
}

setup() {
	skip_unless_supported

	deleted_pod="nvidia-gpu-force-deleted"
	next_pod="nvidia-gpu-after-force-delete"

	setup_common || die "setup_common failed"

	gpus="$(kubectl get node "${node}" -o jsonpath='{.status.allocatable.nvidia\.com/gpu}')"
	[[ "${gpus}" =~ ^[1-9][0-9]*$ ]] || die "${node} has no allocatable nvidia.com/gpu resource"
	# With fewer than every GPU of the node, the second pod could be given
	# GPUs the first one never had.
	if [[ "${SKIP_MULTI_GPU_TESTS:-false}" == "true" && "${gpus}" -gt 1 ]]; then
		skip "every GPU of ${node} has to go to one pod, and SKIP_MULTI_GPU_TESTS forbids that"
	fi

	deleted_pod_config="$(gpu_pod_config "${deleted_pod}")" || die "Could not write the configuration of ${deleted_pod}"
	next_pod_config="$(gpu_pod_config "${next_pod}")" || die "Could not write the configuration of ${next_pod}"
}

@test "A pod gets the GPUs of a pod that was force-deleted while its VM was booting" {
	local old_vmm
	local failed

	retry_kubectl_apply "${deleted_pod_config}"
	old_vmm="$(wait_for_booting_vmm)" || die "The VMM of ${deleted_pod} did not start"

	kubectl delete pod "${deleted_pod}" --force --grace-period=0 --wait=false
	retry_kubectl_apply "${next_pod_config}"

	exec_host "${node}" "kill -0 ${old_vmm}" || \
		skip "the VM of ${deleted_pod} was gone before ${next_pod} asked for its GPUs"

	# The old VM finishes booting before it is stopped, and with many GPUs
	# that takes minutes.
	kubectl wait --for=condition=Ready --timeout=900s "pod/${next_pod}"

	failed="$(failed_sandbox_events "${next_pod}")"
	echo "# Failed attempts to create the sandbox of ${next_pod}: ${failed:-none}"
	! grep -q -E 'error bind device|timed out waiting for QMP ready' <<< "${failed}" || \
		die "A VMM was started for ${next_pod} while its GPUs were still held"

	# Nothing is tested unless the new pod had to wait. A sandbox that gave
	# up waiting says so in its event, and the one that replaced it may have
	# found the GPUs free.
	grep -q 'still held by another process' <<< "${failed}" || waited_for_gpus "${next_pod}" || \
		skip "the GPUs of ${deleted_pod} were free before the VMM of ${next_pod} asked for them"
}

teardown() {
	skip_unless_supported

	kubectl describe pod "${next_pod}" || true
	kubectl delete pod "${deleted_pod}" "${next_pod}" --force --grace-period=0 --ignore-not-found=true

	teardown_common "${node}" "${node_start_time:-}"
}
