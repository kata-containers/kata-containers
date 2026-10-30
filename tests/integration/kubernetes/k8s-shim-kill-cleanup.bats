#!/usr/bin/env bats
#
# Copyright (c) NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# When a shim dies, the container runtime runs the shim's delete command in a
# new process, which only knows the sandbox through its state.json. Kill the
# shim of a running pod and check that this cleanup leaves nothing of the
# sandbox behind. k8s-rootless-vmm.bats does the same for a rootless VMM.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	is_runtime_rs || skip "cleanup after a killed shim is tested for runtime-rs only (KATA_HYPERVISOR=${KATA_HYPERVISOR})"
	[ "${CONTAINER_RUNTIME}" == "crio" ] && skip "CRI-O does not run the shim's delete command after a shim died"

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
	set_node "${pod_config}" "${node}"

	auto_generate_policy "${pod_config_dir}" "${pod_config}"
}

@test "Nothing of a sandbox is left once its shim was killed" {
	local sandbox_id

	retry_kubectl_apply "${pod_config}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" "pod/${pod_name}"

	sandbox_id="$(get_pod_sandbox_id "${pod_name}")"
	[[ -n "${sandbox_id}" ]] || die "No sandbox ID found for pod ${pod_name}"

	kill_shim_and_wait_for_cleanup "${node}" "${sandbox_id}"
}

teardown() {
	is_runtime_rs || skip "cleanup after a killed shim is tested for runtime-rs only (KATA_HYPERVISOR=${KATA_HYPERVISOR})"
	[ "${CONTAINER_RUNTIME}" == "crio" ] && skip "CRI-O does not run the shim's delete command after a shim died"

	[ -f "${pod_config:-}" ] && kubectl delete -f "${pod_config}" --ignore-not-found=true

	teardown_common "${node}" "${node_start_time:-}"
}
