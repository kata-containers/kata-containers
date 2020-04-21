#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"
TEST_INITRD="${TEST_INITRD:-no}"
issue="https://github.com/kata-containers/runtime/issues/1127"
memory_issue="https://github.com/kata-containers/runtime/issues/1249"

setup() {
	skip "test not working see: ${issue}, ${memory_issue}"
	export KUBECONFIG="$HOME/.kube/config"
	get_pod_config_dir
}

@test "Guaranteed QoS" {
	skip "test not working see: ${issue}, ${memory_issue}"

	pod_name="qos-test"

	# Create pod
	kubectl create -f "${pod_config_dir}/pod-guaranteed.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$pod_name"

	# Check pod class
	kubectl get pod "$pod_name" --output=yaml | grep "qosClass: Guaranteed"
}

@test "Burstable QoS" {
	skip "test not working see: ${issue}, ${memory_issue}"

	pod_name="burstable-test"

	# Create pod
	kubectl create -f "${pod_config_dir}/pod-burstable.yam"l

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$pod_name"

	# Check pod class
	kubectl get pod "$pod_name" --output=yaml | grep "qosClass: Burstable"
}

@test "BestEffort QoS" {
	skip "test not working see: ${issue}, ${memory_issue}"
	pod_name="besteffort-test"

	# Create pod
	kubectl create -f "${pod_config_dir}/pod-besteffort.yam"l

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$pod_name"

	# Check pod class
	kubectl get pod "$pod_name" --output=yaml | grep "qosClass: BestEffort"
}

teardown() {
	skip "test not working see: ${issue}, ${memory_issue}"
	kubectl delete pod "$pod_name"
}
