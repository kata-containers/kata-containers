#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

# Saves the ip addresses
declare -a IP_ADDRESSES

setup() {
	export KUBECONFIG="$HOME/.kube/config"
	deployment="hello-world"
	service="my-service"
	port="8080"
	get_pod_config_dir
}

@test "Expose IP Address" {
	wait_time=20
	sleep_time=2

	# Create deployment
	kubectl create -f "${pod_config_dir}/deployment-expose-ip.yaml"

	# Check deployment creation
	cmd="kubectl wait --for=condition=Available deployment/${deployment}"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Get deployment
	kubectl get deployments ${deployment}

	# Describe deployment
	kubectl describe deployments ${deployment}

	# Display information about ReplicaSet objects
	kubectl get replicasets
	kubectl describe replicasets

	# Expose deployment
	kubectl expose deployment/${deployment} --type=LoadBalancer --name=${service}

	# Get service
	kubectl get services ${service}
	kubectl describe services ${service}

	# Check pods are running
	cmd="kubectl get pods -o jsonpath='{.items[*].status.phase}' | grep Running"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Verify pods IP Addresses
	IP_ADDRESSES=$(kubectl get pods -o jsonpath='{.items[*].status.podIP}')

	for i in ${IP_ADDRESSES[@]}; do
		curl http://$i:$port | grep "Hello Kubernetes"
	done
}

teardown() {
	kubectl delete services ${service}
	kubectl delete deployment ${deployment}
}
