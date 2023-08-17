#!/usr/bin/env bats
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	node_name="$(kubectl get node -o name)"
	pod_name="busybox"
	nfd_tee_pod_name="nfd-tee-test"

	get_pod_config_dir
	get_tee
	get_tee_key_resource
	get_tee_key_label_jsonpath
}

@test "USING_NFD: true | Ensure the NodeFeatureRule is properly created" {
	[[ "${USING_NFD}" == "true" ]] || skip "Test skipped as NFD is not used"

	[[ $(kubectl get nodefeaturerule -o name | grep ${tee}-total-keys) ]]
}

@test "USING_NFD: false | Ensure the NodeFeatureRule is not created" {
	[[ "${USING_NFD}" == "false" ]] || skip "Test skipped as NFD is used"

	[[ ! $(kubectl get nodefeaturerule -o name | grep ${tee}-total-keys) ]]
}

@test "USING_NFD: true | Ensure the runtimeclass is properly created" {
	[[ "${USING_NFD}" == "true" ]] || skip "Test skipped as NFD is not used"

	kubectl describe runtimeclass kata-"${KATA_HYPERVISOR}"
	kubectl describe runtimeclass kata-"${KATA_HYPERVISOR}" | grep "${tee_key_resource}"

	[[ $(kubectl describe runtimeclass kata-"${KATA_HYPERVISOR}" | grep "${tee_key_resource}") ]]
}

@test "USING_NFD: false | Ensure the runtimeclass is properly created" {
	[[ "${USING_NFD}" == "false" ]] || skip "Test skipped as NFD is not used"

	for t in "${tees_key_resource[@]}"; do
		# Ensure no tee_key_resource is added to a runtimeclass it does not belong to
		[[ ! $(kubectl describe runtimeclass kata-"${KATA_HYPERVISOR}" | grep "${t}") ]]
	done
}

@test "USING_NFD: true | Validate keys book keeping" {
	[[ "${USING_NFD}" == "true" ]] || skip "Test skipped as NFD is not used"

	# NOTE: I hate using `tr -s ' ' | grep ...` to get such info, but as `kubectl describe node`
	# does *NOT* provide a json output, and checking for Allocatable is out of question, for now,
	# due to https://github.com/kubernetes/kubernetes/issues/115190, this is the best we can do.

	# Create a pod, which should decrease the key capacity for the specific TEE
	kubectl create -f "${pod_config_dir}/busybox-pod.yaml"
	kubectl wait --for=condition=Ready --timeout=${timeout} pod ${pod_name}

	# Ensure we have 1 key marked as allocated
	[[ $(kubectl describe ${node_name} | tr -s ' ' | grep "${tee_key_resource} 1 0") ]]

	# Delete a pod, which should increase the keys allocated for the specific TEE
	kubectl delete -f "${pod_config_dir}/busybox-pod.yaml"
	[[ $(kubectl describe ${node_name} | tr -s ' ' | grep "${tee_key_resource} 0 0") ]]
}

@test "USING_NFD: true | Fail if the limit of keys is reached" {
	[[ "${USING_NFD}" == "true" ]] || skip "Test skipped as NFD is not used"

	# Get the amount of keys provided by the TEE
	eval keys=$(kubectl get ${node_name} ${tee_key_label_jsonpath})
	[[ ${keys} -gt 0 ]]

	# Let's make sure we add the ${keys} amount to the limit requested, as when Kubernetes sums it
	# up with the one key set as part of the podOverhead, it'll exceed the limit of available keys.
	sed -i -e "s|KEY_RESOURCE: KEYS|${tee_key_resource}: ${keys}|g" "${pod_config_dir}/pod-tee-maximum-keys.yaml"
	kubectl create -f "${pod_config_dir}/pod-tee-maximum-keys.yaml"

	sleep 20s

	kubectl describe pod ${nfd_tee_pod_name}
	[[ $(kubectl describe pod ${nfd_tee_pod_name} | grep "Insufficient ${tee_key_resource}") ]]
}

teardown() {
	# Debugging information
	kubectl get runtimeclass || true
	kubectl describe runtimeclass || true
	kubectl get nodefeaturerule -A || true
	kubectl describe nodefeaturerule -A || true
	kubectl describe "pod/${pod_name}" || true
	kubectl delete pod "${pod_name}" || true
	kubectl describe "pod/${nfd_tee_pod_name}" || true
	kubectl delete pod "${nfd_tee_pod_name}" || true
}
