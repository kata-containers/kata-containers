#!/usr/bin/env bats
#
# Copyright (c) 2021 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	node="$(get_one_kata_node)"
	pod_name="test-readonly-volume"
	container_name="busybox-ro-volume-container"
	tmp_file="ro-volume-test-foobarfoofoo"
	ro_volume_suffix="-tmp"
	if exec_host "$node" which apt-get; then
		exec_host "$node" apt-get install -y expect
	elif exec_host "$node" which tdnf; then
		exec_host "$node" tdnf install -y expect
	fi
	copy_file_to_host ro-volume-exp.sh
	get_pod_config_dir
}

@test "Test readonly volume for pods" {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	# Create pod
	kubectl create -f "${pod_config_dir}/pod-readonly-volume.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Validate readonly volume mount inside pod
	check_cmd="mount|grep /tmp"
	kubectl exec $pod_name -- sh -c "$check_cmd" | grep '\<ro\>'

	# Validate readonly volume mount in the guest
	pod_id=$(exec_host "$node" sudo -E crictl pods -q -s Ready --name $pod_name)
	exec_host "$node" sudo bash ./ro-volume-exp.sh $pod_id $ro_volume_suffix $tmp_file || true

	# runtime-rs host path
	rw="/run/kata-containers/shared/sandboxes/$pod_id/rw/"
	ro="/run/kata-containers/shared/sandboxes/$pod_id/ro/"
	
	# runtime-go host path
	shared_mounts="/run/kata-containers/shared/sandboxes/$pod_id/shared/"
	host_mounts="/run/kata-containers/shared/sandboxes/$pod_id/mounts/"

	pid=$(ps -ef | grep $pod_id | grep -v grep | awk '{print $2}')
	if [[ "${KATA_HYPERVISOR}" == "dragonball" ]]; then
		exec_host "$node" "sudo nsenter -t $pid -m ls -lR $rw | grep $tmp_file && echo "should not find $tmp_file in shared mounts" && false"
		exec_host "$node" "sudo nsenter -t $pid -m ls -lR $ro | grep $tmp_file && echo "should not find $tmp_file in host mounts" && false"
	else	
		exec_host "$node" "sudo ls -lR $shared_mounts | grep $tmp_file && echo "should not find $tmp_file in shared mounts" && false"
		exec_host "$node" "sudo ls -lR $host_mounts | grep $tmp_file && echo "should not find $tmp_file in host mounts" && false"
	fi

	# Validate readonly volume mount on the host
	if [[ "${KATA_HYPERVISOR}" == "dragonball" ]]; then
		exec_host "$node" "sudo nsenter -t $pid -m mount | grep $rw | grep -- $ro_volume_suffix | grep '\<ro\>'"
		exec_host "$node" "sudo nsenter -t $pid -m mount | grep $ro | grep -- $ro_volume_suffix | grep '\<ro\>'"
	else 
		container_id=$(exec_host "$node" sudo -E crictl ps -q --state Running --name $container_name)
		exec_host "$node" "mount | grep $shared_mounts | grep $container_id | grep -- $ro_volume_suffix | grep '\<ro\>'"
		exec_host "$node" "mount | grep $host_mounts | grep $container_id | grep -- $ro_volume_suffix | grep '\<ro\>'"
	fi
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	kubectl delete pod "$pod_name"
	if exec_host "$node" which apt-get; then
		exec_host "$node" apt-get autoremove -y expect
	elif exec_host "$node" which tdnf; then
    	exec_host "$node" tdnf remove -y expect
	fi
	exec_host "$node" rm ro-volume-exp.sh
}
