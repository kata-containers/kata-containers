#!/usr/bin/env bats
#
# Copyright (c) 2021 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"
	
	get_pod_config_dir

	pod_name="nested-configmap-secret-pod"
}

@test "Nested mount of a secret volume in a configmap volume for a pod" {
	# Creates a configmap, secret and pod that mounts the secret inside the configmap
	kubectl create -f "${pod_config_dir}/pod-nested-configmap-secret.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check config/secret value are correct
	[ "myconfig" == $(kubectl exec $pod_name -- cat /config/config_key) ]
	[ "mysecret" == $(kubectl exec $pod_name -- cat /config/secret/secret_key) ]
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	# Debugging information
	kubectl describe "pod/$pod_name"

	# Delete the configmap, secret, and pod used for testing
	kubectl delete -f "${pod_config_dir}/pod-nested-configmap-secret.yaml"
}
