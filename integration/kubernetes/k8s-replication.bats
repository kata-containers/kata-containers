#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

setup() {
	export KUBECONFIG="$HOME/.kube/config"
	get_pod_config_dir
}

@test "Replication controller" {
	replication_name="replicationtest"
	number_of_replicas="1"
	wait_time=20
	sleep_time=2

	# Create replication controller
	kubectl create -f "${pod_config_dir}/replication-controller.yaml"

	# Check replication controller
	kubectl describe replicationcontrollers/"$replication_name" | grep "replication-controller"

	# Check pod creation
	pod_name=$(kubectl get pods --output=jsonpath={.items..metadata.name})
	cmd="kubectl wait --for=condition=Ready pod $pod_name"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Check number of pods created for the
	# replication controller is equal to the
	# number of replicas that we defined
	launched_pods=$(echo $pod_name | wc -l)

	[ "$launched_pods" -eq "$number_of_replicas" ]
}

teardown() {
	kubectl delete pod "$pod_name"
	kubectl delete rc "$replication_name"
}
