#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"
TEST_INITRD="${TEST_INITRD:-no}"
issue="https://github.com/kata-containers/runtime/issues/1127"

setup() {
	[ "${TEST_INITRD}" == "yes" ] && skip "test not working see: ${issue}"

	export KUBECONFIG="$HOME/.kube/config"
	get_pod_config_dir

	tmp_file=$(mktemp -d /tmp/data.XXXX)
	pod_yaml=$(mktemp --tmpdir pod_config.XXXXXX.yaml)
	msg="Hello from Kubernetes"
	echo $msg > $tmp_file/index.html
	pod_name="pv-pod"
	# Define temporary file at yaml
	sed -e "s|tmp_data|${tmp_file}|g" ${pod_config_dir}/pv-volume.yaml > "$pod_yaml"
}

@test "Create Persistent Volume" {
	[ "${TEST_INITRD}" == "yes" ] && skip "test not working see: ${issue}"
	wait_time=10
	sleep_time=2
	volume_name="pv-volume"
	volume_claim="pv-claim"

	# Create the persistent volume
	kubectl create -f "$pod_yaml"

	# Check the persistent volume is Available
	cmd="kubectl get pv $volume_name | grep Available"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Create the persistent volume claim
	kubectl create -f "${pod_config_dir}/volume-claim.yaml"

	# Check the persistent volume claim is Bound.
	cmd="kubectl get pvc $volume_claim | grep Bound"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Create pod
	kubectl create -f "${pod_config_dir}/pv-pod.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$pod_name"

	cmd="cat /mnt/index.html"
	kubectl exec $pod_name -- sh -c "$cmd" | grep "$msg"
}

teardown() {
	[ "${TEST_INITRD}" == "yes" ] && skip "test not working see: ${issue}"
	kubectl delete pod "$pod_name"
	kubectl delete pvc "$volume_claim"
	kubectl delete pv "$volume_name"
	rm -f "$pod_yaml"
	rm -rf "$tmp_file"
}
