#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
TEST_INITRD="${TEST_INITRD:-no}"

# Not working on ARM CI see https://github.com/kata-containers/tests/issues/4727  
setup() {
	get_pod_config_dir
}

@test "Guaranteed QoS" {
	pod_name="qos-test"

	# Create pod
	kubectl create -f "${pod_config_dir}/pod-guaranteed.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check pod class
	kubectl get pod "$pod_name" --output=yaml | grep "qosClass: Guaranteed"
}

@test "Burstable QoS" {
	pod_name="burstable-test"

	# Create pod
	kubectl create -f "${pod_config_dir}/pod-burstable.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check pod class
	kubectl get pod "$pod_name" --output=yaml | grep "qosClass: Burstable"
}

@test "BestEffort QoS" {
	pod_name="besteffort-test"

	# Create pod
	kubectl create -f "${pod_config_dir}/pod-besteffort.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check pod class
	kubectl get pod "$pod_name" --output=yaml | grep "qosClass: BestEffort"
}

teardown() {
	kubectl delete pod "$pod_name"
}
