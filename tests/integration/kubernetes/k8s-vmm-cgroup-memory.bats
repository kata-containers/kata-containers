#!/usr/bin/env bats
#
# Copyright (c) 2026 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Verify that the VMM is placed in the pod sandbox cgroup before the guest
# boots, so the guest RAM is charged to the pod under cgroup v2 (first-touch
# accounting) rather than to the cgroup the VMM was spawned in. Applies to
# any hypervisor that runs the guest in a separate VMM process (e.g.
# cloud-hypervisor, qemu) when sandbox_cgroup_only=true. Checks the VMM
# cgroup, the pod cgroup's memory.current and the kubelet working-set, all
# from the host via exec_host.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

# Lower bound (bytes) for guest RAM charged to the pod cgroup. 80 MiB
# separates the fixed runtime (~110-160 MiB) from the regression (~5-12 MiB).
MIN_GUEST_MEMORY_BYTES="${MIN_GUEST_MEMORY_BYTES:-83886080}"

# kubelet stats lag pod readiness, so poll the stats/summary API.
KUBELET_STATS_RETRIES="${KUBELET_STATS_RETRIES:-30}"
KUBELET_STATS_SLEEP="${KUBELET_STATS_SLEEP:-2}"

# True for hypervisors that run the guest in a separate VMM process.
# dragonball runs the guest in-process in the shim, so it has no separate
# VMM to place and is out of scope.
has_separate_vmm() {
	[[ "${KATA_HYPERVISOR}" != *dragonball* ]]
}

# True when the node uses the cgroup v2 unified hierarchy.
host_is_cgroup_v2() {
	exec_host "${node}" "test -f /sys/fs/cgroup/cgroup.controllers" >/dev/null 2>&1
}

# True unless the kata config explicitly sets sandbox_cgroup_only = false
# (the shipped configs default to true).
sandbox_cgroup_only_enabled() {
	local cfg val
	cfg="$(get_kata_runtime_config_file "${node}")" || return 0
	val="$(exec_host "${node}" "grep -E '^[[:space:]]*sandbox_cgroup_only[[:space:]]*=' '${cfg}' | tail -1" || true)"
	if echo "${val}" | grep -q 'false'; then
		return 1
	fi
	return 0
}

# Resolve the running VMM PID on the node. Only one Kata pod runs during the
# test (setup_common clears existing pods), so the VMM is the sole matching
# process, identified by its binary name across hypervisors.
get_vmm_pid() {
	exec_host "${node}" "pgrep -f 'cloud-hypervisor|qemu-system|firecracker|stratovirt' | head -1"
}

# Echo the pod's memory.workingSetBytes from the kubelet stats/summary API.
get_pod_workingset_bytes() {
	kubectl get --raw "/api/v1/nodes/${node}/proxy/stats/summary" 2>/dev/null \
		| jq -r --arg p "${pod_name}" \
			'.pods[] | select(.podRef.name==$p) | .memory.workingSetBytes // empty'
}

setup() {
	setup_common || die "setup_common failed"

	has_separate_vmm || skip "test requires a hypervisor with a separate VMM process (KATA_HYPERVISOR=${KATA_HYPERVISOR})"
	host_is_cgroup_v2 || skip "test requires the cgroup v2 unified hierarchy on the node"
	sandbox_cgroup_only_enabled || skip "test requires sandbox_cgroup_only=true"

	pod_name="besteffort-test"

	yaml_file="${pod_config_dir}/pod-besteffort.yaml"
	set_node "$yaml_file" "$node"

	# Add policy to yaml
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"
}

@test "VMM guest RAM is charged to the pod cgroup and visible to the kubelet" {
	kubectl create -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="$timeout" pod "$pod_name"

	local vmm_pid
	vmm_pid="$(get_vmm_pid)"
	[[ -n "${vmm_pid}" ]] || die "could not find the VMM process on ${node}"
	info "VMM pid: ${vmm_pid}"

	# 1. The VMM must be in the pod's kubepods cgroup, not under
	#    system.slice/containerd.service.
	local vmm_cgroup
	vmm_cgroup="$(exec_host "${node}" "cat /proc/${vmm_pid}/cgroup" | awk -F'::' '/^0::/ {print $2}')"
	[[ -n "${vmm_cgroup}" ]] || die "could not read cgroup v2 path for VMM pid ${vmm_pid}"
	info "VMM cgroup: ${vmm_cgroup}"

	echo "${vmm_cgroup}" | grep -q "kubepods" \
		|| die "VMM is not in a kubepods cgroup: ${vmm_cgroup}"
	if echo "${vmm_cgroup}" | grep -qE 'system\.slice|containerd\.service'; then
		die "VMM landed in the host cgroup instead of the pod cgroup: ${vmm_cgroup}"
	fi

	# 2. The pod cgroup's memory.current must account for the guest RAM.
	local mem_current
	mem_current="$(exec_host "${node}" "cat /sys/fs/cgroup${vmm_cgroup}/memory.current")"
	[[ "${mem_current}" =~ ^[0-9]+$ ]] || die "unexpected memory.current value: ${mem_current}"
	info "pod cgroup memory.current: ${mem_current} bytes (min expected ${MIN_GUEST_MEMORY_BYTES})"

	[[ "${mem_current}" -ge "${MIN_GUEST_MEMORY_BYTES}" ]] \
		|| die "pod cgroup memory.current (${mem_current}) below expected guest RAM (${MIN_GUEST_MEMORY_BYTES}); guest RAM is not charged to the pod cgroup"

	# 3. The kubelet working-set (backing kubectl top / metrics-server / HPA)
	#    is derived from the pod cgroup and must also reflect the guest RAM.
	local workingset=""
	local i
	for (( i = 0; i < KUBELET_STATS_RETRIES; i++ )); do
		workingset="$(get_pod_workingset_bytes)"
		if [[ "${workingset}" =~ ^[0-9]+$ ]] && [[ "${workingset}" -ge "${MIN_GUEST_MEMORY_BYTES}" ]]; then
			break
		fi
		sleep "${KUBELET_STATS_SLEEP}"
	done
	info "kubelet workingSetBytes: ${workingset:-<none>} (min expected ${MIN_GUEST_MEMORY_BYTES})"

	[[ "${workingset}" =~ ^[0-9]+$ ]] \
		|| die "kubelet reported no working-set for pod ${pod_name}"
	[[ "${workingset}" -ge "${MIN_GUEST_MEMORY_BYTES}" ]] \
		|| die "kubelet workingSetBytes (${workingset}) below expected guest RAM (${MIN_GUEST_MEMORY_BYTES}); the guest RAM is not visible to the kubelet"
}

teardown() {
	# Skipped tests leave these unset.
	if [[ -n "${pod_name:-}" ]]; then
		kubectl describe "pod/${pod_name}" || true
	fi
	if [[ -n "${policy_settings_dir:-}" ]]; then
		delete_tmp_policy_settings_dir "${policy_settings_dir}"
	fi
	teardown_common "${node}" "${node_start_time:-}"
}
