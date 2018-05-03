#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	nginx_image="nginx"
	busybox_image="busybox"
	service_name="nginx-service"
	export KUBECONFIG=/etc/kubernetes/admin.conf
	master=$(hostname)
	sudo -E kubectl taint nodes "$master" node-role.kubernetes.io/master:NoSchedule-
	# Pull the images before launching workload. This is mainly because we use
	# a timeout and in slow networks it may result in not been able to pull the image
	# successfully.
	sudo -E crictl pull "$busybox_image"
	sudo -E crictl pull "$nginx_image"
}

@test "Verify nginx connectivity between pods" {
	wait_time=30
	sleep_time=5
	cmd="sudo -E kubectl get pods | grep $service_name | grep Running"
	sudo -E kubectl run "$service_name" --image="$nginx_image" --replicas=2
	sudo -E kubectl expose deployment "$service_name" --port=80
	sudo -E kubectl get svc,pod
	# Wait for nginx service to come up
	waitForProcess "$wait_time" "$sleep_time" "$cmd"
	busybox_pod="test-nginx"
	sudo -E kubectl run $busybox_pod --restart=Never --image="$busybox_image" \
		-- wget --timeout=5 "$service_name"
	cmd="sudo -E kubectl get pods -a | grep $busybox_pod | grep Completed"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"
	sudo -E kubectl logs "$busybox_pod" | grep "index.html"
}

teardown() {
	sudo -E kubectl delete deployment "$service_name"
	sudo -E kubectl delete service "$service_name"
	sudo -E kubectl delete pod "$busybox_pod"
	# Wait for the pods to be deleted
	cmd="sudo -E kubectl get pods | grep found."
	waitForProcess "$wait_time" "$sleep_time" "$cmd"
	sudo -E kubectl get pods
}
