#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
source "/etc/os-release" || source "/usr/lib/os-release"

issue="https://github.com/kata-containers/runtime/issues/1834"

setup() {
	skip "test not working see: ${issue}"
	get_pod_config_dir
}

@test "Port forwarding" {
	skip "test not working see: ${issue}"
	deployment_name="redis-master"

	# Create deployment
	kubectl apply -f "${pod_config_dir}/redis-master-deployment.yaml"

	# Check deployment
	kubectl wait --for=condition=Available --timeout=$timeout deployment/"$deployment_name"
	kubectl expose deployment/"$deployment_name"

	# Get pod name
	pod_name=$(kubectl get pods --output=jsonpath={.items..metadata.name})
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# View replicaset
	kubectl get rs

	# Create service
	kubectl apply -f "${pod_config_dir}/redis-master-service.yaml"

	# Check service
	kubectl get svc | grep redis

	# Check redis service
	port_redis=$(kubectl get pods $pod_name --template='{{(index (index .spec.containers 0).ports 0).containerPort}}{{"\n"}}')

	# Verify that redis is running in the pod and listening on port
	port=6379
	[ "$port_redis" -eq "$port" ]

	# Forward a local port to a port on the pod
	(2&>1 kubectl port-forward "$pod_name" 7000:"$port"> /dev/null) &

	# Run redis-cli
	retries="10"
	ok="0"

	for _ in $(seq 1 "$retries"); do
		if sudo -E redis-cli -p 7000 ping | grep -q "PONG" ; then
			ok="1"
			break;
		fi
		sleep 1
	done

	[ "$ok" -eq "1" ]
}

teardown() {
	skip "test not working see: ${issue}"
	kubectl delete -f "${pod_config_dir}/redis-master-deployment.yaml"
	kubectl delete -f "${pod_config_dir}/redis-master-service.yaml"
}
