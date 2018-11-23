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
	export KUBECONFIG=/etc/kubernetes/admin.conf
	# Pull the images before launching workload.
	sudo -E crictl pull "$busybox_image"
	sudo -E crictl pull "$nginx_image"
	pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
}

@test "Verify nginx connectivity between pods" {
	wait_time=30
	sleep_time=3
	sudo -E kubectl create -f "${pod_config_dir}/${deployment}.yaml"
	sudo -E kubectl wait --for=condition=Available deployment/${deployment}
	sudo -E kubectl expose deployment/${deployment}

	busybox_pod="test-nginx"
	sudo -E kubectl run $busybox_pod --restart=Never --image="$busybox_image" \
		-- wget --timeout=5 "$deployment"
	cmd="sudo -E kubectl get pods -a | grep $busybox_pod | grep Completed"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"
	sudo -E kubectl logs "$busybox_pod" | grep "index.html"
	sudo -E kubectl describe pod "$busybox_pod"
}

teardown() {
	sudo -E kubectl delete deployment "$deployment"
	sudo -E kubectl delete service "$deployment"
	sudo -E kubectl delete pod "$busybox_pod"
}
