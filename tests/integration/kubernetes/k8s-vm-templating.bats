#!/usr/bin/env bats
#
# Copyright (c) 2024 Kata Containers
#
# SPDX-License-Identifier: Apache-2.0
#
# Tests for Kata VM templating (factory) functionality in Kubernetes integration mode

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

# Returns 0 if the current environment supports VM templating, non-zero
# otherwise. VM templating is only supported on non-confidential clh/qemu
# hypervisors, and because it uses shared_fs="none" it also requires a
# block-device-based snapshotter (blockfile or erofs).
vm_templating_supported() {
	[[ "${KATA_HYPERVISOR}" == "clh" || "${KATA_HYPERVISOR}" == "qemu" ]] || return 1
	is_confidential_runtime_class && return 1
	[[ "${SNAPSHOTTER:-}" =~ ^(blockfile|erofs)$ ]] || return 1
	return 0
}

setup() {
	if ! vm_templating_supported; then
		skip "VM templating requires a non-confidential clh/qemu hypervisor and a blockfile/erofs snapshotter (KATA_HYPERVISOR=${KATA_HYPERVISOR}, SNAPSHOTTER=${SNAPSHOTTER:-unset})"
	fi

	setup_common || die "setup_common failed"

	# Build a Kata runtime config drop-in that enables VM templating and
	# disables shared_fs (incompatible with templating).
	local runtime_config_dropin_file="${BATS_TEST_TMPDIR}/99-k8s-vm-templating.toml"
	cat > "${runtime_config_dropin_file}" <<DROPIN
[hypervisor.${KATA_HYPERVISOR}]
shared_fs = "none"
default_vcpus = 1
default_memory = 512

[factory]
enable_template = true
template_path = "/run/vc/vm/template"
DROPIN

	# Install the drop-in on the node selected by setup_common and record the
	# remote path so teardown can remove it.
	dropin_path="$(set_kata_runtime_config_dropin_file "$node" "${runtime_config_dropin_file}")" \
		|| die "Failed to install Kata runtime config drop-in on node $node"
}

@test "Pod can be created with a templated VM" {
	# Initialize the VM template on the target node. Use the absolute path:
	# kata-deploy installs kata-runtime under /opt/kata/bin, which is not on the
	# node's sudo PATH.
	exec_host "$node" "sudo /opt/kata/bin/kata-runtime factory init"

	# The factory init above must have created the template directory. exec_host
	# pipes the remote output through `tr`, so the pipeline's exit status is not
	# the remote command's; assert on the output instead.
	exec_host "$node" "test -d /run/vc/vm/template && echo present" | grep -q present

	pod_name="test-templated-pod"
	ctr_name="test-container"

	pod_config=$(mktemp --tmpdir pod_config.XXXXXX.yaml)
	cp "$pod_config_dir/busybox-template.yaml" "$pod_config"

	sed -i "s/POD_NAME/$pod_name/" "$pod_config"
	sed -i "s/CTR_NAME/$ctr_name/" "$pod_config"

	kubectl create -f "${pod_config}"
	kubectl wait --for=condition=Ready --timeout="$timeout" "pod/${pod_name}"

	grep_pod_exec_output "${pod_name}" "Hello from templated VM" sh -c "echo 'Hello from templated VM'"

	# Confirm the pod's VM was actually spawned from the factory/template
	# rather than booted normally. A normal VM stores its state directly under
	# /run/vc/vm/<sandbox-id>/ (a real directory), whereas a factory-spawned VM
	# is created under a generated UUID directory and /run/vc/vm/<sandbox-id> is
	# a symlink pointing at it (see assignSandbox() in
	# src/runtime/virtcontainers/vm.go). With templating enabled, the factory is
	# the template factory, so the symlink is our signal the template was used.
	local sandbox_id
	sandbox_id="$(exec_host "$node" \
		"crictl --runtime-endpoint unix:///run/containerd/containerd.sock pods --name ${pod_name} --state Ready -q | head -1")"

	exec_host "$node" "test -L /run/vc/vm/${sandbox_id} && echo symlink" | grep -q symlink
}

teardown() {
	vm_templating_supported || return 0

	rm -f "${pod_config:-}"

	# Destroy the VM template and remove the config drop-in on the target node.
	exec_host "$node" "sudo /opt/kata/bin/kata-runtime factory destroy" \
		|| echo "Warning: Failed to destroy VM template on node $node"

	remove_kata_runtime_config_dropin_file "$node" "${dropin_path:-}" \
		|| echo "Warning: Failed to remove Kata runtime config drop-in on node $node"

	teardown_common "${node:-}" "${node_start_time:-}"
}
