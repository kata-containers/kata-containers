#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	versions_file="${BATS_TEST_DIRNAME}/../../versions.yaml"
	nginx_version=$("${GOPATH}/bin/yq" read "$versions_file" "docker_images.nginx.version")
	nginx_image="nginx:$nginx_version"
	busybox_image="busybox"
	deployment="nginx-deployment"
	export KUBECONFIG="$HOME/.kube/config"
	# Pull the images before launching workload.
	sudo -E crictl pull "$busybox_image"
	sudo -E crictl pull "$nginx_image"

	if kubectl get runtimeclass | grep kata; then
		pod_config_dir="${BATS_TEST_DIRNAME}/runtimeclass_workloads"
	else
		pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
	fi
}

@test "Verify nginx connectivity between pods" {
	wait_time=30
	sleep_time=3
	kubectl create -f "${pod_config_dir}/${deployment}.yaml"
	kubectl wait --for=condition=Available deployment/${deployment}
	kubectl expose deployment/${deployment}

	busybox_pod="test-nginx"
	kubectl run $busybox_pod --restart=Never --image="$busybox_image" \
		-- wget --timeout=5 "$deployment"
	cmd="kubectl get pods -a | grep $busybox_pod | grep Completed"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"
	kubectl logs "$busybox_pod" | grep "index.html"
	kubectl describe pod "$busybox_pod"
}

teardown() {
	kubectl delete deployment "$deployment"
	kubectl delete service "$deployment"
	kubectl delete pod "$busybox_pod"
}
