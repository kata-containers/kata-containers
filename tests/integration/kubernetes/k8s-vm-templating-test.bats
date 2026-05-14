#!/usr/bin/env bats
#
# Copyright (c) 2024 Kata Containers
#
# SPDX-License-Identifier: Apache-2.0
#
# Tests for Kata VM templating (factory) functionality in Kubernetes integration mode

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

get_shim_config_file() {
	case "${KATA_HYPERVISOR}" in
		*-runtime-rs)
			echo "/opt/kata/share/defaults/kata-containers/runtime-rs/runtimes/${KATA_HYPERVISOR}/configuration-${KATA_HYPERVISOR}.toml"
			;;
		*)
			echo "/opt/kata/share/defaults/kata-containers/runtimes/${KATA_HYPERVISOR}/configuration-${KATA_HYPERVISOR}.toml"
			;;
	esac
}

# With setup_file and teardown_file being used, we use >&3 in some places to direct output to the terminal
# setup_file is used in BATS for one-time initialization for all tests in the file
setup_file() {
	if [[ "${KATA_HYPERVISOR}" == *-runtime-rs ]]; then
		export skip_vm_templating_tests=true
		return 0
	fi

	setup_common || die "setup_common failed"
	config_file="$(get_shim_config_file)"
	backup_file="${config_file}.bats-vm-templating.bak"

	# Get ALL kata nodes
	mapfile -t all_nodes < <(kubectl get nodes -l katacontainers.io/kata-runtime=true -o name | sed 's|^node/||')
	[[ "${#all_nodes[@]}" -gt 0 ]] || die "No Kata nodes found"

	export all_nodes config_file backup_file

	# Configure and initialize VM templates on all Kata nodes
	for n in "${all_nodes[@]}"; do
		echo "Configuring and initializing VM template on node: $n" >&3
		exec_host "$n" "sudo test -f '${backup_file}' || sudo cp '${config_file}' '${backup_file}'" || die "Failed to backup kata config on node $n"
		exec_host "$n" "sudo sed -i -e 's|^#\\?enable_template[[:space:]]*=.*$|enable_template = true|g' -e 's|^#\\?template_path[[:space:]]*=.*$|template_path = \"/run/vc/vm/template\"|g' -e 's|^#\\?shared_fs[[:space:]]*=.*$|shared_fs = \"none\"|g' '${config_file}'" || die "Failed to update kata config on node $n"
		exec_host "$n" "sudo grep -q '^enable_template[[:space:]]*=' '${config_file}' || echo 'enable_template = true' | sudo tee -a '${config_file}' >/dev/null" || die "Failed to set enable_template on node $n"
		exec_host "$n" "sudo grep -q '^template_path[[:space:]]*=' '${config_file}' || echo 'template_path = \"/run/vc/vm/template\"' | sudo tee -a '${config_file}' >/dev/null" || die "Failed to set template_path on node $n"
		exec_host "$n" "sudo grep -q '^shared_fs[[:space:]]*=' '${config_file}' || echo 'shared_fs = \"none\"' | sudo tee -a '${config_file}' >/dev/null" || die "Failed to set shared_fs on node $n"
		exec_host "$n" "sudo kata-runtime factory init" || die "Failed to initialize VM template on node $n"
	done

	echo "VM templates initialized on ${#all_nodes[@]} nodes" >&3
}

setup() {
	if [[ "${skip_vm_templating_tests:-false}" == "true" ]]; then
		skip "VM templating test is only supported for Go runtime"
	fi

	# Select one node for this test
	setup_common || die "setup_common failed"
}

@test "VM template factory is initialized" {
	# Verify factory state on each node
	for n in "${all_nodes[@]}"; do
		exec_host "$n" "test -d /run/vc/vm/template" || skip "VM template directory not found on $n"
	done
}

@test "Pod can be created with templated VM" {
	pod_name="test-templated-pod"
	ctr_name="test-container"

	pod_config=$(mktemp --tmpdir pod_config.XXXXXX.yaml)
	cp "$pod_config_dir/busybox-template.yaml" "$pod_config"

	sed -i "s/POD_NAME/$pod_name/" "$pod_config"
	sed -i "s/CTR_NAME/$ctr_name/" "$pod_config"

	# Create a simple pod to verify templating works
	kubectl create -f "${pod_config}"
	kubectl wait --for=condition=Ready --timeout=120s "pod/${pod_name}" || die "Pod failed to reach Ready state"

	# Verify the pod is running
	kubectl get pod "${pod_name}" | grep Running || die "Pod is not in Running state"

	# Basic test: verify we can execute a command in the pod
	kubectl exec "${pod_name}" -- sh -c "echo 'Hello from templated VM' && exit 0"
}

teardown() {
	# Clean up pod from previous test
	kubectl delete pod "test-templated-pod" 2>/dev/null || true

	teardown_common "${node}" "${node_start_time:-}"
}

teardown_file() {
	if [[ "${skip_vm_templating_tests:-false}" == "true" ]]; then
		return 0
	fi

	# Clean up VM templates on all Kata nodes
	for n in "${all_nodes[@]}"; do
		echo "Destroying VM template on node: $n" >&3
		exec_host "$n" "kata-runtime factory destroy" || echo "Warning: Failed to destroy VM template on node $n" >&3
		exec_host "$n" "if [ -f '${backup_file}' ]; then sudo mv '${backup_file}' '${config_file}'; fi" || echo "Warning: Failed to restore kata config on node $n" >&3
	done

	echo "VM templates destroyed on ${#all_nodes[@]} nodes" >&3
}
