#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
TEST_INITRD="${TEST_INITRD:-no}"

setup() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	get_pod_config_dir

	node=$(get_one_kata_node)
	tmp_file=$(mktemp -u /tmp/data.XXXX)
	exec_host "$node" mkdir $tmp_file
	pv_yaml=$(mktemp --tmpdir pv_config.XXXXXX.yaml)
	pod_yaml=$(mktemp --tmpdir pod_config.XXXXXX.yaml)
	msg="Hello from Kubernetes"
	exec_host "$node" "echo $msg > $tmp_file/index.html"
	pod_name="pv-pod"
	# Define temporary file at yaml
	sed -e "s|tmp_data|${tmp_file}|g" ${pod_config_dir}/pv-volume.yaml > "$pv_yaml"
	sed -e "s|NODE|${node}|g" "${pod_config_dir}/pv-pod.yaml" > "$pod_yaml"

	# Add policy to the pod yaml
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	cmd="cat /mnt/index.html"
	exec_command=(sh -c "${cmd}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${pod_yaml}"
}

@test "Create Persistent Volume" {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"

	volume_name="pv-volume"
	volume_claim="pv-claim"

	# Create the persistent volume
	kubectl create -f "$pv_yaml"

	# Check the persistent volume is Available
	cmd="kubectl get pv $volume_name | grep Available"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Create the persistent volume claim
	kubectl create -f "${pod_config_dir}/volume-claim.yaml"

	# Check the persistent volume claim is Bound.
	cmd="kubectl get pvc $volume_claim | grep Bound"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Create pod
	kubectl create -f "$pod_yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	grep_pod_exec_output "$pod_name" "$msg" "${exec_command[@]}"
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
	rm -f "$pod_yaml"
	kubectl delete pvc "$volume_claim"
	kubectl delete pv "$volume_name"
	rm -f "$pv_yaml"
	exec_host "$node" rm -rf "$tmp_file"

	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}
