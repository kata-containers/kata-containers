#!/usr/bin/env bats
#
# Copyright (c) 2021 Apple Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"
	get_pod_config_dir
}

@test "certs rollover works" {
        pod_name="inotify-testing"
        anp_name="inotify-testing"

        # Create ANP for use with my deployment
        kubectl apply -f "${pod_config_dir}"/inotify-anp-policy.yaml

        # Create deployment that expects identity-certs
        kubectl apply -f "${pod_config_dir}"/inotify-certs-pod.yaml
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

        # Update ANP in order to force updated certs to be pushed
        kubectl apply -f "${pod_config_dir}"/inotify-anp-updated-policy.yaml
        sleep 30
 
        # Verify we saw the update
        result=$(kubectl get pod "$pod_name" --output="jsonpath={.status.containerStatuses[]}")
        echo $result | grep -vq Error

        kubectl delete anp "$anp_name"
}

@test "configmap update works, and preserves symlinks" {
        pod_name="inotify-configmap-testing"

        # Create configmap for my deployment
        kubectl apply -f "${pod_config_dir}"/inotify-configmap.yaml

        # Create deployment that expects identity-certs
        kubectl apply -f "${pod_config_dir}"/inotify-configmap-pod.yaml
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

        # Update configmap
        kubectl apply -f "${pod_config_dir}"/inotify-updated-configmap.yaml

        # Ideally we'd wait for the pod to complete...
        sleep 120
 
        # Verify we saw the update
        result=$(kubectl get pod "$pod_name" --output="jsonpath={.status.containerStatuses[]}")
        echo $result | grep -vq Error

        kubectl logs "pod/$pod_name" | grep updated

        kubectl delete configmap cm
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"
	kubectl logs "pod/$pod_name"
	kubectl delete pod "$pod_name"
}
