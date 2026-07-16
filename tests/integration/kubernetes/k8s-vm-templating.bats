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
# hypervisors (including runtime-rs CLH), and because it disables shared
# filesystem support it also requires a block-device-based snapshotter.
vm_templating_supported() {
	[[ "${KATA_HYPERVISOR}" == "clh" || "${KATA_HYPERVISOR}" == "clh-runtime-rs" || "${KATA_HYPERVISOR}" == "qemu" ]] || return 1
	is_confidential_runtime_class && return 1
	[[ "${SNAPSHOTTER:-}" =~ ^(blockfile|erofs)$ ]] || return 1
	return 0
}

factory_command() {
	local command="$1"

	case "${KATA_HYPERVISOR}" in
		clh-runtime-rs)
			exec_host "$node" "nsenter --mount=/proc/1/ns/mnt /opt/kata/bin/kata-ctl factory ${command}"
			;;
		*)
			exec_host "$node" "nsenter --mount=/proc/1/ns/mnt /opt/kata/bin/kata-runtime --config ${kata_config_path} factory ${command}"
			;;
	esac
}

configure_runtime_rs_factory() {
	[[ "${KATA_HYPERVISOR}" == "clh-runtime-rs" ]] || return 0

	local backup_id
	backup_id="$(printf '%s' "${BATS_TEST_TMPDIR}" | sha256sum | cut -c1-12)"
	kata_ctl_config_path="/etc/kata-containers/runtime-rs/configuration.toml"
	kata_ctl_config_backup_path="${kata_ctl_config_path}.k8s-vm-templating.${backup_id}.bak"

	# Comment out shared_fs because drop-ins cannot remove an existing key.
	# Point kata-ctl at that same per-shim config because it has no --config flag.
	exec_host "$node" "set -e; \
		mkdir -p /etc/kata-containers/runtime-rs; \
		sed -i '/^[[:space:]]*shared_fs[[:space:]]*=/s/^/# k8s-vm-templating: /' ${kata_config_path}; \
		grep -q '^# k8s-vm-templating: [[:space:]]*shared_fs[[:space:]]*=' ${kata_config_path}; \
		if [ -e ${kata_ctl_config_path} ] || [ -L ${kata_ctl_config_path} ]; then \
			mv ${kata_ctl_config_path} ${kata_ctl_config_backup_path}; \
		fi; \
		ln -s ${kata_config_path} ${kata_ctl_config_path}"
}

restore_runtime_rs_factory() {
	[[ "${KATA_HYPERVISOR}" == "clh-runtime-rs" ]] || return 0
	[[ -n "${kata_ctl_config_path:-}" ]] || return 0

	exec_host "$node" "set -e; \
		sed -i 's/^# k8s-vm-templating: //' ${kata_config_path}; \
		rm -f ${kata_ctl_config_path}; \
		if [ -e ${kata_ctl_config_backup_path} ] || [ -L ${kata_ctl_config_backup_path} ]; then \
			mv ${kata_ctl_config_backup_path} ${kata_ctl_config_path}; \
		fi"
}

setup() {
	if ! vm_templating_supported; then
		skip "VM templating requires a non-confidential clh/qemu hypervisor (including runtime-rs CLH) and a blockfile/erofs snapshotter (KATA_HYPERVISOR=${KATA_HYPERVISOR}, SNAPSHOTTER=${SNAPSHOTTER:-unset})"
	fi

	setup_common || die "setup_common failed"

	# Build a Kata runtime config drop-in that enables VM templating and
	# disables shared_fs (incompatible with templating).
	# QEMU VM templating requires an initrd, CLH does not.
	local factory_section="factory"
	local hypervisor_name="${KATA_HYPERVISOR}"
	local rootfs_override=""
	local shared_fs_override='shared_fs = "none"'
	if [[ "${KATA_HYPERVISOR}" == "qemu" ]]; then
		rootfs_override=$'image = ""\ninitrd = "/opt/kata/share/kata-containers/kata-containers-initrd.img"'
	fi
	if [[ "${KATA_HYPERVISOR}" == "clh-runtime-rs" ]]; then
		factory_section="hypervisor.clh.factory"
		hypervisor_name="clh"
		shared_fs_override=""
	fi

	local runtime_config_dropin_file="${BATS_TEST_TMPDIR}/99-k8s-vm-templating.toml"
	cat > "${runtime_config_dropin_file}" <<DROPIN
[hypervisor.${hypervisor_name}]
${shared_fs_override}
default_vcpus = 1
default_memory = 512
${rootfs_override}

[${factory_section}]
enable_template = true
template_path = "/run/vc/vm/template"
DROPIN

	# Install the drop-in on the node selected by setup_common and record the
	# remote path so teardown can remove it.
	dropin_path="$(set_kata_runtime_config_dropin_file "$node" "${runtime_config_dropin_file}")" \
		|| die "Failed to install Kata runtime config drop-in on node $node"

	if [[ "${KATA_HYPERVISOR}" == "clh-runtime-rs" ]]; then
		kata_config_path="$(get_kata_runtime_config_file "$node")" \
			|| die "Failed to find Kata runtime config for ${KATA_HYPERVISOR}"
		configure_runtime_rs_factory \
			|| die "Failed to configure kata-ctl for ${KATA_HYPERVISOR}"
	else
		# kata-runtime defaults to the QEMU config; point it at the active
		# hypervisor so factory commands use the correct configuration.
		kata_config_path="/opt/kata/share/defaults/kata-containers/runtimes/${KATA_HYPERVISOR}/configuration-${KATA_HYPERVISOR}.toml"
	fi

	# Tests reuse workers, so clear factory state left by an interrupted run.
	factory_command destroy || true
}

@test "Pod can be created with a templated VM" {
	# Initialize the VM template on the target node.
	factory_command init

	# The factory init above must have created the template directory. exec_host
	# pipes the remote output through `tr`, so the pipeline's exit status is not
	# the remote command's; assert on the output instead. Check inside PID 1's
	# mount namespace, where the template tmpfs was actually mounted.
	exec_host "$node" "nsenter --mount=/proc/1/ns/mnt test -f /run/vc/vm/template/memory && echo present" | grep -q present

	pod_name="test-templated-pod"
	ctr_name="test-container"

	pod_config=$(mktemp --tmpdir pod_config.XXXXXX.yaml)
	cp "$pod_config_dir/busybox-template.yaml" "$pod_config"
	yq -i ".spec.nodeName = \"${node}\"" "$pod_config"

	sed -i "s/POD_NAME/$pod_name/" "$pod_config"
	sed -i "s/CTR_NAME/$ctr_name/" "$pod_config"

	kubectl create -f "${pod_config}"
	kubectl wait --for=condition=Ready --timeout="$timeout" "pod/${pod_name}"

	grep_pod_exec_output "${pod_name}" "Hello from templated VM" sh -c "echo 'Hello from templated VM'"

	if [[ "${KATA_HYPERVISOR}" != "clh-runtime-rs" ]]; then
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
	fi
}

teardown() {
	vm_templating_supported || return 0

	rm -f "${pod_config:-}"

	# Destroy the VM template and remove the config drop-in on the target node.
	# factory destroy must run in PID 1's mount namespace to unmount the template
	# tmpfs that factory init created there (see the @test for details).
	factory_command destroy \
		|| echo "Warning: Failed to destroy VM template on node $node"
	restore_runtime_rs_factory \
		|| echo "Warning: Failed to restore kata-ctl configuration on node $node"

	remove_kata_runtime_config_dropin_file "$node" "${dropin_path:-}" \
		|| echo "Warning: Failed to remove Kata runtime config drop-in on node $node"

	teardown_common "${node:-}" "${node_start_time:-}"
}
