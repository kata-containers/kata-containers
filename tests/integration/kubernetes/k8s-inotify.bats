#!/usr/bin/env bats
#
# Copyright (c) 2021 Apple Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"
	get_pod_config_dir
	configmap_yaml_file="${pod_config_dir}/inotify-configmap.yaml"
	pod_yaml_file="${pod_config_dir}/inotify-configmap-pod.yaml"
}

@test "configmap update works, and preserves symlinks" {
        pod_name="inotify-configmap-testing"

        # TODO: disabled due to #8889
        # auto_generate_policy "${pod_yaml_file}" "${configmap_yaml_file}"

        # Create configmap for my deployment
        kubectl apply -f "${configmap_yaml_file}"

        # Create deployment that expects identity-certs
        kubectl apply -f "${pod_yaml_file}"
        kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

        # Update configmap
        kubectl apply -f "${pod_config_dir}"/inotify-updated-configmap.yaml

        # Ideally we'd wait for the pod to complete...
        sleep 120

        # Verify we saw the update
        result=$(kubectl get pod "$pod_name" --output="jsonpath={.status.containerStatuses[]}")
        echo $result | grep -vq Error
}



teardown() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"
	# Debugging information
	kubectl describe "pod/$pod_name"
	kubectl delete pod "$pod_name"
	kubectl delete configmap cm
}
