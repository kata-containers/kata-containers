#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"
	get_pod_config_dir

	pod_name="pod-pv"
	volume_claim="local-pvc"
	ctr_mount_path="/mnt"
}

@test "Persistent Volume Support" {
	# Create Persistent Volume Claim
	pcl "${pod_config_dir}/pvc.pcl" | kubectl create -f -

	# Create Workload using Volume
	pcl "${pod_config_dir}/pod-pv.pcl" | kubectl create -f -
	kubectl wait --for condition=ready --timeout=$timeout "pod/${pod_name}"

	# Verify persistent volume claim is bound
	kubectl get "pvc/${volume_claim}" | grep "Bound"

	# write on the mounted volume
	ctr_message="Hello World"
	ctr_file="${ctr_mount_path}/file.txt"
	kubectl exec "$pod_name" -- sh -c "echo $ctr_message > $ctr_file"
	kubectl exec "$pod_name" -- sh -c "grep '$ctr_message' $ctr_file"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	# Delete k8s resources
	kubectl delete pod "$pod_name"
	kubectl delete pvc "$volume_claim"
}
