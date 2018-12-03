#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	export KUBECONFIG=/etc/kubernetes/admin.conf
	pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
	tmp_file=$(mktemp -d /tmp/data.XXXX)
	msg="Hello from Kubernetes"
	echo $msg > $tmp_file/index.html
	pod_name="pv-pod"
	# Define temporary file at yaml
	sed -i "s|tmp_data|${tmp_file}|g" ${pod_config_dir}/pv-volume.yaml
}

@test "Create Persistent Volume" {
	volume_name="pv-volume"
	volume_claim="pv-claim"

	# Create the persistent volume
	sudo -E kubectl create -f "${pod_config_dir}/pv-volume.yaml"

	# Check the persistent volume
	sudo -E kubectl get pv $volume_name | grep Available

	# Create the persistent volume claim
	sudo -E kubectl create -f "${pod_config_dir}/volume-claim.yaml"

	# Check the persistent volume claim
	sudo -E kubectl get pvc $volume_claim | grep Bound

	# Create pod
	sudo -E kubectl create -f "${pod_config_dir}/pv-pod.yaml"

	# Check pod creation
	sudo -E kubectl wait --for=condition=Ready pod "$pod_name"

	cmd="cat /mnt/index.html"
	sudo -E kubectl exec $pod_name -- sh -c "$cmd" | grep "$msg"
}

teardown() {
	sudo -E kubectl delete pod "$pod_name"
	sudo rm -rf $tmp_file
}
