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

	# kata-runtime defaults to the QEMU config; point it at the active
	# hypervisor so that factory init/destroy use the correct configuration.
	kata_config_path="/opt/kata/share/defaults/kata-containers/runtimes/${KATA_HYPERVISOR}/configuration-${KATA_HYPERVISOR}.toml"
}

@test "Pod can be created with a templated VM" {
	# Initialize the VM template on the target node.
	exec_host "$node" "nsenter --mount=/proc/1/ns/mnt /opt/kata/bin/kata-runtime --config ${kata_config_path} factory init"

	# The factory init above must have created the template directory. exec_host
	# pipes the remote output through `tr`, so the pipeline's exit status is not
	# the remote command's; assert on the output instead. Check inside PID 1's
	# mount namespace, where the template tmpfs was actually mounted.
	exec_host "$node" "nsenter --mount=/proc/1/ns/mnt test -f /run/vc/vm/template/memory && echo present" | grep -q present

	pod_name="test-templated-pod"
	ctr_name="test-container"

	pod_config=$(mktemp --tmpdir pod_config.XXXXXX.yaml)
	cp "$pod_config_dir/busybox-template.yaml" "$pod_config"

	sed -i "s/POD_NAME/$pod_name/" "$pod_config"
	sed -i "s/CTR_NAME/$ctr_name/" "$pod_config"

	kubectl create -f "${pod_config}"
	kubectl wait --for=condition=Ready --timeout="$timeout" "pod/${pod_name}"

	grep_pod_exec_output "${pod_name}" "Hello from templated VM" sh -c "echo 'Hello from templated VM'"

	# Confirm at least one VM sandbox under /run/vc/vm/ is a symlink, which
	# proves the factory/template path was used. A non-templated VM creates a
	# real directory at /run/vc/vm/<sandbox-id>/, whereas a factory-spawned VM
	# stores its state under a generated UUID and /run/vc/vm/<sandbox-id> is a
	# symlink pointing at it (see assignSandbox() in
	# src/runtime/virtcontainers/vm.go). Inspect PID 1's mount namespace, where
	# the shim creates these entries alongside the template tmpfs.
	exec_host "$node" \
		"nsenter --mount=/proc/1/ns/mnt find /run/vc/vm -maxdepth 1 -mindepth 1 -type l ! -name template | grep -q . && echo symlink" \
		| grep -q symlink
}

teardown() {
	vm_templating_supported || return 0

	rm -f "${pod_config:-}"

	# Destroy the VM template and remove the config drop-in on the target node.
	# factory destroy must run in PID 1's mount namespace to unmount the template
	# tmpfs that factory init created there (see the @test for details).
	exec_host "$node" "nsenter --mount=/proc/1/ns/mnt /opt/kata/bin/kata-runtime --config ${kata_config_path} factory destroy" \
		|| echo "Warning: Failed to destroy VM template on node $node"

	remove_kata_runtime_config_dropin_file "$node" "${dropin_path:-}" \
		|| echo "Warning: Failed to remove Kata runtime config drop-in on node $node"

	teardown_common "${node:-}" "${node_start_time:-}"
}
