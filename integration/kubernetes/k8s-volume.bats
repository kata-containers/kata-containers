#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
TEST_INITRD="${TEST_INITRD:-no}"
issue="https://github.com/kata-containers/runtime/issues/1127"

setup() {
	[ "${TEST_INITRD}" == "yes" ] && skip "test not working see: ${issue}"

	export KUBECONFIG=/etc/kubernetes/admin.conf

	if sudo -E kubectl get runtimeclass | grep kata; then
		pod_config_dir="${BATS_TEST_DIRNAME}/runtimeclass_workloads"
	else
		pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
	fi

	tmp_file=$(mktemp -d /tmp/data.XXXX)
	msg="Hello from Kubernetes"
	echo $msg > $tmp_file/index.html
	pod_name="pv-pod"
	# Define temporary file at yaml
	sed -i "s|tmp_data|${tmp_file}|g" ${pod_config_dir}/pv-volume.yaml
}

@test "Create Persistent Volume" {
	[ "${TEST_INITRD}" == "yes" ] && skip "test not working see: ${issue}"
	wait_time=10
	sleep_time=2
	volume_name="pv-volume"
	volume_claim="pv-claim"

	# Create the persistent volume
	sudo -E kubectl create -f "${pod_config_dir}/pv-volume.yaml"

	# Check the persistent volume
	sudo -E kubectl get pv $volume_name | grep Available

	# Create the persistent volume claim
	sudo -E kubectl create -f "${pod_config_dir}/volume-claim.yaml"

	# Check the persistent volume claim
	cmd="sudo -E kubectl get pvc $volume_claim | grep Bound"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Create pod
	sudo -E kubectl create -f "${pod_config_dir}/pv-pod.yaml"

	# Check pod creation
	sudo -E kubectl wait --for=condition=Ready pod "$pod_name"

	cmd="cat /mnt/index.html"
	sudo -E kubectl exec $pod_name -- sh -c "$cmd" | grep "$msg"
}

teardown() {
	[ "${TEST_INITRD}" == "yes" ] && skip "test not working see: ${issue}"
	sudo -E kubectl delete pod "$pod_name"
	sudo -E kubectl delete pvc "$volume_claim"
	sudo -E kubectl delete pv "$volume_name"
	sudo rm -rf $tmp_file
}
