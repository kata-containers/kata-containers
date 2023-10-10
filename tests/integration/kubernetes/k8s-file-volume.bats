#!/usr/bin/env bats
#
# Copyright (c) 2022 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
TEST_INITRD="${TEST_INITRD:-no}"

setup() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"
	pod_name="test-file-volume"
	container_name="busybox-file-volume-container"
	node="$(get_one_kata_node)"
	tmp_file=$(exec_host "$node" mktemp /tmp/file-volume-test-foo.XXXXX)
	mount_path="/tmp/foo.txt"
	file_body="test"
	get_pod_config_dir
}

@test "Test readonly volume for pods" {
	# Write test body to temp file
	exec_host "$node" "echo "$file_body" > $tmp_file"

	# Create test yaml
	sed -e "s|HOST_FILE|$tmp_file|" ${pod_config_dir}/pod-file-volume.yaml > ${pod_config_dir}/test-pod-file-volume.yaml
	sed -i "s|MOUNT_PATH|$mount_path|" ${pod_config_dir}/test-pod-file-volume.yaml
	sed -i "s|NODE|$node|" ${pod_config_dir}/test-pod-file-volume.yaml

	# Create pod
	kubectl create -f "${pod_config_dir}/test-pod-file-volume.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Validate file volume body inside the pod
	file_in_container=$(kubectl exec $pod_name -- cat $mount_path)
	[ "$file_body" == "$file_in_container" ]
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"
	kubectl delete pod "$pod_name"
	exec_host "$node" rm -f $tmp_file
	rm -f ${pod_config_dir}/test-pod-file-volume.yaml.yaml
}
