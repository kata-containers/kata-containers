#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	export KUBECONFIG=/etc/kubernetes/admin.conf
	pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
}

@test "Guaranteed QoS" {
	pod_name="qos-test"

	# Create pod
	sudo -E kubectl create -f "${pod_config_dir}/pod-guaranteed.yaml"

	# Check pod creation
	sudo -E kubectl wait --for=condition=Ready pod "$pod_name"

	# Check pod class
	sudo -E kubectl get pod "$pod_name" --output=yaml | grep "qosClass: Guaranteed"
}

@test "Burstable QoS" {
	pod_name="burstable-test"

	# Create pod
	sudo -E kubectl create -f "${pod_config_dir}/pod-burstable.yam"l

	# Check pod creation
	sudo -E kubectl wait --for=condition=Ready pod "$pod_name"

	# Check pod class
	sudo -E kubectl get pod "$pod_name" --output=yaml | grep "qosClass: Burstable"
}

@test "BestEffort QoS" {
	pod_name="besteffort-test"

	# Create pod
	sudo -E kubectl create -f "${pod_config_dir}/pod-besteffort.yam"l

	# Check pod creation
	sudo -E kubectl wait --for=condition=Ready pod "$pod_name"

	# Check pod class
	sudo -E kubectl get pod "$pod_name" --output=yaml | grep "qosClass: BestEffort"
}

teardown() {
	sudo -E kubectl delete pod "$pod_name"
}
