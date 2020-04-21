#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

setup() {
	versions_file="${BATS_TEST_DIRNAME}/../../versions.yaml"
	nginx_version=$("${GOPATH}/bin/yq" read "$versions_file" "docker_images.nginx.version")
	nginx_image="nginx:$nginx_version"
	replicas="3"
	deployment="nginx-deployment"
	export KUBECONFIG="$HOME/.kube/config"
	sudo -E crictl pull "$nginx_image"
	get_pod_config_dir
}

@test "Scale nginx deployment" {
	wait_time=30
	sleep_time=3

	sed -e "s/\${nginx_version}/${nginx_image}/" \
		"${pod_config_dir}/${deployment}.yaml" > "${pod_config_dir}/test-${deployment}.yaml"

	kubectl create -f "${pod_config_dir}/test-${deployment}.yaml"
	kubectl wait --for=condition=Available --timeout=60s deployment/${deployment}
	kubectl expose deployment/${deployment}
	kubectl scale deployment/${deployment} --replicas=${replicas}
	cmd="kubectl get deployment/${deployment} -o yaml | grep 'availableReplicas: ${replicas}'"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"
}

teardown() {
	rm -f "${pod_config_dir}/test-${deployment}.yaml"
	kubectl delete deployment "$deployment"
	kubectl delete service "$deployment"
}
