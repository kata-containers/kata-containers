#!/usr/bin/env bats
# Copyright (c) 2026 CoreWeave Inc.
#
# SPDX-License-Identifier: Apache-2.0

# Variables are supplied by Bats and the loaded test helpers.
# shellcheck disable=SC2154

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[[ "${KATA_HYPERVISOR}" == "rs-fc" ]] || skip "requires Rust Firecracker"
	setup_common || die "setup_common failed"
	pod_name="besteffort-test"
	yaml_file="${pod_config_dir}/pod-besteffort.yaml"
	set_node "${yaml_file}" "${node}"
	auto_generate_policy "${pod_config_dir}" "${yaml_file}"
}

@test "Firecracker pod uses the Rust shim and shuts down its VMM" {
	kubectl create -f "${yaml_file}"
	kubectl wait --for=condition=Ready --timeout="${timeout}" pod "${pod_name}"

	local pod_uid shim_pid vmm_pid shim_binary
	pod_uid="$(kubectl get pod "${pod_name}" -o jsonpath='{.metadata.uid}')"
	shim_pid="$(shim_pid_for_pod "${node}" "${pod_uid}")"
	[[ "${shim_pid}" =~ ^[0-9]+$ ]]
	shim_binary="$(exec_host "${node}" "readlink -f /proc/${shim_pid}/exe")"
	[[ "${shim_binary}" == "/opt/kata/runtime-rs/bin/containerd-shim-kata-v2" ]]
	vmm_pid="$(exec_host "${node}" "pgrep -P ${shim_pid} -x firecracker")"
	[[ "${vmm_pid}" =~ ^[0-9]+$ ]]

	kubectl delete pod "${pod_name}" --wait=true --timeout="${timeout}"
	# Pod deletion can precede the shim's asynchronous cleanup.
	waitForProcess "${wait_time}" "${sleep_time}" \
		"exec_host '${node}' 'test ! -e /proc/${vmm_pid} && test ! -e /proc/${shim_pid}'"
}

teardown() {
	[[ "${KATA_HYPERVISOR}" == "rs-fc" ]] || return 0
	kubectl describe pod "${pod_name}" || true
	kubectl delete pod "${pod_name}" --ignore-not-found=true --wait=true --timeout="${timeout}"
	teardown_common "${node}" "${node_start_time:-}"
}
