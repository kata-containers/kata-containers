#!/usr/bin/env bats
#
# Copyright (c) 2026 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Under sandbox_cgroup_only on a cgroup v2 node, the runtime must place the
# shim, the VMM and virtiofsd in the same pod (kubepods) cgroup. The
# regression this guards leaves the VMM and virtiofsd in
# system.slice/containerd.service while only the shim reaches kubepods. Runs
# on any hypervisor with a separate VMM process; all checks use exec_host.

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

host_is_cgroup_v2() {
	exec_host "${node}" "test -f /sys/fs/cgroup/cgroup.controllers" >/dev/null 2>&1
}

sandbox_cgroup_only_enabled() {
	local cfg
	cfg="$(get_kata_runtime_config_file "${node}")" || return 1
	exec_host "${node}" "grep -qE '^\s*sandbox_cgroup_only\s*=\s*true' '${cfg}'"
}

# Echo the cgroup v2 path (the part after "0::") for a pid on the node.
# Returns non-zero if the path cannot be determined.
host_cgroup_v2_path() {
	local pid="${1}" path
	# On cgroup v2 the file is a single line "0::<path>", e.g.
	#   0::/kubepods.slice/.../cri-containerd-<cid>.scope
	# Split on "::" and print the path that follows the "0::" prefix.
	path="$(exec_host "${node}" "cat /proc/${pid}/cgroup" | awk -F'::' '/^0::/ {print $2}')"
	[[ -n "${path}" ]] || return 1
	echo "${path}"
}

setup() {
	node="$(get_one_kata_node)"
	has_separate_vmm || skip "test requires a hypervisor with a separate VMM process (KATA_HYPERVISOR=${KATA_HYPERVISOR})"
	[[ -n "${node:-}" ]] || skip "no kata-runtime node found"
	host_is_cgroup_v2 || skip "test requires the cgroup v2 unified hierarchy on the node"
	sandbox_cgroup_only_enabled || skip "test requires sandbox_cgroup_only=true"

	setup_common || die "setup_common failed"

	pod_name="besteffort-test"

	yaml_file="${pod_config_dir}/pod-besteffort.yaml"
	set_node "$yaml_file" "$node"

	auto_generate_policy "${pod_config_dir}" "${yaml_file}"
}

@test "runtime, virtiofsd and VMM share the pod sandbox cgroup" {
	kubectl create -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="$timeout" pod "$pod_name"

	# Select the shim by pod's UID
	local pod_uid
	pod_uid="$(kubectl get pod "${pod_name}" -o jsonpath='{.metadata.uid}')"

	local shim_pid vmm_pid vfsd_pid
	shim_pid="$(shim_pid_for_pod "${node}" "${pod_uid}")"

	vmm_pid="$(exec_host "${node}" "pgrep -P ${shim_pid} -f 'cloud-hypervisor|qemu-system|firecracker|stratovirt'")"

	vfsd_pid="$(exec_host "${node}" "pgrep -P ${shim_pid} virtiofsd || true")"

	info "pod uid: ${pod_uid}, shim pid: ${shim_pid}, VMM pid: ${vmm_pid}, virtiofsd pid: ${vfsd_pid:-<none>}"

	local shim_cgroup
	shim_cgroup="$(host_cgroup_v2_path "${shim_pid}")"
	info "shim cgroup: ${shim_cgroup}"
	echo "${shim_cgroup}" | grep -q "kubepods" || die "shim is not in a kubepods cgroup: ${shim_cgroup}"

	# The VMM must be co-located with the shim, not left in the spawn cgroup.
	local vmm_cgroup
	vmm_cgroup="$(host_cgroup_v2_path "${vmm_pid}")"
	info "VMM cgroup: ${vmm_cgroup}"
	[[ "${vmm_cgroup}" == "${shim_cgroup}" ]] || die "VMM cgroup (${vmm_cgroup}) differs from the shim/pod cgroup (${shim_cgroup}); guest RAM would be mischarged"

	# virtiofsd (when a separate daemon is used) must be co-located too.
	if [[ -n "${vfsd_pid}" ]]; then
		local vfsd_cgroup
		vfsd_cgroup="$(host_cgroup_v2_path "${vfsd_pid}")"
		info "virtiofsd cgroup: ${vfsd_cgroup}"
		[[ "${vfsd_cgroup}" == "${shim_cgroup}" ]] || die "virtiofsd cgroup (${vfsd_cgroup}) differs from the shim/pod cgroup (${shim_cgroup})"
	fi
}

teardown() {
	has_separate_vmm || skip "test requires a hypervisor with a separate VMM process (KATA_HYPERVISOR=${KATA_HYPERVISOR})"
	[[ -n "${node:-}" ]] || skip "no kata-runtime node found"
	host_is_cgroup_v2 || skip "test requires the cgroup v2 unified hierarchy on the node"
	sandbox_cgroup_only_enabled || skip "test requires sandbox_cgroup_only=true"

	teardown_common "${node}" "${node_start_time:-}"
}
