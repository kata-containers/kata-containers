#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"
issue="https://github.com/kata-containers/runtime/issues/1931"

setup() {
	skip "test not working with ${ID} see: ${issue}"
	versions_file="${BATS_TEST_DIRNAME}/../../versions.yaml"
	nginx_version=$("${GOPATH}/bin/yq" read "$versions_file" "docker_images.nginx.version")
	nginx_image="nginx:$nginx_version"

	export KUBECONFIG="$HOME/.kube/config"
	get_pod_config_dir
}

@test "Replication controller" {
	skip "test not working with ${ID} see: ${issue}"
	replication_name="replicationtest"
	number_of_replicas="1"
	wait_time=20
	sleep_time=2

	# Create yaml
	sed -e "s/\${nginx_version}/${nginx_image}/" \
		"${pod_config_dir}/replication-controller.yaml" > "${pod_config_dir}/test-replication-controller.yaml"

	# Create replication controller
	kubectl create -f "${pod_config_dir}/test-replication-controller.yaml"

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
	skip "test not working with ${ID} see: ${issue}"
	rm -f "${pod_config_dir}/test-replication-controller.yaml"
	kubectl delete pod "$pod_name"
	kubectl delete rc "$replication_name"
}
